//! Windows only: drives the selector with `SendInput`. The X11 equivalent is
//! `live_pick.rs`.
#![cfg(target_os = "windows")]

//! Drive a real drag against the live selector.
//!
//! Like its X11 twin, this takes no shortcuts: it runs the real selector, moves
//! the real cursor, presses the real (synthetic) button, and checks the
//! rectangle that comes back. It is the only thing that proves the selector
//! works.
//!
//! Safe to run on a live desktop. The X11 version gets that from its pointer
//! grab; here it takes three precautions of its own:
//!
//! - A *sink* — a plain window of this process — sits under the drag area for
//!   the whole test, so a button event the selector does not take lands on the
//!   test, never on whatever the user has open.
//! - No press is sent unless the selector's own window is under the cursor, and
//!   no button event at all unless the window there belongs to this process.
//! - A test that fails mid-drag releases the button and puts the cursor back on
//!   the way out, rather than leaving the mouse held down for the user.

use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

use inklift_shot::{
    Capturer, Frame, Outcome, Rect, WindowsCapturer, pick_live_region_ready, virtual_bounds,
};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateSolidBrush, UpdateWindow};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::RemoteDesktop::{
    WTS_CURRENT_SERVER_HANDLE, WTS_CURRENT_SESSION, WTS_SESSIONSTATE_LOCK, WTSFreeMemory,
    WTSINFOEXW, WTSQuerySessionInformationW, WTSSessionInfoEx,
};
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_READOBJECTS, OpenInputDesktop,
};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PWSTR, w};

/// Outline colour the selector paints, as RGB.
const ACCENT: [u8; 3] = [0x4D, 0x8F, 0xBF];
/// The selector's full-screen window class.
const CATCHER_CLASS: &str = "inklift-selection";

fn desktop() -> bool {
    match unsafe { OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS) } {
        Ok(d) => {
            let _ = unsafe { CloseDesktop(d) };
            if locked() {
                eprintln!("skipped: the session is locked");
                return false;
            }
            true
        }
        Err(e) => {
            eprintln!("skipped: no interactive desktop ({e})");
            false
        }
    }
}

/// The lock screen does not stop `OpenInputDesktop` succeeding; see `locked`
/// in tests/win_live.rs for the measurement.
fn locked() -> bool {
    let mut info = PWSTR::null();
    let mut len = 0;
    if let Err(e) = unsafe {
        WTSQuerySessionInformationW(
            Some(WTS_CURRENT_SERVER_HANDLE),
            WTS_CURRENT_SESSION,
            WTSSessionInfoEx,
            &mut info,
            &mut len,
        )
    } {
        eprintln!("could not read the session's lock state ({e}); assuming unlocked");
        return false;
    }
    let flags = unsafe { (*(info.0 as *const WTSINFOEXW)).Data.WTSInfoExLevel1.SessionFlags };
    unsafe { WTSFreeMemory(info.0.cast()) };
    flags == WTS_SESSIONSTATE_LOCK as i32
}

/// The cursor, and the selector that owns it, are global: two drags cannot run
/// at once, and cargo runs tests in a binary concurrently.
static POINTER: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy)]
enum Step {
    Move(i32, i32),
    Down,
    Up,
    RightClick,
    Escape,
    /// Let the selector and the compositor catch up.
    Wait(u64),
    /// Photograph a region mid-drag, while the outline is up.
    Snap(Rect),
}

fn window_under_cursor() -> (HWND, u32, String) {
    let mut pt = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut pt);
        let hwnd = WindowFromPoint(pt);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let mut name = [0u16; 64];
        let len = GetClassNameW(hwnd, &mut name) as usize;
        (hwnd, pid, String::from_utf16_lossy(&name[..len]))
    }
}

fn send_mouse(flags: MOUSE_EVENT_FLAGS) {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 { mi: MOUSEINPUT { dwFlags: flags, ..Default::default() } },
    };
    let sent = unsafe { SendInput(&[input], size_of::<INPUT>() as i32) };
    assert_eq!(sent, 1, "SendInput was blocked (is a higher-integrity window focused?)");
}

fn send_key(vk: VIRTUAL_KEY, up: bool) {
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                ..Default::default()
            },
        },
    };
    let sent = unsafe { SendInput(&[input], size_of::<INPUT>() as i32) };
    assert_eq!(sent, 1, "SendInput was blocked");
}

