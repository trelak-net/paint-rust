use std::collections::VecDeque;

pub type Pixel = [u8; 4];
pub const MAX_PIXELS: usize = 4096 * 4096;
const HISTORY_BYTES: usize = 128 * 1024 * 1024;

#[derive(Clone)]
struct Snapshot {
    pixels: Vec<Pixel>,
    revision: u64,
    label: String,
}

pub struct Document {
    pub width: u32,
    pub height: u32,
    pixels: Vec<Pixel>,
    undo: VecDeque<Snapshot>,
    redo: Vec<Snapshot>,
    pending: Option<Snapshot>,
    revision: u64,
    next_revision: u64,
    saved_revision: u64,
}

impl Document {
    pub fn new(width: u32, height: u32, background: Pixel) -> Result<Self, String> {
        Self::validate_size(width, height)?;
        Self::from_pixels(
            width,
            height,
            vec![background; width as usize * height as usize],
        )
    }

    pub fn validate_size(width: u32, height: u32) -> Result<(), String> {
        if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_PIXELS as u64 {
            return Err("Image must contain between 1 and 16,777,216 pixels.".into());
        }
        Ok(())
    }

    pub fn from_pixels(width: u32, height: u32, pixels: Vec<Pixel>) -> Result<Self, String> {
        Self::validate_size(width, height)?;
        if pixels.len() != width as usize * height as usize {
            return Err("Pixel buffer does not match image dimensions.".into());
        }
        Ok(Self {
            width,
            height,
            pixels,
            undo: VecDeque::new(),
            redo: Vec::new(),
            pending: None,
            revision: 0,
            next_revision: 1,
            saved_revision: 0,
        })
    }

    pub fn pixels(&self) -> &[Pixel] {
        &self.pixels
    }
    pub fn dirty(&self) -> bool {
        self.revision != self.saved_revision
            || self
                .pending
                .as_ref()
                .is_some_and(|s| s.pixels != self.pixels)
    }
    pub fn mark_saved(&mut self) {
        self.saved_revision = self.revision;
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn history(&self) -> impl Iterator<Item = &str> {
        self.undo.iter().rev().map(|s| s.label.as_str())
    }
    pub fn history_bytes(&self) -> usize {
        (self.undo.len() + self.redo.len()) * self.pixels.len() * 4
    }
    pub fn editing(&self) -> bool {
        self.pending.is_some()
    }

    pub fn begin(&mut self, label: &str) {
        if self.pending.is_none() {
            self.pending = Some(Snapshot {
                pixels: self.pixels.clone(),
                revision: self.revision,
                label: label.into(),
            });
        }
    }

    pub fn commit(&mut self) -> bool {
        let Some(before) = self.pending.take() else {
            return false;
        };
        if before.pixels == self.pixels {
            return false;
        }
        self.undo.push_back(before);
        self.redo.clear();
        self.revision = self.next_revision;
        self.next_revision += 1;
        while self.history_bytes() > HISTORY_BYTES {
            self.undo.pop_front();
        }
        true
    }

    pub fn cancel(&mut self) {
        if let Some(before) = self.pending.take() {
            self.pixels = before.pixels;
        }
    }

    pub fn undo(&mut self) {
        self.cancel();
        if let Some(previous) = self.undo.pop_back() {
            self.redo.push(Snapshot {
                pixels: std::mem::replace(&mut self.pixels, previous.pixels),
                revision: self.revision,
                label: previous.label,
            });
            self.revision = previous.revision;
        }
    }

    pub fn redo(&mut self) {
        self.cancel();
        if let Some(next) = self.redo.pop() {
            self.undo.push_back(Snapshot {
                pixels: std::mem::replace(&mut self.pixels, next.pixels),
                revision: self.revision,
                label: next.label,
            });
            self.revision = next.revision;
        }
    }

    pub fn sample(&self, x: i32, y: i32) -> Option<Pixel> {
        self.index(x, y).map(|i| self.pixels[i])
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.width as i32 && y < self.height as i32)
            .then_some(y.max(0) as usize * self.width as usize + x.max(0) as usize)
    }

    pub fn stroke(
        &mut self,
        from: (f32, f32),
        to: (f32, f32),
        diameter: f32,
        color: Pixel,
        erase: bool,
    ) {
        assert!(self.editing(), "Begin a transaction before drawing");
        let radius = diameter.max(1.0) / 2.0;
        let min_x = ((from.0.min(to.0) - radius).floor() as i32).max(0);
        let max_x = ((from.0.max(to.0) + radius).ceil() as i32).min(self.width as i32 - 1);
        let min_y = ((from.1.min(to.1) - radius).floor() as i32).max(0);
        let max_y = ((from.1.max(to.1) + radius).ceil() as i32).min(self.height as i32 - 1);
        let dx = to.0 - from.0;
        let dy = to.1 - from.1;
        let length_sq = dx * dx + dy * dy;
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let t = if length_sq > 0.0 {
                    ((px - from.0) * dx + (py - from.1) * dy) / length_sq
                } else {
                    0.0
                }
                .clamp(0.0, 1.0);
                if (px - from.0 - t * dx).powi(2) + (py - from.1 - t * dy).powi(2)
                    <= radius * radius
                {
                    let i = y as usize * self.width as usize + x as usize;
                    self.pixels[i] = if erase { [0; 4] } else { color };
                }
            }
        }
    }

    pub fn fill(&mut self, x: i32, y: i32, color: Pixel) {
        assert!(self.editing(), "Begin a transaction before filling");
        let Some(start) = self.index(x, y) else {
            return;
        };
        let target = self.pixels[start];
        if target == color {
            return;
        }
        let width = self.width as usize;
        let mut stack = vec![start];
        self.pixels[start] = color;
        while let Some(i) = stack.pop() {
            let mut visit = |n: usize| {
                if self.pixels[n] == target {
                    self.pixels[n] = color;
                    stack.push(n);
                }
            };
            if i % width > 0 {
                visit(i - 1);
            }
            if i % width + 1 < width {
                visit(i + 1);
            }
            if i >= width {
                visit(i - width);
            }
            if i + width < self.width as usize * self.height as usize {
                visit(i + width);
            }
        }
    }

    pub fn invert(&mut self) {
        self.begin("Invert colors");
        for pixel in &mut self.pixels {
            for c in &mut pixel[..3] {
                *c = 255 - *c;
            }
        }
        self.commit();
    }
}
