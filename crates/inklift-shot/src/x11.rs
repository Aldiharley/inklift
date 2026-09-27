use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as RandrExt;
use x11rb::protocol::xproto::{ConnectionExt, ImageFormat, Window};
use x11rb::rust_connection::RustConnection;

use crate::capture::{Capturer, Monitor};
use crate::frame::Frame;
use crate::geometry::Rect;

/// Screen capture over the X11 protocol.
///
/// Speaks the wire protocol directly through `x11rb`, so it needs no C
/// libraries, no pkg-config and no build script.
pub struct X11Capturer {
    conn: RustConnection,
    root: Window,
    root_bounds: Rect,
}

impl X11Capturer {
    pub fn new() -> Result<Self, String> {
        let (conn, screen_num) =
            x11rb::connect(None).map_err(|e| format!("cannot reach the X server: {e}"))?;
        let screen = &conn.setup().roots[screen_num];
        let root_bounds = Rect::new(
            0,
            0,
            screen.width_in_pixels as u32,
            screen.height_in_pixels as u32,
        );
        let root = screen.root;
        Ok(Self { conn, root, root_bounds })
    }

    /// The whole virtual desktop, in root coordinates.
    pub fn root_bounds(&self) -> Rect {
        self.root_bounds
    }
}

impl Capturer for X11Capturer {
    /// Enumerate via RandR 1.5, falling back to treating the root window as a
    /// single screen when RandR is unavailable or reports nothing.
    fn monitors(&self) -> Result<Vec<Monitor>, String> {
        let listed = self
            .conn
            .randr_get_monitors(self.root, true)
            .ok()
            .and_then(|c| c.reply().ok());

        if let Some(reply) = listed {
            let mut out = Vec::new();
            for m in reply.monitors {
                let name = self
                    .conn
                    .get_atom_name(m.name)
                    .ok()
                    .and_then(|c| c.reply().ok())
                    .map(|r| String::from_utf8_lossy(&r.name).into_owned())
                    .unwrap_or_else(|| "screen".into());
                out.push(Monitor {
                    name,
                    bounds: Rect::new(m.x as i32, m.y as i32, m.width as u32, m.height as u32),
                    primary: m.primary,
                });
            }
            if !out.is_empty() {
                return Ok(out);
            }
        }

        Ok(vec![Monitor {
            name: "root".into(),
            bounds: self.root_bounds,
            primary: true,
        }])
    }

    fn grab(&self, region: &Rect) -> Result<Frame, String> {
        let region = region
            .clamped_to(&self.root_bounds)
            .ok_or_else(|| format!("region {region} lies outside the {} desktop", self.root_bounds))?;

        let reply = self
            .conn
            .get_image(
                ImageFormat::Z_PIXMAP,
                self.root,
                region.x as i16,
                region.y as i16,
                region.width as u16,
                region.height as u16,
                !0,
            )
            .map_err(|e| format!("capture request failed: {e}"))?
            .reply()
            .map_err(|e| format!("capture failed: {e}"))?;

        // Fail loudly on an unexpected format rather than returning an image
        // with the colour channels quietly swapped.
        let pixels = region.area() as usize;
        if pixels == 0 {
            return Err("capture region is empty".into());
        }
        let bytes_per_pixel = reply.data.len() / pixels;
        if bytes_per_pixel != 4 {
            return Err(format!(
                "expected 4 bytes per pixel at depth 24/32, got {bytes_per_pixel} \
                 (depth {}). This display uses a pixel format inklift does not handle yet.",
                reply.depth
            ));
        }
        if !matches!(reply.depth, 24 | 32) {
            return Err(format!("unsupported colour depth {}", reply.depth));
        }

        Frame::from_bgrx(region.width, region.height, reply.data)
    }
}