/// SendInput's button flags are physical; with the buttons swapped in
/// Settings, "left" arrives as the secondary button and would cancel.
fn buttons() -> [MOUSE_EVENT_FLAGS; 4] {
    let swapped = unsafe { GetSystemMetrics(SM_SWAPBUTTON) } != 0;
    if swapped {
        [MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP]
    } else {
        [MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP]
    }
}

/// Dispatch this thread's messages for `ms`, so the sink paints and never
/// looks hung.
fn pump(ms: u64) {
    let until = Instant::now() + Duration::from_millis(ms);
    loop {
        let mut msg = MSG::default();
        while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        if Instant::now() >= until {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn perform(step: Step) {
    let [down, up, rdown, rup] = buttons();
    match step {
        Step::Move(x, y) => unsafe { SetCursorPos(x, y) }.expect("move the cursor"),
        Step::Down => {
            let (_, _, class) = window_under_cursor();
            assert_eq!(
                class, CATCHER_CLASS,
                "the selector does not cover the cursor; refusing to press"
            );
            send_mouse(down);
        }
        // Mid-drag the selector holds the mouse capture, so these reach it
        // whichever of its windows — the catcher or a bar — is under the
        // cursor.
        Step::RightClick | Step::Up => {
            let (_, pid, class) = window_under_cursor();
            assert_eq!(
                pid,
                unsafe { GetCurrentProcessId() },
                "a {class} window of another program is under the cursor; refusing \
                 to send it a button event"
            );
            if matches!(step, Step::Up) {
                send_mouse(up);
            } else {
                send_mouse(rdown);
                pump(20);
                send_mouse(rup);
            }
        }
        Step::Escape => {
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(GetForegroundWindow(), Some(&mut pid)) };
            assert_eq!(
                pid,
                unsafe { GetCurrentProcessId() },
                "the selector is not in the foreground; refusing to send Escape to \
                 another program"
            );
            send_key(VK_ESCAPE, false);
            send_key(VK_ESCAPE, true);
        }
        Step::Wait(ms) => pump(ms),
        Step::Snap(_) => unreachable!("handled by drag_and_snap()"),
    }
    pump(25);
}

unsafe extern "system" fn plain(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(h, m, w, l) }
}

/// A white, never-activated window of this process under `area`, for stray
/// button events to land on. Topmost so it is above the user's windows; the
/// selector's catcher, created after it, goes above it in turn.
struct Sink(HWND);

impl Sink {
    fn over(area: Rect) -> Self {
        unsafe {
            let module = GetModuleHandleW(None).unwrap();
            let class = WNDCLASSW {
                lpfnWndProc: Some(plain),
                hInstance: module.into(),
                lpszClassName: w!("inklift-test-sink"),
                hbrBackground: CreateSolidBrush(COLORREF(0x00FF_FFFF)),
                ..Default::default()
            };
            RegisterClassW(&class); // a second registration fails harmlessly
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("inklift-test-sink"), w!(""), WS_POPUP,
                area.x, area.y, area.width as i32, area.height as i32,
                None, None, Some(module.into()), None,
            )
            .expect("create the sink window");
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            let _ = UpdateWindow(hwnd);
            pump(60);
            Sink(hwnd)
        }
    }
}

impl Drop for Sink {
    fn drop(&mut self) {
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

/// Undo what a failed test would otherwise leave behind: a held button and a
/// cursor somewhere the user did not put it.
struct Restore(POINT);

impl Drop for Restore {
    fn drop(&mut self) {
        // Both sides physical: GetAsyncKeyState reads the physical button, and
        // SendInput's flags name physical buttons.
        if unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) } as u16 & 0x8000 != 0 {
            send_mouse(MOUSEEVENTF_LEFTUP);
        }
        let _ = unsafe { SetCursorPos(self.0.x, self.0.y) };
        pump(20);
    }
}

fn cursor_is_still_at(at: (i32, i32)) {
    let mut pt = POINT::default();
    let _ = unsafe { GetCursorPos(&mut pt) };
    assert_eq!(
        (pt.x, pt.y),
        at,
        "the cursor is not where this test put it — something else moved it. \
         Is someone using the mouse? Rerun without touching it."
    );
}

/// Run the real selector, drive the real cursor through `steps`, return what it
/// decided.
fn drag(steps: &[Step]) -> Option<Outcome> {
    drag_and_snap(steps).map(|(outcome, _, _)| outcome)
}

