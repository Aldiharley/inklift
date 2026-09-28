//! Windows only: runs the real app and closes its window the way the title
//! bar's X does.
#![cfg(target_os = "windows")]

//! Closing the window must leave inklift in the tray.
//!
//! The app is tray-resident by design (`design/ux-architecture.md`): the tray's
//! Open, Copy again and Lift items all assume the process outlives its window.
//! Tauri's default is the opposite — the last window closing ends the process,
//! and the tray icon goes with it. That was measured, not supposed: before the
//! fix, SC_CLOSE on the main window left no process and no tray behind.
//!
//! Skips itself where there is no interactive desktop to open a window on.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_READOBJECTS, OpenInputDesktop,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::BOOL;

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

/// Kills the app on the way out, whatever the test decided.
struct App(Child);

impl Drop for App {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Start the real binary and wait for the frontend to say it is wired up —
/// the app's own signal that the window exists and is live.
fn launch() -> App {
    let mut child = Command::new(env!("CARGO_BIN_EXE_inklift-gui"))
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .expect("start inklift-gui");
    let stderr = child.stderr.take().unwrap();
    let app = App(child);
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            eprintln!("  app: {line}");
            if line.contains("wired up") {
                let _ = tx.send(());
            }
        }
    });
    rx.recv_timeout(Duration::from_secs(60)).expect("the app never reported its window wired up");
    app
}

/// The app's main window: its visible top-level Tauri window.
fn main_window(pid: u32) -> Option<HWND> {
    struct Search {
        pid: u32,
        found: Option<HWND>,
    }
    unsafe extern "system" fn each(hwnd: HWND, data: LPARAM) -> BOOL {
        let s = unsafe { &mut *(data.0 as *mut Search) };
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        let mut class = [0u16; 64];
        let len = unsafe { GetClassNameW(hwnd, &mut class) } as usize;
        if pid == s.pid && String::from_utf16_lossy(&class[..len]) == "Tauri Window" {
            s.found = Some(hwnd);
            return false.into();
        }
        true.into()
    }
    let mut search = Search { pid, found: None };
    let _ = unsafe { EnumWindows(Some(each), LPARAM(&mut search as *mut _ as isize)) };
    search.found
}

#[test]
fn closing_the_window_leaves_the_app_running_in_the_tray() {
    if !desktop() {
        return;
    }
    let mut app = launch();
    let pid = app.0.id();
    let hwnd = main_window(pid).expect("the app has a main window");
    assert!(unsafe { IsWindowVisible(hwnd) }.as_bool(), "the window starts visible");

    // Exactly what a click on the title bar's X sends.
    unsafe { PostMessageW(Some(hwnd), WM_SYSCOMMAND, WPARAM(SC_CLOSE as usize), LPARAM(0)) }
        .expect("post SC_CLOSE");

    let until = Instant::now() + Duration::from_secs(3);
    while Instant::now() < until && unsafe { IsWindowVisible(hwnd) }.as_bool() {
        std::thread::sleep(Duration::from_millis(50));
    }
    // Give an exit that is on its way time to land before looking.
    std::thread::sleep(Duration::from_secs(1));

    assert!(
        app.0.try_wait().expect("poll the app").is_none(),
        "closing the window ended the process, and the tray icon went with it — \
         the app is meant to stay in the tray until Quit"
    );
    assert!(
        unsafe { IsWindow(Some(hwnd)) }.as_bool(),
        "the window was destroyed rather than hidden; the tray's Open, Copy again \
         and Lift items all need it to still exist"
    );
    assert!(!unsafe { IsWindowVisible(hwnd) }.as_bool(), "the window is still showing");
}
