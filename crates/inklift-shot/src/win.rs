//! Screen capture on Windows, through GDI.
//!
//! GDI rather than DXGI Desktop Duplication or Windows.Graphics.Capture: a
//! one-shot grab of a rectangle is exactly what `BitBlt` from the screen DC
//! does, it reads the composited desktop DWM presents, and it needs no device,
//! no swap of frames, and no WinRT. The newer APIs earn their complexity when
//! streaming frames, which inklift never does.

use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, CreateCompatibleBitmap,
    CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, EnumDisplayMonitors, GetDC,
    GetDIBits, GetMonitorInfoW, HBITMAP, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW, ReleaseDC,
    SRCCOPY, SelectObject,
};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    SetThreadDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;
use windows::core::BOOL;

use crate::capture::{Capturer, Monitor, virtual_bounds};
use crate::frame::Frame;
use crate::geometry::Rect;

/// Switches the calling thread to per-monitor DPI awareness until dropped.
///
/// Without it, Windows lies to a DPI-unaware caller about every coordinate and
/// size on a scaled monitor: at 150% a 3840-pixel-wide panel is reported as
/// 2560, and a grab of what that caller *thinks* is a region comes back
/// stretched from a different one. Whether the process is aware depends on who
/// built it. Measured with `GetDpiAwarenessContextForProcess`: the desktop app
/// runs per-monitor v2, because Tauri's runtime opts in as it starts; the CLI
/// and every test binary run unaware. So rather than trust the process, every
/// entry point here sets the thread's awareness for itself and puts it back
/// after.
///
/// Per-monitor v2 needs Windows 10 1703. Nothing older is still supported by
/// Microsoft, nor by current WebView2.
pub(crate) struct PhysicalPixels(DPI_AWARENESS_CONTEXT);

impl PhysicalPixels {
    pub(crate) fn enter() -> Result<Self, String> {
        let previous =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        if previous.0.is_null() {
            return Err("this version of Windows cannot report screen coordinates in real \
                        pixels; screen capture needs Windows 10 (1703) or later"
                .into());
        }
        Ok(Self(previous))
    }
}

impl Drop for PhysicalPixels {
    fn drop(&mut self) {
        unsafe { SetThreadDpiAwarenessContext(self.0) };
    }
}

/// Screen capture through GDI.
///
/// Holds nothing: a screen DC is cheap to get and must not be held across
/// calls, since the thread's DPI awareness decides what it measures in.
pub struct WindowsCapturer {
    _private: (),
}

impl WindowsCapturer {
    pub fn new() -> Result<Self, String> {
        Ok(Self { _private: () })
    }
}

unsafe extern "system" fn collect(monitor: HMONITOR, _: HDC, _: *mut RECT, list: LPARAM) -> BOOL {
    // SAFETY: `list` is the &mut Vec passed to EnumDisplayMonitors below, which
    // outlives the synchronous enumeration.
    let list = unsafe { &mut *(list.0 as *mut Vec<HMONITOR>) };
    list.push(monitor);
    true.into()
}

fn enumerate() -> Result<Vec<Monitor>, String> {
    let mut handles: Vec<HMONITOR> = Vec::new();
    let ok = unsafe {
        EnumDisplayMonitors(None, None, Some(collect), LPARAM(&mut handles as *mut _ as isize))
    };
    if !ok.as_bool() {
        return Err(format!(
            "Windows would not list the screens: {}",
            windows::core::Error::from_win32()
        ));
    }

    let mut out = Vec::with_capacity(handles.len());
    for handle in handles {
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        let ok = unsafe { GetMonitorInfoW(handle, &mut info as *mut _ as *mut MONITORINFO) };
        // A monitor unplugged between the two calls; the others still count.
        if !ok.as_bool() {
            continue;
        }
        let r = info.monitorInfo.rcMonitor;
        let len = info.szDevice.iter().position(|&c| c == 0).unwrap_or(info.szDevice.len());
        out.push(Monitor {
            name: String::from_utf16_lossy(&info.szDevice[..len]),
            bounds: Rect::new(r.left, r.top, (r.right - r.left) as u32, (r.bottom - r.top) as u32),
            primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
        });
    }
    if out.is_empty() {
        return Err("Windows reported no screens".into());
    }
    Ok(out)
}