/// As [`drag`], also returning a frame for every `Step::Snap`, and one of
/// `ring(sel())` grabbed the moment the selector returns — which is when the
/// app captures.
fn drag_and_snap(steps: &[Step]) -> Option<(Outcome, Vec<Frame>, Frame)> {
    let _guard = POINTER.lock().unwrap_or_else(|e| e.into_inner());
    if !desktop() {
        return None;
    }
    // Physical pixels for SetCursorPos, matching what the selector reports.
    unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let cap = WindowsCapturer::new().expect("a capturer on a live desktop");
    let bounds = virtual_bounds(&cap.monitors().unwrap()).unwrap();

    let s = sel();
    let _sink = Sink::over(Rect::new(s.x - 60, s.y - 60, s.width + 120, s.height + 120));
    let mut home = POINT::default();
    let _ = unsafe { GetCursorPos(&mut home) };
    // Declared after the sink, so it runs first and a stray release lands there.
    let _restore = Restore(home);

    let (tx, rx) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(pick_live_region_ready(bounds, 8, move || {
            let _ = ready_tx.send(());
        }));
    });
    if ready_rx.recv_timeout(Duration::from_secs(10)).is_err() {
        panic!("the selector never armed: {:?}", rx.recv_timeout(Duration::from_secs(5)));
    }

    let mut snaps = Vec::new();
    let mut placed = None;
    for &step in steps {
        // Checked before and after every step: a hand on the real mouse turns
        // a correct selector into a wrong rectangle, and that must not read as
        // a selector bug. Seen for real — a click came back as a 70x39 drag.
        if let Some(at) = placed {
            cursor_is_still_at(at);
        }
        match step {
            Step::Snap(r) => snaps.push(cap.grab(&r).expect("capture mid-drag")),
            _ => perform(step),
        }
        if let Step::Move(x, y) = step {
            placed = Some((x, y));
        }
        if let Some(at) = placed {
            cursor_is_still_at(at);
        }
    }

    let (outcome, after) = loop {
        // keep the sink responsive while the selector finishes
        match rx.recv_timeout(Duration::from_millis(20)) {
            Ok(result) => {
                let outcome = result.expect("the selector failed");
                break (outcome, cap.grab(&ring(s)).expect("capture straight after"));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => pump(0),
            Err(e) => panic!("the selector never returned: {e}"),
        }
    };
    Some((outcome, snaps, after))
}

/// Where the drags happen: an offset from the primary monitor's origin, which
/// on Windows is always (0, 0). Needs a primary screen of at least 900x600.
fn sel() -> Rect {
    let _px = unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let cap = WindowsCapturer::new().expect("a capturer on a live desktop");
    let p = cap.monitors().unwrap().into_iter().find(|m| m.primary).unwrap().bounds;
    Rect::new(p.x + 500, p.y + 300, 300, 200)
}

#[test]
fn a_dragged_rectangle_comes_back_as_the_selection() {
    if !desktop() {
        return;
    }
    let s = sel();
    let Some(outcome) = drag(&[
        Step::Move(s.x, s.y),
        Step::Down,
        Step::Move(s.x + 150, s.y + 100),
        Step::Move(s.right(), s.bottom()),
        Step::Up,
    ]) else {
        return;
    };
    assert_eq!(outcome, Outcome::Selected(s));
}

#[test]
fn a_backwards_drag_gives_the_same_rectangle() {
    if !desktop() {
        return;
    }
    // bottom-right to top-left: users do this constantly
    let s = sel();
    let Some(outcome) = drag(&[
        Step::Move(s.right(), s.bottom()),
        Step::Down,
        Step::Move(s.x + 100, s.y + 80),
        Step::Move(s.x, s.y),
        Step::Up,
    ]) else {
        return;
    };
    assert_eq!(outcome, Outcome::Selected(s));
}

#[test]
fn a_click_without_a_drag_is_a_misclick_not_a_capture() {
    if !desktop() {
        return;
    }
    let s = sel();
    let Some(outcome) = drag(&[Step::Move(s.x + 200, s.y + 100), Step::Down, Step::Up]) else {
        return;
    };
    assert_eq!(outcome, Outcome::Cancelled);
}

#[test]
fn a_right_click_mid_drag_cancels() {
    if !desktop() {
        return;
    }
    let s = sel();
    let Some(outcome) = drag(&[
        Step::Move(s.x, s.y),
        Step::Down,
        Step::Move(s.right(), s.bottom()),
        Step::RightClick,
        Step::Up,
    ]) else {
        return;
    };
    assert_eq!(outcome, Outcome::Cancelled);
}

