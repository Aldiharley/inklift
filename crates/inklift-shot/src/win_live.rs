//! Selecting a region on the live screen, on Windows.
//!
//! The same shape as the X11 selector in `live.rs`, and for the same reasons:
//! the user drags over their real screen, nothing paints a copy of it, and the
//! only thing drawn is an outline of four thin windows strictly outside the
//! selection. Every rule about what a drag means lives in [`SelectionState`];
//! this module translates window messages into it and draws what it reports.
//!
//! One thing X11 gives for free has to be built here. An X11 pointer grab takes
//! every click on the display before it happens; `SetCapture` only takes the
//! mouse once a button has gone down over one of this thread's windows. So a
//! window has to be under the first press. That is the *catcher*: one layered
//! window over the whole virtual desktop at alpha 1/255. At that alpha it is
//! invisible — no pixel moves by more than one level — yet still hit-tested,
//! which a fully transparent or colour-keyed window is not: a click on a keyed
//! pixel falls through to whatever is underneath, which is exactly the click
//! this has to stop. It is destroyed with the outline before any capture.

use std::cell::Cell;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{
    COLORREF, ERROR_CLASS_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, POINT, WPARAM,
};
use windows::Win32::Graphics::Dwm::{DWMWA_TRANSITIONS_FORCEDISABLED, DwmFlush, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{BLACK_BRUSH, ClientToScreen, CreateSolidBrush, GetStockObject, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, ReleaseCapture, SetCapture, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, PCWSTR, w};

use crate::geometry::Rect;
use crate::outline::outline_bars;
use crate::selection::{Outcome, SelectionState};
use crate::win::PhysicalPixels;

/// Outline colour, matching the app's accent: RGB 4D 8F BF, which a COLORREF
/// spells backwards.
const ACCENT: COLORREF = COLORREF(0x00BF_8F4D);
/// Outline thickness in pixels.
const THICKNESS: u32 = 2;
/// Nobody drags for two minutes. A bound guarantees the catcher comes down
/// even if the session is abandoned, because a leaked full-screen window makes
/// the whole desktop unclickable.
const DEADLINE: Duration = Duration::from_secs(120);

const CATCHER: PCWSTR = w!("inklift-selection");
const BAR: PCWSTR = w!("inklift-outline");

thread_local! {
    /// Set when another window takes the mouse from the catcher mid-drag.
    /// Without it the release would go to that window and the drag would sit
    /// pending until the deadline.
    static LOST_CAPTURE: Cell<bool> = const { Cell::new(false) };
}

unsafe extern "system" fn catcher_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // lParam is the window gaining the capture
    if msg == WM_CAPTURECHANGED && lp.0 != hwnd.0 as isize {
        LOST_CAPTURE.set(true);
    }
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

unsafe extern "system" fn bar_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

/// Window classes are per process and registering one twice is an error, so
/// this happens once however many selections are made.
fn register_classes() -> Result<(), String> {
    static REGISTERED: OnceLock<Result<(), String>> = OnceLock::new();
    REGISTERED
        .get_or_init(|| unsafe {
            let module = GetModuleHandleW(None).map_err(|e| format!("no module handle: {e}"))?;
            let cross = LoadCursorW(None, IDC_CROSS).unwrap_or_default();
            let register = |name: PCWSTR, proc: WNDPROC, brush: HBRUSH| {
                let class = WNDCLASSW {
                    style: CS_HREDRAW | CS_VREDRAW,
                    lpfnWndProc: proc,
                    hInstance: module.into(),
                    hCursor: cross,
                    hbrBackground: brush,
                    lpszClassName: name,
                    ..Default::default()
                };
                if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
                    return Err(format!(
                        "could not set up the selection window: {}",
                        windows::core::Error::from_win32()
                    ));
                }
                Ok(())
            };
            // Painted black, but at alpha 1 that black is never seen.
            register(CATCHER, Some(catcher_proc), HBRUSH(GetStockObject(BLACK_BRUSH).0))?;
            register(BAR, Some(bar_proc), CreateSolidBrush(ACCENT))
        })
        .clone()
}

/// The catcher and the four bars. Dropping it releases the mouse and destroys
/// every window, so no exit path — an error, a panic — can leave a
/// full-screen window sitting over the desktop.
struct Overlay {
    catcher: HWND,
    bars: [HWND; 4],
}

