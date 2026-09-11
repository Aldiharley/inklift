/// A binary ink/paper decision per pixel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mask {
    width: usize,
    height: usize,
    data: Vec<bool>,
}

impl Mask {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, data: vec![false; width * height] }
    }

    /// # Panics
    /// If `data.len() != width * height`.
    pub fn from_vec(width: usize, height: usize, data: Vec<bool>) -> Self {
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
    pub fn get(&self, x: usize, y: usize) -> bool {
        self.data[y * self.width + x]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, value: bool) {
        self.data[y * self.width + x] = value;
    }

    #[inline]
    pub fn data(&self) -> &[bool] {
        &self.data
    }

    #[inline]
    pub fn data_mut(&mut self) -> &mut [bool] {
        &mut self.data
    }

    /// True when `other` has the same dimensions.
    pub fn same_shape_as(&self, other: &Mask) -> bool {
        self.width == other.width && self.height == other.height
    }

    /// How many pixels are marked as ink.
    pub fn count(&self) -> usize {
        self.data.iter().filter(|&&b| b).count()
    }

    /// Fraction of the image marked as ink, 0.0..=1.0.
    pub fn coverage(&self) -> f32 {
        if self.data.is_empty() {
            return 0.0;
        }
        self.count() as f32 / self.data.len() as f32
    }
}
