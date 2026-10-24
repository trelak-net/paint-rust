use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
};

pub type Pixel = [u8; 4];
pub const MAX_PIXELS: usize = 4096 * 4096;
const HISTORY_BYTES: usize = 128 * 1024 * 1024;

#[derive(Clone, PartialEq)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub opacity: u8,
    pixels: Arc<Vec<Pixel>>,
}

impl Layer {
    pub fn new(name: String, pixels: Vec<Pixel>) -> Self {
        Self {
            name,
            visible: true,
            opacity: 255,
            pixels: Arc::new(pixels),
        }
    }
    pub fn pixels(&self) -> &[Pixel] {
        &self.pixels
    }
}

#[derive(Clone)]
struct Snapshot {
    layers: Vec<Layer>,
    active: usize,
    revision: u64,
    label: String,
}

pub struct Document {
    pub width: u32,
    pub height: u32,
    layers: Vec<Layer>,
    active: usize,
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
        Self::from_layers(
            width,
            height,
            vec![Layer::new("Background".into(), pixels)],
            0,
        )
    }

    pub fn from_layers(
        width: u32,
        height: u32,
        layers: Vec<Layer>,
        active: usize,
    ) -> Result<Self, String> {
        Self::validate_size(width, height)?;
        let size = width as usize * height as usize;
        if layers.is_empty()
            || layers.len() > 32
            || active >= layers.len()
            || layers.len() * size * 4 > 256 * 1024 * 1024
            || layers
                .iter()
                .any(|l| l.pixels.len() != size || l.name.len() > 4096)
        {
            return Err("Invalid layer data or document memory limit exceeded.".into());
        }
        Ok(Self {
            width,
            height,
            layers,
            active,
            undo: VecDeque::new(),
            redo: Vec::new(),
            pending: None,
            revision: 0,
            next_revision: 1,
            saved_revision: 0,
        })
    }

    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }
    pub fn active_layer(&self) -> usize {
        self.active
    }
    pub fn select_layer(&mut self, index: usize) {
        if !self.editing() && index < self.layers.len() {
            self.active = index;
        }
    }
    pub fn can_add_layer(&self) -> bool {
        self.layers.len() < 32
            && (self.layers.len() + 1) * self.width as usize * self.height as usize * 4
                <= 256 * 1024 * 1024
    }
    pub fn add_layer(&mut self, duplicate: bool) -> bool {
        if self.editing() || !self.can_add_layer() {
            return false;
        }
        self.begin(if duplicate {
            "Duplicate layer"
        } else {
            "Add layer"
        });
        let layer = if duplicate {
            let mut layer = self.layers[self.active].clone();
            while layer.name.len() > 4091 {
                layer.name.pop();
            }
            layer.name.push_str(" copy");
            layer
        } else {
            Layer {
                name: format!("Layer {}", self.layers.len() + 1),
                visible: true,
                opacity: 255,
                pixels: Arc::new(vec![[0; 4]; self.width as usize * self.height as usize]),
            }
        };
        self.active += 1;
        self.layers.insert(self.active, layer);
        self.commit()
    }
    pub fn remove_layer(&mut self) -> bool {
        if self.editing() || self.layers.len() == 1 {
            return false;
        }
        self.begin("Delete layer");
        self.layers.remove(self.active);
        self.active = self.active.min(self.layers.len() - 1);
        self.commit()
    }
    pub fn move_layer(&mut self, up: bool) -> bool {
        let next = if up {
            self.active + 1
        } else {
            self.active.saturating_sub(1)
        };
        if self.editing() || next == self.active || next >= self.layers.len() {
            return false;
        }
        self.begin("Reorder layer");
        self.layers.swap(self.active, next);
        self.active = next;
        self.commit()
    }
    pub fn set_visibility(&mut self, index: usize, visible: bool) {
        if self.editing() || index >= self.layers.len() {
            return;
        }
        self.begin("Layer visibility");
        self.layers[index].visible = visible;
        self.commit();
    }
    pub fn set_layer_properties(&mut self, name: String, opacity: u8) {
        if self.editing() {
            return;
        }
        if name.len() > 4096 {
            return;
        }
        self.begin("Layer properties");
        let layer = &mut self.layers[self.active];
        layer.name = if name.trim().is_empty() {
            "Layer".into()
        } else {
            name
        };
        layer.opacity = opacity;
        self.commit();
    }
    pub fn composite(&self) -> Vec<Pixel> {
        let mut pixels = vec![[0; 4]; self.width as usize * self.height as usize];
        let mut first = true;
        for layer in self.layers.iter().filter(|l| l.visible && l.opacity != 0) {
            if first && layer.opacity == 255 {
                pixels.copy_from_slice(&layer.pixels);
            } else {
                for (dst, src) in pixels.iter_mut().zip(layer.pixels.iter()) {
                    *dst = blend(*dst, *src, layer.opacity);
                }
            }
            first = false;
        }
        pixels
    }
    pub fn is_flat_raster(&self) -> bool {
        self.layers.len() == 1 && self.layers[0].visible && self.layers[0].opacity == 255
    }

    pub fn pixels(&self) -> &[Pixel] {
        &self.layers[self.active].pixels
    }
    pub fn dirty(&self) -> bool {
        self.revision != self.saved_revision
            || self
                .pending
                .as_ref()
                .is_some_and(|s| s.layers != self.layers)
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
        let mut buffers = HashSet::new();
        self.undo
            .iter()
            .chain(self.redo.iter())
            .flat_map(|s| &s.layers)
            .filter(|layer| buffers.insert(Arc::as_ptr(&layer.pixels)))
            .map(|layer| layer.pixels.len() * 4)
            .sum()
    }
    pub fn editing(&self) -> bool {
        self.pending.is_some()
    }

    pub fn begin(&mut self, label: &str) {
        if self.pending.is_none() {
            self.pending = Some(Snapshot {
                layers: self.layers.clone(),
                active: self.active,
                revision: self.revision,
                label: label.into(),
            });
        }
    }

    pub fn commit(&mut self) -> bool {
        let Some(before) = self.pending.take() else {
            return false;
        };
        if before.layers == self.layers {
            return false;
        }
        self.undo.push_back(before);
        self.redo.clear();
        self.revision = self.next_revision;
        self.next_revision += 1;
        self.trim_history();
        true
    }

    fn trim_history(&mut self) {
        while self.history_bytes() > HISTORY_BYTES {
            if self.undo.pop_front().is_none() {
                self.redo.remove(0);
            }
        }
    }

    pub fn cancel(&mut self) {
        if let Some(before) = self.pending.take() {
            self.layers = before.layers;
            self.active = before.active;
        }
    }

    pub fn undo(&mut self) {
        self.cancel();
        if let Some(previous) = self.undo.pop_back() {
            self.redo.push(Snapshot {
                layers: std::mem::replace(&mut self.layers, previous.layers),
                active: std::mem::replace(&mut self.active, previous.active),
                revision: self.revision,
                label: previous.label,
            });
            self.revision = previous.revision;
            self.trim_history();
        }
    }

    pub fn redo(&mut self) {
        self.cancel();
        if let Some(next) = self.redo.pop() {
            self.undo.push_back(Snapshot {
                layers: std::mem::replace(&mut self.layers, next.layers),
                active: std::mem::replace(&mut self.active, next.active),
                revision: self.revision,
                label: next.label,
            });
            self.revision = next.revision;
            self.trim_history();
        }
    }

    pub fn sample(&self, x: i32, y: i32) -> Option<Pixel> {
        self.index(x, y).map(|i| {
            self.layers
                .iter()
                .filter(|l| l.visible)
                .fold([0; 4], |dst, layer| {
                    blend(dst, layer.pixels[i], layer.opacity)
                })
        })
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
        let pixels = Arc::make_mut(&mut self.layers[self.active].pixels);
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
                    pixels[i] = if erase { [0; 4] } else { color };
                }
            }
        }
    }

    pub fn fill(&mut self, x: i32, y: i32, color: Pixel) {
        assert!(self.editing(), "Begin a transaction before filling");
        let Some(start) = self.index(x, y) else {
            return;
        };
        let pixels = Arc::make_mut(&mut self.layers[self.active].pixels);
        let target = pixels[start];
        if target == color {
            return;
        }
        let width = self.width as usize;
        let mut stack = vec![start];
        pixels[start] = color;
        while let Some(i) = stack.pop() {
            let mut visit = |n: usize| {
                if pixels[n] == target {
                    pixels[n] = color;
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
        for pixel in Arc::make_mut(&mut self.layers[self.active].pixels) {
            for c in &mut pixel[..3] {
                *c = 255 - *c;
            }
        }
        self.commit();
    }
}

fn blend(dst: Pixel, src: Pixel, opacity: u8) -> Pixel {
    let sa = u32::from(src[3]) * u32::from(opacity);
    if sa == 0 {
        return dst;
    }
    if sa == 65025 {
        return src;
    }
    let da = u32::from(dst[3]);
    let alpha = sa * 255 + da * (65025 - sa);
    let mut result = [0; 4];
    for c in 0..3 {
        result[c] =
            ((u32::from(src[c]) * sa * 255 + u32::from(dst[c]) * da * (65025 - sa) + alpha / 2)
                / alpha) as u8;
    }
    result[3] = ((alpha + 32512) / 65025) as u8;
    result
}