impl Overlay {
    fn create(desktop: Rect) -> Result<Self, String> {
        let module = unsafe { GetModuleHandleW(None) }.map_err(|e| format!("no module handle: {e}"))?;
        let catcher = unsafe {
            CreateWindowExW(
                // TOOLWINDOW keeps it off the taskbar and out of Alt-Tab.
                WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                CATCHER, w!("inklift selection"), WS_POPUP,
                desktop.x, desktop.y, desktop.width as i32, desktop.height as i32,
                None, None, Some(module.into()), None,
            )
        }
        .map_err(|e| format!("could not cover the screen for selection: {e}"))?;
        // From here on, Drop cleans up whatever has been built.
        let mut overlay = Overlay { catcher, bars: [HWND::default(); 4] };
        unsafe { SetLayeredWindowAttributes(catcher, COLORREF(0), 1, LWA_ALPHA) }
            .map_err(|e| format!("could not make the selection window see-through: {e}"))?;
        no_animation(catcher);

        for bar in overlay.bars.iter_mut() {
            // Owned by the catcher, so always stacked above it; NOACTIVATE so
            // showing one never takes the keyboard from it.
            //
            // LAYERED + TRANSPARENT makes a bar invisible to hit-testing, so
            // the mouse is always over the catcher even where a bar is drawn.
            // Without it the drag test measured selections 2 px short: a
            // release over a bar went to the bar, in coordinates relative to
            // where the bar had been before it moved to follow the drag.
            *bar = unsafe {
                CreateWindowExW(
                    WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW
                        | WS_EX_NOACTIVATE,
                    BAR, w!(""), WS_POPUP,
                    0, 0, 1, 1,
                    Some(catcher), None, Some(module.into()), None,
                )
            }
            .map_err(|e| format!("could not create the outline: {e}"))?;
            // fully opaque: layered only for the hit-testing
            unsafe { SetLayeredWindowAttributes(*bar, COLORREF(0), 255, LWA_ALPHA) }
                .map_err(|e| format!("could not create the outline: {e}"))?;
            no_animation(*bar);
        }
        Ok(overlay)
    }

    fn show_outline(&self, sel: Option<Rect>) {
        match sel {
            Some(rect) => {
                for (hwnd, bar) in self.bars.iter().zip(outline_bars(&rect, THICKNESS)) {
                    let _ = unsafe {
                        SetWindowPos(
                            *hwnd, None,
                            bar.x, bar.y, bar.width as i32, bar.height as i32,
                            SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW,
                        )
                    };
                }
            }
            None => {
                for hwnd in &self.bars {
                    let _ = unsafe { ShowWindow(*hwnd, SW_HIDE) };
                }
            }
        }
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseCapture();
            for hwnd in self.bars.iter().filter(|h| !h.is_invalid()) {
                let _ = DestroyWindow(*hwnd);
            }
            let _ = DestroyWindow(self.catcher);
        }
    }
}

/// DWM fades some windows out when they close. A fading bar is a bar that is
/// still on screen when the capture is taken, so ask for none.
fn no_animation(hwnd: HWND) {
    let on = BOOL::from(true);
    let _ = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            &on as *const _ as *const _,
            size_of::<BOOL>() as u32,
        )
    };
}

/// The whole virtual desktop, in physical pixels (the caller holds
/// `PhysicalPixels`).
fn virtual_desktop() -> Rect {
    unsafe {
        Rect::new(
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN).max(1) as u32,
            GetSystemMetrics(SM_CYVIRTUALSCREEN).max(1) as u32,
        )
    }
}

/// Let the user drag a region on the real screen.
///
/// Returns once they finish, cancel, or the deadline passes. The catcher and
/// all four bars are gone, and the screen repainted, on every exit path.
pub fn pick_live_region(bounds: Rect, min: u32) -> Result<Outcome, String> {
    pick_live_region_ready(bounds, min, || {})
}

/// As [`pick_live_region`], but calls `on_ready` the moment the selector is
/// armed — the catcher is over the screen and the next press is ours.
///
/// Nothing outside can observe that moment reliably, and anything driving the
/// selector rather than a human needs to: a synthetic click sent a moment too
/// early lands on whatever window is underneath.
pub fn pick_live_region_ready<F: FnOnce()>(
    bounds: Rect,
    min: u32,
    on_ready: F,
) -> Result<Outcome, String> {
    // Every coordinate below — the catcher's size, every mouse position, every
    // bar — is in physical pixels, the same space `WindowsCapturer` reports.
    // Windows created on this thread keep this awareness for life.
    let _px = PhysicalPixels::enter()?;
    register_classes()?;
    LOST_CAPTURE.set(false);

    let overlay = Overlay::create(virtual_desktop())?;
    let result = drag_loop(&overlay, bounds, min, on_ready);
    drop(overlay);
    settle();
    result
}

