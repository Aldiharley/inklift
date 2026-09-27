//! The interactive region picker.
//!
//! Deliberately thin. Every decision lives in [`SelectionState`]; this module
//! only opens a window, blits pixels, and forwards events. That keeps the part
//! which cannot be unit-tested down to "does a window appear and do clicks
//! reach it".

use std::num::NonZeroU32;
use std::rc::Rc;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowLevel};

use crate::frame::Frame;
use crate::geometry::Rect;
use crate::selection::{Outcome, SelectionState};

/// How much the unselected area is darkened. Low enough to still read the
/// screen underneath, dark enough that the selection stands out.
const DIM: u32 = 45; // percent of original brightness
const BORDER: i32 = 1;
const BORDER_COLOR: u32 = 0x00_4d_7c_ff;

struct Overlay {
    frame: Frame,
    bounds: Rect,
    state: SelectionState,
    dimmed: Vec<u32>,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    cursor: (i32, i32),
}

impl Overlay {
    fn new(frame: Frame, bounds: Rect, min: (u32, u32)) -> Self {
        // Pre-dim once. Redrawing then only has to brighten the selection,
        // which keeps dragging smooth on a large desktop.
        let (w, h) = (frame.width(), frame.height());
        let mut dimmed = Vec::with_capacity(w as usize * h as usize);
        for y in 0..h {
            for x in 0..w {
                let [r, g, b] = frame.pixel(x, y);
                let d = |c: u8| (c as u32 * DIM / 100) & 0xff;
                dimmed.push((d(r) << 16) | (d(g) << 8) | d(b));
            }
        }
        Self {
            state: SelectionState::new(bounds, min.0, min.1),
            frame,
            bounds,
            dimmed,
            window: None,
            surface: None,
            cursor: (0, 0),
        }
    }

    /// Window coordinates are relative to the overlay; the state machine works
    /// in global screen coordinates.
    fn to_global(&self, x: f64, y: f64) -> (i32, i32) {
        (self.bounds.x + x as i32, self.bounds.y + y as i32)
    }

    fn redraw(&mut self) {
        let Some(surface) = self.surface.as_mut() else { return };
        let (w, h) = (self.frame.width(), self.frame.height());
        let Ok(mut buffer) = surface.buffer_mut() else { return };
        buffer.copy_from_slice(&self.dimmed);

        if let Some(sel) = self.state.current() {
            let local = sel.relative_to(&self.bounds);
            for y in local.y.max(0)..local.bottom().min(h as i32) {
                for x in local.x.max(0)..local.right().min(w as i32) {
                    let [r, g, b] = self.frame.pixel(x as u32, y as u32);
                    buffer[y as usize * w as usize + x as usize] =
                        ((r as u32) << 16) | ((g as u32) << 8) | b as u32;
                }
            }
            draw_border(&mut buffer, w as i32, h as i32, &local);
        }
        let _ = buffer.present();
    }

    fn finish(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.exit();
    }
}

fn draw_border(buffer: &mut [u32], w: i32, h: i32, r: &Rect) {
    let mut put = |x: i32, y: i32| {
        if x >= 0 && y >= 0 && x < w && y < h {
            buffer[y as usize * w as usize + x as usize] = BORDER_COLOR;
        }
    };
    for t in 0..BORDER {
        for x in r.x..r.right() {
            put(x, r.y - 1 - t);
            put(x, r.bottom() + t);
        }
        for y in r.y..r.bottom() {
            put(r.x - 1 - t, y);
            put(r.right() + t, y);
        }
    }
}

impl ApplicationHandler for Overlay {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("inklift - select a region")
            .with_decorations(false)
            .with_transparent(false)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_inner_size(winit::dpi::PhysicalSize::new(
                self.frame.width(),
                self.frame.height(),
            ))
            .with_position(winit::dpi::PhysicalPosition::new(self.bounds.x, self.bounds.y));

        let Ok(window) = event_loop.create_window(attrs) else {
            event_loop.exit();
            return;
        };
        let window = Rc::new(window);
        window.set_cursor(winit::window::Cursor::Icon(winit::window::CursorIcon::Crosshair));

        if let Ok(context) = softbuffer::Context::new(window.clone()) {
            if let Ok(mut surface) = softbuffer::Surface::new(&context, window.clone()) {
                if let (Some(w), Some(h)) = (
                    NonZeroU32::new(self.frame.width()),
                    NonZeroU32::new(self.frame.height()),
                ) {
                    let _ = surface.resize(w, h);
                }
                self.surface = Some(surface);
            }
        }
        self.window = Some(window);
        self.redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: winit::window::WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.state.cancel();
                self.finish(event_loop);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed
                    && matches!(event.logical_key, Key::Named(NamedKey::Escape))
                {
                    self.state.cancel();
                    self.finish(event_loop);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = self.to_global(position.x, position.y);
                self.state.drag(self.cursor.0, self.cursor.1);
                self.redraw();
            }
            WindowEvent::MouseInput { state, button, .. } => match (button, state) {
                (MouseButton::Left, ElementState::Pressed) => {
                    self.state.press(self.cursor.0, self.cursor.1);
                    self.redraw();
                }
                (MouseButton::Left, ElementState::Released) => {
                    self.state.release();
                    self.finish(event_loop);
                }
                (MouseButton::Right, ElementState::Pressed) => {
                    self.state.cancel();
                    self.finish(event_loop);
                }
                _ => {}
            },
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }
}

/// Show `frame` full-screen and let the user drag out a region.
///
/// `frame` must already be captured: showing the overlay first would put the
/// overlay itself into the picture.
pub fn pick_region(frame: Frame, bounds: Rect, min: (u32, u32)) -> Result<Outcome, String> {
    let event_loop = EventLoop::new().map_err(|e| format!("cannot start an event loop: {e}"))?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut overlay = Overlay::new(frame, bounds, min);
    event_loop
        .run_app(&mut overlay)
        .map_err(|e| format!("overlay failed: {e}"))?;
    Ok(overlay.state.outcome())
}