#[test]
fn escape_mid_drag_cancels() {
    if !desktop() {
        return;
    }
    let s = sel();
    let Some(outcome) = drag(&[
        Step::Move(s.x, s.y),
        Step::Down,
        Step::Move(s.right(), s.bottom()),
        Step::Escape,
        // the selector is gone by now; this release lands on the sink
        Step::Up,
    ]) else {
        return;
    };
    assert_eq!(outcome, Outcome::Cancelled);
}

/// The ring two pixels wide around `sel`: exactly where the outline goes.
fn ring(sel: Rect) -> Rect {
    Rect::new(sel.x - 2, sel.y - 2, sel.width + 4, sel.height + 4)
}

/// For each of the four edges, the share of its pixels in the outline colour,
/// and one sample pixel for the failure message. `inset` 0 reads the frame's
/// outermost pixels; 2 reads the selection's own edge when the frame is
/// `ring(sel)`.
fn edge_shares(frame: &Frame, inset: u32) -> [(f64, &'static str, [u8; 3]); 4] {
    let (w, h) = (frame.width(), frame.height());
    let (lo, hx, hy) = (inset, w - 1 - inset, h - 1 - inset);
    let edges: [(&str, Vec<(u32, u32)>); 4] = [
        ("top", (lo..=hx).map(|x| (x, lo)).collect()),
        ("bottom", (lo..=hx).map(|x| (x, hy)).collect()),
        ("left", (lo..=hy).map(|y| (lo, y)).collect()),
        ("right", (lo..=hy).map(|y| (hx, y)).collect()),
    ];
    edges.map(|(name, pts)| {
        let hits = pts.iter().filter(|&&(x, y)| frame.pixel(x, y) == ACCENT).count();
        let (mx, my) = pts[pts.len() / 2];
        (hits as f64 * 100.0 / pts.len() as f64, name, frame.pixel(mx, my))
    })
}

fn most(shares: [(f64, &'static str, [u8; 3]); 4]) -> (f64, &'static str, [u8; 3]) {
    shares.into_iter().fold((f64::MIN, "", [0; 3]), |a, b| if b.0 > a.0 { b } else { a })
}

fn least(shares: [(f64, &'static str, [u8; 3]); 4]) -> (f64, &'static str, [u8; 3]) {
    shares.into_iter().fold((f64::MAX, "", [0; 3]), |a, b| if b.0 < a.0 { b } else { a })
}

/// The selector must not photograph its own outline.
///
/// The bars are drawn strictly outside the selection and destroyed before the
/// capture, but "destroyed" is a request to the compositor, and a frame
/// composed before it acted still has them in. That is an empirical question,
/// so this measures it rather than reasoning about it.
///
/// It checks three things: that mid-drag the outline really is on screen in the
/// expected colour, exactly around the selection (without that, the checks
/// after it would pass for a selector that never drew anything); that the
/// capture's own edges hold none of it; and that the ring where the bars were
/// is clear too, which is the part a slow teardown would fail.
#[test]
fn the_capture_contains_no_pixel_of_the_selectors_own_outline() {
    if !desktop() {
        return;
    }
    let s = sel();
    let Some((outcome, snaps, after)) = drag_and_snap(&[
        Step::Move(s.x, s.y),
        Step::Down,
        Step::Move(s.x + 150, s.y + 100),
        Step::Move(s.right(), s.bottom()),
        Step::Wait(150),
        Step::Snap(ring(s)),
        Step::Up,
    ]) else {
        return;
    };
    assert_eq!(outcome, Outcome::Selected(s));

    let (up, up_edge, seen) = least(edge_shares(&snaps[0], 0));
    assert!(
        up > 90.0,
        "mid-drag, the {up_edge} bar covers only {up:.1}% of the ring around the \
         selection (its middle reads {seen:?}, the outline is {ACCENT:?}) — the \
         outline is missing, misplaced, or not the colour this test looks for, so \
         the residue checks below would prove nothing"
    );

    let (inside, inside_edge, _) = most(edge_shares(&after, 2));
    let (outside, outside_edge, _) = most(edge_shares(&after, 0));
    eprintln!(
        "outline residue: mid-drag ring {up:.1}%; after, selection edge worst \
         {inside:.1}% ({inside_edge}), ring worst {outside:.1}% ({outside_edge})"
    );
    assert!(
        inside < 50.0,
        "the {inside_edge} edge of the capture is {inside:.1}% outline colour — \
         the selector photographed its own border"
    );
    assert!(
        outside < 50.0,
        "the {outside_edge} ring just outside the selection is still {outside:.1}% \
         outline colour — the bars had not gone when the capture was taken"
    );
}