/// Hand every message waiting on this thread to its window.
fn pump() {
    let mut msg = MSG::default();
    while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn drag_loop<F: FnOnce()>(
    overlay: &Overlay,
    bounds: Rect,
    min: u32,
    on_ready: F,
) -> Result<Outcome, String> {
    unsafe {
        let _ = ShowWindow(overlay.catcher, SW_SHOW);
        // Best effort: Windows only lets a process take the foreground when the
        // user just interacted with it, which a click on the app's button or
        // tray menu is. Without it Escape is caught by the poll below instead,
        // and the first click activates the catcher anyway.
        let _ = SetForegroundWindow(overlay.catcher);
    }
    pump();

    // Armed: the catcher covers the screen and is hit-tested.
    on_ready();

    let mut state = SelectionState::new(bounds, min, min);
    let started = Instant::now();

    while started.elapsed() < DEADLINE {
        unsafe { MsgWaitForMultipleObjects(None, false, 20, QS_ALLINPUT) };
        let mut msg = MSG::default();
        while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
            // Client coordinates made global. They are the catcher's — the
            // bars are not hit-tested — and the catcher never moves, so the
            // conversion is exact. Signed: under capture the pointer can be
            // left of or above the window's origin.
            let at = || {
                let mut pt = POINT {
                    x: (msg.lParam.0 & 0xFFFF) as u16 as i16 as i32,
                    y: ((msg.lParam.0 >> 16) & 0xFFFF) as u16 as i16 as i32,
                };
                let _ = unsafe { ClientToScreen(msg.hwnd, &mut pt) };
                (pt.x, pt.y)
            };
            match msg.message {
                WM_LBUTTONDOWN => {
                    let (x, y) = at();
                    state.press(x, y);
                    // Keep the drag ours to the end, whatever window the
                    // pointer is over when the button comes up.
                    unsafe { SetCapture(overlay.catcher) };
                    LOST_CAPTURE.set(false);
                }
                WM_MOUSEMOVE => {
                    let (x, y) = at();
                    state.drag(x, y);
                    overlay.show_outline(state.current());
                }
                WM_LBUTTONUP => {
                    // Where the button came up is where the drag ended, even if
                    // the last move before it was coalesced away.
                    let (x, y) = at();
                    state.drag(x, y);
                    state.release();
                }
                // Right-click is the universal "back out of this".
                WM_RBUTTONDOWN => state.cancel(),
                WM_KEYDOWN if msg.wParam.0 == VK_ESCAPE.0 as usize => state.cancel(),
                _ => {}
            }
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if state.outcome() != Outcome::Pending {
                break;
            }
        }

        // Escape while another window kept the keyboard: see SetForegroundWindow.
        if unsafe { GetAsyncKeyState(VK_ESCAPE.0 as i32) } as u16 & 0x8000 != 0 {
            state.cancel();
        }
        if LOST_CAPTURE.get() {
            state.cancel();
        }
        match state.outcome() {
            Outcome::Pending => {}
            settled => return Ok(settled),
        }
    }
    Err("the selection timed out".into())
}