/// The screen's device context, released on drop.
struct ScreenDc(HDC);

impl Drop for ScreenDc {
    fn drop(&mut self) {
        unsafe { ReleaseDC(None, self.0) };
    }
}

/// A memory DC, deleted on drop.
struct MemoryDc(HDC);

impl Drop for MemoryDc {
    fn drop(&mut self) {
        let _ = unsafe { DeleteDC(self.0) };
    }
}

/// A bitmap, deleted on drop. GDI will not delete one that is still selected
/// into a DC, so `grab` deselects it as soon as the copy is done.
struct Bitmap(HBITMAP);

impl Drop for Bitmap {
    fn drop(&mut self) {
        let _ = unsafe { DeleteObject(self.0.into()) };
    }
}

impl Capturer for WindowsCapturer {
    /// Every monitor, in physical pixels, positioned in the virtual desktop.
    /// `rcMonitor` is already global and goes negative left of or above the
    /// primary, which Windows always places at the origin.
    fn monitors(&self) -> Result<Vec<Monitor>, String> {
        let _px = PhysicalPixels::enter()?;
        enumerate()
    }

    fn grab(&self, region: &Rect) -> Result<Frame, String> {
        let _px = PhysicalPixels::enter()?;
        let desktop = virtual_bounds(&enumerate()?).ok_or("Windows reported no screens")?;
        let region = region
            .clamped_to(&desktop)
            .ok_or_else(|| format!("region {region} lies outside the {desktop} desktop"))?;
        let (w, h) = (region.width as i32, region.height as i32);

        unsafe {
            let screen = GetDC(None);
            if screen.is_invalid() {
                return Err("could not open the screen for reading".into());
            }
            let screen = ScreenDc(screen);
            let memory = MemoryDc(CreateCompatibleDC(Some(screen.0)));
            if memory.0.is_invalid() {
                return Err("could not allocate a buffer for the capture".into());
            }
            let bitmap = Bitmap(CreateCompatibleBitmap(screen.0, w, h));
            if bitmap.0.is_invalid() {
                return Err(format!("could not allocate a {w}x{h} buffer for the capture"));
            }

            let previous = SelectObject(memory.0, bitmap.0.into());
            // CAPTUREBLT includes layered windows — tooltips, menus, and some
            // apps' whole windows — which are part of what the user sees.
            let copied = BitBlt(
                memory.0, 0, 0, w, h,
                Some(screen.0), region.x, region.y,
                SRCCOPY | CAPTUREBLT,
            );
            // GetDIBits refuses a bitmap that is still selected into a DC.
            SelectObject(memory.0, previous);
            if let Err(e) = copied {
                // Seen when the secure desktop is up: the lock screen, or a UAC
                // prompt. Neither can be read by an ordinary process.
                return Err(format!(
                    "Windows would not let inklift read the screen ({e}). This happens \
                     while the lock screen or an administrator prompt is showing; try \
                     again once it has gone."
                ));
            }

            // Negative height asks for top-down rows; positive would hand back a
            // bottom-up DIB and the capture would be upside down. 32 bits per
            // pixel with BI_RGB is blue, green, red, padding: exactly BGRX.
            let mut info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            // 32-bit rows are always DWORD-aligned, so the stride is exact.
            let stride = w as usize * 4;
            let mut data = vec![0u8; stride * h as usize];
            let rows = GetDIBits(
                memory.0, bitmap.0, 0, h as u32,
                Some(data.as_mut_ptr().cast()), &mut info, DIB_RGB_COLORS,
            );
            if rows != h {
                return Err(format!("the capture came back with {rows} of {h} rows"));
            }
            Frame::with_stride(region.width, region.height, stride, data)
        }
    }
}
