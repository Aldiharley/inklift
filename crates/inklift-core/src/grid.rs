/// A single-channel image plane: row-major `f32`, nominally in `[0, 1]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    width: usize,
    height: usize,
    data: Vec<f32>,
}

impl Grid {
    pub fn new(width: usize, height: usize) -> Self {
        Self::filled(width, height, 0.0)
    }

    pub fn filled(width: usize, height: usize, value: f32) -> Self {
        Self { width, height, data: vec![value; width * height] }
    }

    /// Wrap an existing row-major buffer.
    ///
    /// # Panics
    /// If `data.len() != width * height`.
    pub fn from_vec(width: usize, height: usize, data: Vec<f32>) -> Self {
        assert_eq!(data.len(), width * height, "buffer does not match dimensions");
        Self { width, height, data }
    }

    #[inline]
    pub fn width(&self) -> usize {
        self.width
    }

    #[inline]
    pub fn height(&self) -> usize {
        self.height
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> f32 {
        self.data[y * self.width + x]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, value: f32) {
        self.data[y * self.width + x] = value;
    }

    #[inline]
    pub fn data(&self) -> &[f32] {
        &self.data
    }

    #[inline]
    pub fn data_mut(&mut self) -> &mut [f32] {
        &mut self.data
    }

    /// Apply `f` to every sample in place.
    pub fn map_in_place(&mut self, mut f: impl FnMut(f32) -> f32) {
        for v in &mut self.data {
            *v = f(*v);
        }
    }

    /// A new grid of the same shape, each sample derived from this one.
    pub fn map(&self, f: impl Fn(f32) -> f32) -> Grid {
        Grid {
            width: self.width,
            height: self.height,
            data: self.data.iter().map(|&v| f(v)).collect(),
        }
    }

    /// True when `other` has the same dimensions.
    pub fn same_shape(&self, other: &Grid) -> bool {
        self.width == other.width && self.height == other.height
    }
}
