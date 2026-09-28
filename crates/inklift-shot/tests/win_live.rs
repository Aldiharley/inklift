//! Exercises real GDI capture against the live desktop. Skips itself when there
//! is no interactive desktop (a service session, a locked workstation), so the
//! suite still passes there.
#![cfg(target_os = "windows")]

use std::time::Duration;

use inklift_shot::{Capturer, Frame, Rect, WindowsCapturer, virtual_bounds};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::Graphics::Gdi::{
    CreateSolidBrush, DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplaySettingsW, HBRUSH,
    UpdateWindow,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_READOBJECTS, OpenInputDesktop,
};
use windows::Win32::UI::HiDpi::{
    AreDpiAwarenessContextsEqual, GetThreadDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

/// The input desktop is what the user sees. Opening it fails under a service,
/// on a locked workstation and behind a UAC prompt — all places there is
/// nothing to capture.
fn desktop() -> bool {
    match unsafe { OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS) } {
        Ok(d) => {
            let _ = unsafe { CloseDesktop(d) };
            true
        }
        Err(e) => {
            eprintln!("skipped: no interactive desktop ({e})");
            false
        }
    }
}

fn capturer() -> Option<WindowsCapturer> {
    if !desktop() {
        return None;
    }
    // Past the desktop check a failure is a real one, not a reason to skip.
    Some(WindowsCapturer::new().expect("a capturer on a live desktop"))
}

#[test]
fn monitors_are_enumerated_with_sane_geometry() {
    let Some(c) = capturer() else { return };
    let monitors = c.monitors().expect("should enumerate");

    assert!(!monitors.is_empty(), "at least one screen must be reported");
    for m in &monitors {
        assert!(m.bounds.width > 0 && m.bounds.height > 0, "{m:?} has no area");
    }
    let primaries: Vec<_> = monitors.iter().filter(|m| m.primary).collect();
    assert_eq!(primaries.len(), 1, "exactly one primary screen: {monitors:?}");
    // Windows defines the primary monitor as the one at the origin.
    assert_eq!((primaries[0].bounds.x, primaries[0].bounds.y), (0, 0));
    let all = virtual_bounds(&monitors).unwrap();
    assert!(all.width > 0 && all.height > 0);
    eprintln!("monitors: {monitors:?}");
}

/// The trap the porting notes warn about. This test process declares no DPI
/// awareness, so on a scaled display Windows hands it shrunken, virtualised
/// coordinates unless the capturer takes care of it. The display *mode* is
/// immune to that — it is the panel's real pixel count — so every monitor must
/// be reported at exactly its mode's size.
#[test]
fn monitors_are_reported_in_physical_pixels_whatever_the_process_dpi_awareness() {
    let Some(c) = capturer() else { return };
    for m in c.monitors().expect("should enumerate") {
        let device: Vec<u16> = m.name.encode_utf16().chain([0]).collect();
        let mut mode = DEVMODEW { dmSize: size_of::<DEVMODEW>() as u16, ..Default::default() };
        let ok = unsafe {
            EnumDisplaySettingsW(PCWSTR(device.as_ptr()), ENUM_CURRENT_SETTINGS, &mut mode)
        };
        assert!(ok.as_bool(), "{} is not a display device name", m.name);
        assert_eq!(
            (m.bounds.width, m.bounds.height),
            (mode.dmPelsWidth, mode.dmPelsHeight),
            "{} is reported at {}x{} but its display mode is {}x{} — these are \
             DPI-virtualised coordinates, and grabs would come back the wrong size",
            m.name, m.bounds.width, m.bounds.height, mode.dmPelsWidth, mode.dmPelsHeight
        );
    }
}

#[test]
fn a_whole_monitor_captures_at_exactly_its_reported_size() {
    let Some(c) = capturer() else { return };
    for m in c.monitors().unwrap() {
        let frame = c.grab(&m.bounds).expect("capture a whole monitor");
        assert_eq!((frame.width(), frame.height()), (m.bounds.width, m.bounds.height), "{}", m.name);
    }
}

#[test]
fn a_small_region_captures_at_exactly_the_requested_size() {
    let Some(c) = capturer() else { return };
    let region = Rect::new(0, 0, 64, 32);

    let frame = c.grab(&region).expect("capture should succeed");

    assert_eq!((frame.width(), frame.height()), (64, 32));
    assert_eq!(frame.to_rgba8().len(), 64 * 32 * 4);
}

#[test]
fn a_region_across_a_monitor_seam_comes_back_whole() {
    let Some(c) = capturer() else { return };
    let monitors = c.monitors().unwrap();
    let Some(second) = monitors.iter().find(|m| !m.primary) else {
        eprintln!("one monitor; no seam to cross");
        return;
    };
    // straddle the second monitor's left or top edge, whichever it shares
    let b = second.bounds;
    let region = Rect::new(b.x - 40, b.y + 40, 80, 60).clamped_to(&virtual_bounds(&monitors).unwrap());
    let region = region.expect("the seam region is on the desktop");
    let frame = c.grab(&region).expect("capture across the seam");
    assert_eq!((frame.width(), frame.height()), (region.width, region.height));
}

