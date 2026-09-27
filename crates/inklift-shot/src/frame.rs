use inklift_core::Grid;

use crate::geometry::Rect;

/// Bytes per pixel in the formats we accept.
const BPP: usize = 4;

/// A captured image, stored as the X server hands it over: BGRX, one row every
/// `stride` bytes.
///
/// Kept in the source byte order rather than converted on capture, so a large
/// grab costs one allocation and cropping stays a copy of the rows that matter.
#[derive(Clone, Debug)]
pub struct Frame {
    width: u32,
    height: u32,
    stride: usize,
    data: Vec<u8>,
}

impl Frame {
    /// Tightly packed BGRX, no row padding.
    pub fn from_bgrx(width: u32, height: u32, data: Vec<u8>) -> Result<Self, String> {
        Self::with_stride(width, height, width as usize * BPP, data)
    }

    /// BGRX where each row starts every `stride` bytes.
    pub fn with_stride(
        width: u32,
        height: u32,
        stride: usize,
        data: Vec<u8>,
    ) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err("a frame needs a non-zero width and height".into());
        }
        if stride < width as usize * BPP {
            return Err(format!(
                "stride {stride} is too small for {width} pixels at {BPP} bytes each"
            ));
        }
        let needed = stride * height as usize;
        if data.len() < needed {
            return Err(format!(
                "buffer holds {} bytes but {width}x{height} at stride {stride} needs {needed}",
                data.len()
            ));
        }
        Ok(Self { width, height, stride, data })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    #[inline]
    fn offset(&self, x: u32, y: u32) -> usize {
        y as usize * self.stride + x as usize * BPP
    }

    /// One pixel as `[r, g, b]`.
    ///
    /// # Panics
    /// If `x` or `y` is outside the frame.
    #[inline]
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 3] {
        assert!(x < self.width && y < self.height, "pixel ({x},{y}) is outside the frame");
        let i = self.offset(x, y);
        // Source order is blue, green, red, padding.
        [self.data[i + 2], self.data[i + 1], self.data[i]]
    }

    /// Copy out a sub-rectangle, in this frame's own coordinates.
    ///
    /// Refuses a rectangle that is not fully inside. Silently returning a
    /// smaller image than asked for would show up much later as a mysteriously
    /// cropped result.
    pub fn crop(&self, rect: &Rect) -> Result<Frame, String> {
        let frame = Rect::new(0, 0, self.width, self.height);
        if !frame.contains(rect) || rect.is_empty() {
            return Err(format!(
                "region {rect} is not inside the {}x{} frame",
                self.width, self.height
            ));
        }
        let (rw, rh) = (rect.width as usize, rect.height as usize);
        let row_bytes = rw * BPP;
        let mut out = vec![0u8; row_bytes * rh];
        for row in 0..rh {
            let src = self.offset(rect.x as u32, rect.y as u32 + row as u32);
            out[row * row_bytes..(row + 1) * row_bytes]
                .copy_from_slice(&self.data[src..src + row_bytes]);
        }
        Frame::from_bgrx(rect.width, rect.height, out)
    }

    /// Hand off to the extractor: three `[0, 1]` planes in R, G, B order.
    pub fn to_planes(&self) -> [Grid; 3] {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut planes = [Grid::new(w, h), Grid::new(w, h), Grid::new(w, h)];
        for y in 0..self.height {
            for x in 0..self.width {
                let [r, g, b] = self.pixel(x, y);
                let i = y as usize * w + x as usize;
                planes[0].data_mut()[i] = r as f32 / 255.0;
                planes[1].data_mut()[i] = g as f32 / 255.0;
                planes[2].data_mut()[i] = b as f32 / 255.0;
            }
        }
        planes
    }

    /// Straight RGBA, fully opaque, for writing a PNG or filling the clipboard.
    pub fn to_rgba8(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.width as usize * self.height as usize * 4);
        for y in 0..self.height {
            for x in 0..self.width {
                let [r, g, b] = self.pixel(x, y);
                out.extend_from_slice(&[r, g, b, 255]);
            }
        }
        out
    }
}

/// Take a globally-positioned region out of a frame captured at `origin`.
///
/// This is what interactive capture must use instead of grabbing the screen a
/// second time: by the time a region has been chosen, the overlay is sitting
/// on top of the screen and a fresh grab photographs the overlay rather than
/// what is underneath it.
pub fn crop_global(frame: &Frame, origin: Rect, region: Rect) -> Result<Frame, String> {
    frame.crop(&region.relative_to(&origin))
}