/// Wait for the screen to be the screen again, once the windows are destroyed.
///
/// The capture is taken straight after this returns, so the outline must be
/// gone from what the compositor presents — not merely destroyed. `DwmFlush`
/// blocks until DWM presents a frame composed after the call; two, because the
/// first may already have been composing when the windows went.
///
/// No fixed sleep, unlike X11's 90 ms, because none was measured to be needed.
/// Under DWM every top-level window draws into its own surface, so destroying
/// ours uncovers pixels that are already rendered: nothing underneath has to
/// repaint. Measured on Windows 11 at 60 Hz (`measure_how_long_the_outline_
/// outlives_its_windows` below), in 80 rounds the outline was never read back
/// after destroy even with no flush at all; the flushes cost ~33 ms and are
/// kept because they are a real signal where a sleep would be a guess, and they
/// scale with a slower display's refresh rate where a constant would not.
fn settle() {
    pump();
    for _ in 0..2 {
        if unsafe { DwmFlush() }.is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::Capturer;
    use crate::win::WindowsCapturer;

    fn outline_visible(cap: &WindowsCapturer, sel: Rect) -> bool {
        // the middle of the top bar
        let probe = Rect::new(sel.x + sel.width as i32 / 2, sel.y - 2, 1, 2);
        cap.grab(&probe).map(|f| f.pixel(0, 0) == [0x4D, 0x8F, 0xBF]).unwrap_or(false)
    }

    /// How long the outline stays on screen after its windows are destroyed.
    ///
    /// Opt-in because it puts windows on the desktop:
    /// `cargo test -p inklift-shot --lib -- --ignored --nocapture --test-threads=1 measure`
    /// (one at a time: each would otherwise read the other's windows).
    #[test]
    #[ignore]
    fn measure_how_long_the_outline_outlives_its_windows() {
        let _px = PhysicalPixels::enter().unwrap();
        register_classes().unwrap();
        let cap = WindowsCapturer::new().unwrap();
        let sel = Rect::new(500, 300, 300, 200);

        let (mut bare, mut flushed) = (Vec::new(), Vec::new());
        for round in 0..40 {
            let overlay = Overlay::create(virtual_desktop()).unwrap();
            unsafe { let _ = ShowWindow(overlay.catcher, SW_SHOWNOACTIVATE); }
            overlay.show_outline(Some(sel));
            let shown = Instant::now();
            while !outline_visible(&cap, sel) {
                pump();
                assert!(shown.elapsed() < Duration::from_secs(2), "the outline never appeared");
            }
            let gone = Instant::now();
            drop(overlay);
            // Alternate: poll straight away, or after the two DwmFlush calls
            // `settle` makes, to see what the flush buys.
            if round % 2 == 1 {
                pump();
                for _ in 0..2 {
                    let _ = unsafe { DwmFlush() };
                }
            }
            let mut polls = 0;
            while outline_visible(&cap, sel) {
                polls += 1;
                assert!(gone.elapsed() < Duration::from_secs(2), "the outline never went");
            }
            let ms = gone.elapsed().as_secs_f64() * 1000.0;
            if round % 2 == 1 { flushed.push((ms, polls)) } else { bare.push((ms, polls)) }
        }
        for (name, v) in [("destroy only", &mut bare), ("destroy + 2x DwmFlush", &mut flushed)] {
            v.sort_by(|a, b| a.0.total_cmp(&b.0));
            let dirty = v.iter().filter(|(_, polls)| *polls > 0).count();
            eprintln!(
                "{name}: clean after min {:.1} ms, median {:.1} ms, max {:.1} ms; \
                 {dirty} of {} rounds read the outline at least once after destroy",
                v[0].0, v[v.len() / 2].0, v[v.len() - 1].0, v.len()
            );
        }
    }

    /// How long an ordinary app window stays on screen after `ShowWindow(SW_HIDE)`,
    /// which is what hiding the main window before a pick does. Unlike the
    /// outline, it keeps DWM's default transitions, as the app's window does.
    #[test]
    #[ignore]
    fn measure_how_long_a_hidden_app_window_stays_on_screen() {
        let _px = PhysicalPixels::enter().unwrap();
        register_classes().unwrap();
        let cap = WindowsCapturer::new().unwrap();
        let module = unsafe { GetModuleHandleW(None) }.unwrap();
        let area = Rect::new(400, 250, 500, 300);
        let probe = Rect::new(area.x + 250, area.y + 150, 1, 1);
        let seen = || cap.grab(&probe).map(|f| f.pixel(0, 0) == [0x4D, 0x8F, 0xBF]).unwrap_or(false);

        let mut times = Vec::new();
        for _ in 0..15 {
            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_TOPMOST, BAR, w!("probe"), WS_OVERLAPPEDWINDOW,
                    area.x - 20, area.y - 60, area.width as i32 + 40, area.height as i32 + 120,
                    None, None, Some(module.into()), None,
                )
            }
            .unwrap();
            unsafe { let _ = ShowWindow(hwnd, SW_SHOW); }
            let shown = Instant::now();
            while !seen() {
                pump();
                assert!(shown.elapsed() < Duration::from_secs(3), "the window never appeared");
            }
            pump();
            std::thread::sleep(Duration::from_millis(300));
            let hidden = Instant::now();
            unsafe { let _ = ShowWindow(hwnd, SW_HIDE); }
            while seen() {
                pump();
                assert!(hidden.elapsed() < Duration::from_secs(3), "the window never went");
            }
            times.push(hidden.elapsed().as_secs_f64() * 1000.0);
            unsafe { let _ = DestroyWindow(hwnd); }
            pump();
        }
        times.sort_by(f64::total_cmp);
        eprintln!(
            "hidden app window: gone from the screen after min {:.1} ms, median {:.1} ms, max {:.1} ms",
            times[0], times[times.len() / 2], times[times.len() - 1]
        );
    }
}