#[test]
fn a_region_outside_the_desktop_is_refused() {
    let Some(c) = capturer() else { return };
    assert!(c.grab(&Rect::new(100_000, 100_000, 10, 10)).is_err());
    assert!(c.grab(&Rect::new(-100_000, -100_000, 10, 10)).is_err());
}

#[test]
fn captures_are_self_consistent() {
    let Some(c) = capturer() else { return };
    let region = Rect::new(4, 4, 32, 16);

    let a = c.grab(&region).unwrap();
    let b = c.grab(&region).unwrap();

    assert_eq!(a.to_rgba8().len(), b.to_rgba8().len());
}

/// The capturer switches the calling thread to per-monitor DPI awareness for
/// the duration of each call. Leaking that switch would silently change how
/// every later window on the caller's thread is scaled.
#[test]
fn capturing_leaves_the_callers_dpi_awareness_as_it_found_it() {
    let Some(c) = capturer() else { return };
    let before = unsafe { GetThreadDpiAwarenessContext() };
    let _ = c.monitors().unwrap();
    let _ = c.grab(&Rect::new(0, 0, 8, 8)).unwrap();
    let after = unsafe { GetThreadDpiAwarenessContext() };
    assert!(unsafe { AreDpiAwarenessContextsEqual(before, after) }.as_bool());
}

unsafe extern "system" fn plain(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(h, m, w, l) }
}

/// A topmost, never-activated window filled with one colour.
fn swatch(class: PCWSTR, rgb: (u8, u8, u8), at: Rect) -> HWND {
    unsafe {
        let module = GetModuleHandleW(None).unwrap();
        let (r, g, b) = rgb;
        let brush: HBRUSH = CreateSolidBrush(windows::Win32::Foundation::COLORREF(
            r as u32 | (g as u32) << 8 | (b as u32) << 16,
        ));
        let wc = WNDCLASSW {
            lpfnWndProc: Some(plain),
            hInstance: module.into(),
            lpszClassName: class,
            hbrBackground: brush,
            ..Default::default()
        };
        RegisterClassW(&wc); // a second registration fails harmlessly
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class, w!(""), WS_POPUP,
            at.x, at.y, at.width as i32, at.height as i32,
            None, None, Some(module.into()), None,
        )
        .expect("create a swatch window");
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = UpdateWindow(hwnd);
        hwnd
    }
}

fn pump(for_ms: u64) {
    let until = std::time::Instant::now() + Duration::from_millis(for_ms);
    while std::time::Instant::now() < until {
        let mut msg = MSG::default();
        while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn centre(f: &Frame, r: &Rect, origin: &Rect) -> [u8; 3] {
    let x = (r.x - origin.x) as u32 + r.width / 2;
    let y = (r.y - origin.y) as u32 + r.height / 2;
    f.pixel(x, y)
}

/// Put known colours on the real screen and read them back.
///
/// Pins three things at once: the byte order (red must not come back as blue),
/// the coordinate mapping (a grab offset by a DPI scale reads the wrong
/// swatch, or the desktop), and that the image is the live screen rather than
/// a black frame.
#[test]
fn known_colours_on_screen_are_read_back_exactly() {
    let Some(c) = capturer() else { return };
    // Physical coordinates for the swatches, whatever this process's awareness.
    let _dpi = unsafe {
        windows::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(
            windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        )
    };
    let red = Rect::new(200, 200, 60, 40);
    let green = Rect::new(260, 200, 60, 40);
    let blue = Rect::new(320, 200, 60, 40);
    let windows = [
        swatch(w!("inklift-test-red"), (255, 0, 0), red),
        swatch(w!("inklift-test-green"), (0, 255, 0), green),
        swatch(w!("inklift-test-blue"), (0, 0, 255), blue),
    ];
    pump(150);
    let _ = unsafe { DwmFlush() };

    let region = Rect::new(200, 200, 180, 40);
    let frame = c.grab(&region);
    for w in windows {
        let _ = unsafe { DestroyWindow(w) };
    }
    let frame = frame.expect("capture the swatches");

    assert_eq!(centre(&frame, &red, &region), [255, 0, 0], "red must not come back as blue");
    assert_eq!(centre(&frame, &green, &region), [0, 255, 0]);
    assert_eq!(centre(&frame, &blue, &region), [0, 0, 255]);
    // and the edges line up to the pixel: the last red column, the first green
    assert_eq!(frame.pixel(59, 20), [255, 0, 0], "a swatch edge is off by a pixel");
    assert_eq!(frame.pixel(60, 20), [0, 255, 0], "a swatch edge is off by a pixel");
}
