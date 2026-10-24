use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
};

pub type Pixel = [u8; 4];

#[derive(Clone, Copy, PartialEq)]
pub enum Shape {
    Line,
    Rectangle,
    Ellipse,
}

pub struct ShapeStyle {
    pub color: Pixel,
    pub width: f32,
    pub filled: bool,
}

pub struct GradientStyle {
    pub start: Pixel,
    pub end: Pixel,
    pub radial: bool,
}

pub const MAX_PIXELS: usize = 4096 * 4096;
const HISTORY_BYTES: usize = 128 * 1024 * 1024;

pub use crate::selection::{EllipseSelection, Selection};

fn selected(selection: Option<&Selection>, x: usize, y: usize) -> bool {
    selection.is_none_or(|rect| rect.contains(x, y))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    Difference,
    Add,
}
impl BlendMode {
    pub const ALL: [Self; 8] = [
        Self::Normal,
        Self::Multiply,
        Self::Screen,
        Self::Overlay,
        Self::Darken,
        Self::Lighten,
        Self::Difference,
        Self::Add,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Multiply => "Multiply",
            Self::Screen => "Screen",
            Self::Overlay => "Overlay",
            Self::Darken => "Darken",
            Self::Lighten => "Lighten",
            Self::Difference => "Difference",
            Self::Add => "Add",
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub opacity: u8,
    pub blend_mode: BlendMode,
    pixels: Arc<Vec<Pixel>>,
}

impl Layer {
    pub fn new(name: String, pixels: Vec<Pixel>) -> Self {
        Self {
            name,
            visible: true,
            opacity: 255,
            blend_mode: BlendMode::Normal,
            pixels: Arc::new(pixels),
        }
    }
    pub fn pixels(&self) -> &[Pixel] {
        &self.pixels
    }
}

#[derive(Clone)]
struct Snapshot {
    width: u32,
    height: u32,
    selection: Option<Selection>,
    layers: Vec<Layer>,
    active: usize,
    revision: u64,
    label: String,
}

pub struct Document {
    pub width: u32,
    pub height: u32,
    selection: Option<Selection>,
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
            selection: None,
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

    pub fn selection(&self) -> Option<Selection> {
        self.selection.clone()
    }
    pub fn set_selection(&mut self, selection: Option<Selection>) {
        if self.editing() {
            return;
        }
        self.selection = selection.and_then(|s| {
            let x = s.x.min(self.width);
            let y = s.y.min(self.height);
            let width = s.width.min(self.width - x);
            let height = s.height.min(self.height - y);
            ((width > 0 && height > 0) || s.mask.is_some()).then_some(Selection {
                ellipse: s.ellipse,
                mask: s.mask,
                x,
                y,
                width,
                height,
            })
        });
    }
    pub fn clear_pixels(&mut self) {
        self.begin("Delete pixels");
        let width = self.width as usize;
        let selection = self.selection.clone();
        for (i, pixel) in Arc::make_mut(&mut self.layers[self.active].pixels)
            .iter_mut()
            .enumerate()
        {
            if selected(selection.as_ref(), i % width, i / width) {
                *pixel = [0; 4];
            }
        }
        self.commit();
    }
    pub fn crop_to_selection(&mut self) -> bool {
        let Some(rect) = self.selection.clone() else {
            return false;
        };
        if self.editing()
            || rect.width == 0
            || rect.height == 0
            || (rect.x == 0
                && rect.y == 0
                && rect.width == self.width
                && rect.height == self.height)
        {
            return false;
        }
        self.begin("Crop to selection");
        for layer in &mut self.layers {
            let mut pixels = Vec::with_capacity(rect.width as usize * rect.height as usize);
            for y in rect.y..rect.y + rect.height {
                let start = y as usize * self.width as usize + rect.x as usize;
                pixels.extend_from_slice(&layer.pixels[start..start + rect.width as usize]);
            }
            layer.pixels = Arc::new(pixels);
        }
        self.width = rect.width;
        self.height = rect.height;
        self.selection = None;
        self.commit()
    }

    /// Publish a computed image as one edit while retaining this document's history.
    pub fn apply_transformed(&mut self, replacement: Self, label: &str) -> Result<bool, String> {
        if self.editing() {
            return Err("Finish the current edit first.".into());
        }
        if self.width == replacement.width
            && self.height == replacement.height
            && self.layers == replacement.layers
        {
            return Ok(false);
        }
        self.begin(label);
        self.width = replacement.width;
        self.height = replacement.height;
        self.layers = replacement.layers;
        self.active = replacement.active;
        self.selection = None;
        Ok(self.commit())
    }

    fn transform_layers(
        &mut self,
        label: &str,
        width: u32,
        height: u32,
        transform: impl Fn(&[Pixel]) -> Vec<Pixel>,
    ) -> Result<bool, String> {
        Self::validate_size(width, height)?;
        if self.editing() {
            return Err("Finish the current edit first.".into());
        }
        if u64::from(width) * u64::from(height) * 4 * self.layers.len() as u64 > 256 * 1024 * 1024 {
            return Err("The resized layers would exceed the 256 MiB document limit.".into());
        }
        self.begin(label);
        for layer in &mut self.layers {
            layer.pixels = Arc::new(transform(&layer.pixels));
        }
        self.width = width;
        self.height = height;
        self.selection = None;
        Ok(self.commit())
    }
    pub fn resize_image(&mut self, width: u32, height: u32, smooth: bool) -> Result<bool, String> {
        if (width, height) == (self.width, self.height) {
            return Ok(false);
        }
        let old = (self.width, self.height);
        self.transform_layers("Resize image", width, height, |pixels| {
            crate::transform::resize(pixels, old, (width, height), smooth)
        })
    }
    pub fn resize_canvas(
        &mut self,
        width: u32,
        height: u32,
        anchor: usize,
    ) -> Result<bool, String> {
        if anchor > 8 {
            return Err("Invalid canvas anchor.".into());
        }
        if (width, height) == (self.width, self.height) {
            return Ok(false);
        }
        let old = (self.width, self.height);
        self.transform_layers("Resize canvas", width, height, |pixels| {
            crate::transform::canvas(pixels, old, (width, height), anchor)
        })
    }
    pub fn reorient(&mut self, operation: crate::transform::Orientation) -> Result<bool, String> {
        use crate::transform::Orientation;
        let (width, height) = (self.width, self.height);
        let new = if matches!(
            operation,
            Orientation::Clockwise | Orientation::Counterclockwise
        ) {
            (height, width)
        } else {
            (width, height)
        };
        let label = match operation {
            Orientation::Clockwise => "Rotate clockwise",
            Orientation::Counterclockwise => "Rotate counterclockwise",
            Orientation::HalfTurn => "Rotate 180 degrees",
            Orientation::FlipHorizontal => "Flip horizontal",
            Orientation::FlipVertical => "Flip vertical",
        };
        self.transform_layers(label, new.0, new.1, |pixels| {
            crate::transform::orient(pixels, width, height, operation)
        })
    }

    pub fn copy_pixels(&self, merged: bool) -> (u32, u32, Vec<Pixel>) {
        let rect = self.selection.clone().unwrap_or(Selection {
            mask: None,
            ellipse: None,
            x: 0,
            y: 0,
            width: self.width,
            height: self.height,
        });
        let flattened;
        let source = if merged {
            flattened = self.composite();
            &flattened[..]
        } else {
            self.pixels()
        };
        let mut pixels = Vec::with_capacity(rect.width as usize * rect.height as usize);
        for y in rect.y..rect.y + rect.height {
            let start = y as usize * self.width as usize + rect.x as usize;
            for x in 0..rect.width as usize {
                pixels.push(if rect.contains(rect.x as usize + x, y as usize) {
                    source[start + x]
                } else {
                    [0; 4]
                });
            }
        }
        (rect.width, rect.height, pixels)
    }
    pub fn paste_layer(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[Pixel],
    ) -> Result<bool, String> {
        self.insert_image_layer(width, height, pixels, "Pasted image", "Paste layer")
    }

    pub fn import_layer(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[Pixel],
        name: &str,
    ) -> Result<bool, String> {
        self.insert_image_layer(width, height, pixels, name, "Import image layer")
    }

    fn insert_image_layer(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[Pixel],
        name: &str,
        action: &str,
    ) -> Result<bool, String> {
        Self::validate_size(width, height)?;
        if name.len() > 4096 {
            return Err("Layer name is too long.".into());
        }
        if pixels.len() != width as usize * height as usize {
            return Err("Invalid image pixel buffer.".into());
        }
        if self.editing() || !self.can_add_layer() {
            return Err("Cannot add another layer to this document.".into());
        }
        let (x, y) = self.selection.as_ref().map(|s| (s.x, s.y)).unwrap_or((
            (self.width.saturating_sub(width)) / 2,
            (self.height.saturating_sub(height)) / 2,
        ));
        let clipped_width = width.min(self.width - x);
        let clipped_height = height.min(self.height - y);
        let mut output = vec![[0; 4]; self.width as usize * self.height as usize];
        for row in 0..clipped_height {
            let src = row as usize * width as usize;
            let dst = (row + y) as usize * self.width as usize + x as usize;
            output[dst..dst + clipped_width as usize]
                .copy_from_slice(&pixels[src..src + clipped_width as usize]);
        }
        self.begin(action);
        self.active += 1;
        let name = if name.trim().is_empty() {
            "Imported image"
        } else {
            name
        };
        self.layers
            .insert(self.active, Layer::new(name.into(), output));
        self.selection = Some(Selection {
            mask: None,
            ellipse: None,
            x,
            y,
            width: clipped_width,
            height: clipped_height,
        });
        Ok(self.commit())
    }

    pub fn snapshot(&self) -> Self {
        Self::from_layers(self.width, self.height, self.layers.clone(), self.active)
            .expect("valid document snapshot")
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn mark_unsaved(&mut self) {
        self.saved_revision = u64::MAX;
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
                blend_mode: BlendMode::Normal,
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
    pub fn can_flatten(&self) -> bool {
        !self.editing()
            && (!self.is_flat_raster() || self.layers[0].blend_mode != BlendMode::Normal)
    }

    pub fn flatten(&mut self) -> bool {
        self.flatten_cancellable(&std::sync::atomic::AtomicBool::new(false))
            .unwrap_or(false)
    }

    pub fn flatten_cancellable(&mut self, cancel: &std::sync::atomic::AtomicBool) -> Option<bool> {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return None;
        }
        if !self.can_flatten() {
            return Some(false);
        }
        let pixels = self.composite_cancellable(cancel)?;
        self.begin("Flatten image");
        self.layers = vec![Layer::new("Flattened image".into(), pixels)];
        self.active = 0;
        Some(self.commit())
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
        self.set_layer_style(name, opacity, self.layers[self.active].blend_mode);
    }
    pub fn set_layer_style(&mut self, name: String, opacity: u8, mode: BlendMode) {
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
        layer.blend_mode = mode;
        self.commit();
    }
    pub fn composite(&self) -> Vec<Pixel> {
        self.composite_cancellable(&std::sync::atomic::AtomicBool::new(false))
            .expect("uncancelled composite")
    }

    pub fn composite_cancellable(
        &self,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Option<Vec<Pixel>> {
        use std::sync::atomic::Ordering;
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let mut pixels = vec![[0; 4]; self.width as usize * self.height as usize];
        let mut first = true;
        for layer in self.layers.iter().filter(|l| l.visible && l.opacity != 0) {
            for (output, source) in pixels.chunks_mut(4096).zip(layer.pixels.chunks(4096)) {
                if cancel.load(Ordering::Relaxed) {
                    return None;
                }
                if first && layer.opacity == 255 {
                    output.copy_from_slice(source);
                } else {
                    for (dst, src) in output.iter_mut().zip(source) {
                        *dst = blend_layer(*dst, *src, layer.opacity, layer.blend_mode);
                    }
                }
            }
            first = false;
        }
        Some(pixels)
    }
    pub fn is_flat_raster(&self) -> bool {
        self.layers.len() == 1 && self.layers[0].visible && self.layers[0].opacity == 255
    }

    pub fn pixels(&self) -> &[Pixel] {
        &self.layers[self.active].pixels
    }
    pub fn dirty(&self) -> bool {
        self.revision != self.saved_revision
            || self.pending.as_ref().is_some_and(|s| {
                s.layers != self.layers || s.width != self.width || s.height != self.height
            })
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
    pub fn redo_history(&self) -> impl Iterator<Item = &str> {
        self.redo
            .iter()
            .rev()
            .map(|snapshot| snapshot.label.as_str())
    }

    pub fn history_bytes(&self) -> usize {
        let mut buffers = HashSet::new();
        let mut masks = HashSet::new();
        let mask_bytes: usize = self
            .undo
            .iter()
            .chain(self.redo.iter())
            .filter_map(|s| s.selection.as_ref().and_then(|s| s.mask.as_ref()))
            .filter(|mask| masks.insert(Arc::as_ptr(&mask.pixels)))
            .map(|mask| mask.pixels.len())
            .sum();
        self.undo
            .iter()
            .chain(self.redo.iter())
            .flat_map(|s| &s.layers)
            .filter(|layer| buffers.insert(Arc::as_ptr(&layer.pixels)))
            .map(|layer| layer.pixels.len() * 4)
            .sum::<usize>()
            + mask_bytes
    }
    pub fn editing(&self) -> bool {
        self.pending.is_some()
    }

    pub fn begin(&mut self, label: &str) {
        if self.pending.is_none() {
            self.pending = Some(Snapshot {
                width: self.width,
                height: self.height,
                selection: self.selection.clone(),
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
        if before.layers == self.layers
            && before.width == self.width
            && before.height == self.height
        {
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
            self.width = before.width;
            self.height = before.height;
            self.selection = before.selection.clone();
            self.layers = before.layers;
            self.active = before.active;
        }
    }

    pub fn undo(&mut self) {
        self.cancel();
        if let Some(previous) = self.undo.pop_back() {
            self.redo.push(Snapshot {
                width: std::mem::replace(&mut self.width, previous.width),
                height: std::mem::replace(&mut self.height, previous.height),
                selection: std::mem::replace(&mut self.selection, previous.selection),
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
                width: std::mem::replace(&mut self.width, next.width),
                height: std::mem::replace(&mut self.height, next.height),
                selection: std::mem::replace(&mut self.selection, next.selection),
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
                    blend_layer(dst, layer.pixels[i], layer.opacity, layer.blend_mode)
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
        let selection = self.selection.clone();
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
                if !selected(selection.as_ref(), x as usize, y as usize) {
                    continue;
                }
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

    pub fn shared_pixels(&self) -> Arc<Vec<Pixel>> {
        self.layers[self.active].pixels.clone()
    }
    pub fn publish_preview(&mut self, pixels: Vec<Pixel>) -> Result<(), String> {
        if pixels.len() != self.width as usize * self.height as usize {
            return Err("Invalid preview dimensions.".into());
        }
        let before = self
            .pending
            .as_ref()
            .ok_or("No active preview transaction")?;
        self.layers = before.layers.clone();
        self.active = before.active;
        self.layers[self.active].pixels = Arc::new(pixels);
        Ok(())
    }

    pub fn preview_gradient(&mut self, from: (f32, f32), to: (f32, f32), style: GradientStyle) {
        let before = self
            .pending
            .as_ref()
            .expect("Begin a gradient transaction first");
        self.layers = before.layers.clone();
        self.active = before.active;
        let selection = self.selection.clone();
        let output = Arc::make_mut(&mut self.layers[self.active].pixels);
        let (dx, dy) = (f64::from(to.0 - from.0), f64::from(to.1 - from.1));
        let distance = dx * dx + dy * dy;
        for y in 0..self.height {
            for x in 0..self.width {
                if !selected(selection.as_ref(), x as usize, y as usize) {
                    continue;
                }
                let (px, py) = (
                    f64::from(x) + 0.5 - f64::from(from.0),
                    f64::from(y) + 0.5 - f64::from(from.1),
                );
                let t = if distance <= f64::EPSILON {
                    0.0
                } else if style.radial {
                    (px * px + py * py).sqrt() / distance.sqrt()
                } else {
                    (px * dx + py * dy) / distance
                };
                let t = t.clamp(0.0, 1.0);
                let a = f64::from(style.start[3]) * (1.0 - t);
                let b = f64::from(style.end[3]) * t;
                let mut color = [0; 4];
                let alpha = a + b;
                if alpha > 0.0 {
                    for (c, channel) in color[..3].iter_mut().enumerate() {
                        *channel = ((f64::from(style.start[c]) * a + f64::from(style.end[c]) * b)
                            / alpha)
                            .round()
                            .clamp(0.0, 255.0) as u8;
                    }
                    color[3] = alpha.round().clamp(0.0, 255.0) as u8;
                }
                let i = y as usize * self.width as usize + x as usize;
                output[i] = blend(output[i], color, 255);
            }
        }
    }

    pub fn preview_move(&mut self, dx: i32, dy: i32, copy: bool, selection_only: bool) {
        let before = self
            .pending
            .as_ref()
            .expect("Begin a move transaction first");
        self.layers = before.layers.clone();
        self.active = before.active;
        self.selection = before.selection.clone();
        if dx == 0 && dy == 0 {
            return;
        }
        let area = before.selection.clone().unwrap_or(Selection {
            mask: None,
            ellipse: None,
            x: 0,
            y: 0,
            width: self.width,
            height: self.height,
        });
        if area.width == 0 || area.height == 0 {
            return;
        }
        let left = i64::from(area.x) + i64::from(dx);
        let top = i64::from(area.y) + i64::from(dy);
        let x = left.clamp(0, i64::from(self.width)) as u32;
        let y = top.clamp(0, i64::from(self.height)) as u32;
        let right = (left + i64::from(area.width)).clamp(0, i64::from(self.width)) as u32;
        let bottom = (top + i64::from(area.height)).clamp(0, i64::from(self.height)) as u32;
        self.selection = (right > x && bottom > y).then_some(Selection {
            mask: area.mask.clone().map(|mut mask| {
                mask.x = mask.x.saturating_add(i64::from(dx));
                mask.y = mask.y.saturating_add(i64::from(dy));
                mask
            }),
            ellipse: area.ellipse.map(|mut ellipse| {
                ellipse.x = ellipse.x.saturating_add(i64::from(dx));
                ellipse.y = ellipse.y.saturating_add(i64::from(dy));
                ellipse
            }),
            x,
            y,
            width: right - x,
            height: bottom - y,
        });
        if selection_only {
            return;
        }
        let source = before.layers[before.active].pixels.clone();
        let output = Arc::make_mut(&mut self.layers[self.active].pixels);
        if !copy {
            for y in area.y..area.y + area.height {
                let start = y as usize * self.width as usize + area.x as usize;
                for x in 0..area.width as usize {
                    if area.contains(area.x as usize + x, y as usize) {
                        output[start + x] = [0; 4];
                    }
                }
            }
        }
        for sy in area.y..area.y + area.height {
            for sx in area.x..area.x + area.width {
                if !area.contains(sx as usize, sy as usize) {
                    continue;
                }
                let x = i64::from(sx) + i64::from(dx);
                let y = i64::from(sy) + i64::from(dy);
                if x < 0 || y < 0 || x >= i64::from(self.width) || y >= i64::from(self.height) {
                    continue;
                }
                let dst = y as usize * self.width as usize + x as usize;
                output[dst] = blend(
                    output[dst],
                    source[sy as usize * self.width as usize + sx as usize],
                    255,
                );
            }
        }
    }

    pub fn preview_text(&mut self, style: crate::text::TextStyle<'_>) -> Result<(), String> {
        if !style.size.is_finite()
            || style.size < 1.0
            || style.size > 512.0
            || style.text.len() > 16384
        {
            return Err("Text exceeds the supported size or length.".into());
        }
        let before = self
            .pending
            .as_ref()
            .ok_or("Begin a text transaction first")?;
        self.layers = before.layers.clone();
        self.active = before.active;
        let pixels = Arc::make_mut(&mut self.layers[self.active].pixels);
        crate::text::render(
            pixels,
            self.width,
            self.height,
            self.selection.clone(),
            style,
        );
        Ok(())
    }

    pub fn preview_shape(
        &mut self,
        shape: Shape,
        from: (f32, f32),
        to: (f32, f32),
        style: ShapeStyle,
    ) {
        let before = self
            .pending
            .as_ref()
            .expect("Begin a transaction before previewing");
        self.layers = before.layers.clone();
        self.active = before.active;
        if shape == Shape::Line || (from.0 - to.0).abs() < 1.0 || (from.1 - to.1).abs() < 1.0 {
            self.stroke(from, to, style.width, style.color, false);
            return;
        }
        let (left, right) = (from.0.min(to.0), from.0.max(to.0));
        let (top, bottom) = (from.1.min(to.1), from.1.max(to.1));
        let (cx, cy) = ((left + right) * 0.5, (top + bottom) * 0.5);
        let (rx, ry) = ((right - left) * 0.5, (bottom - top) * 0.5);
        let width = style.width.max(1.0);
        let selection = self.selection.clone();
        let pixels = Arc::make_mut(&mut self.layers[self.active].pixels);
        for y in (top.floor() as i32).max(0)..(bottom.ceil() as i32).min(self.height as i32) {
            for x in (left.floor() as i32).max(0)..(right.ceil() as i32).min(self.width as i32) {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let painted = match shape {
                    Shape::Rectangle => {
                        px >= left
                            && px <= right
                            && py >= top
                            && py <= bottom
                            && (style.filled
                                || px < left + width
                                || px > right - width
                                || py < top + width
                                || py > bottom - width)
                    }
                    Shape::Ellipse => {
                        ((px - cx) / rx).powi(2) + ((py - cy) / ry).powi(2) <= 1.0
                            && (style.filled
                                || rx <= width
                                || ry <= width
                                || ((px - cx) / (rx - width)).powi(2)
                                    + ((py - cy) / (ry - width)).powi(2)
                                    >= 1.0)
                    }
                    Shape::Line => unreachable!(),
                };
                if painted && selected(selection.as_ref(), x as usize, y as usize) {
                    pixels[y as usize * self.width as usize + x as usize] = style.color;
                }
            }
        }
    }

    pub fn fill(&mut self, x: i32, y: i32, color: Pixel) {
        let _ = self.fill_cancellable(x, y, color, &std::sync::atomic::AtomicBool::new(false));
    }

    /// Returns false when cancelled. The caller must discard or cancel the edit.
    pub fn fill_cancellable(
        &mut self,
        x: i32,
        y: i32,
        color: Pixel,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> bool {
        use std::sync::atomic::Ordering;
        assert!(self.editing(), "Begin a transaction before filling");
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        let Some(start) = self.index(x, y) else {
            return true;
        };
        let selection = self.selection.clone();
        if !selected(selection.as_ref(), x as usize, y as usize) {
            return true;
        }
        let target = self.layers[self.active].pixels[start];
        if target == color {
            return true;
        }
        let pixels = Arc::make_mut(&mut self.layers[self.active].pixels);
        let width = self.width as usize;
        let mut stack = vec![start];
        pixels[start] = color;
        let mut visited = 0usize;
        while let Some(i) = stack.pop() {
            if visited.is_multiple_of(4096) && cancel.load(Ordering::Relaxed) {
                return false;
            }
            visited += 1;
            let mut visit = |n: usize| {
                if pixels[n] == target && selected(selection.as_ref(), n % width, n / width) {
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
        !cancel.load(Ordering::Relaxed)
    }

    pub fn invert(&mut self) {
        let _ = self.invert_cancellable(&std::sync::atomic::AtomicBool::new(false));
    }

    pub fn invert_cancellable(&mut self, cancel: &std::sync::atomic::AtomicBool) -> bool {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return false;
        }
        self.begin("Invert colors");
        let width = self.width as usize;
        let selection = self.selection.clone();
        for (i, pixel) in Arc::make_mut(&mut self.layers[self.active].pixels)
            .iter_mut()
            .enumerate()
        {
            if i.is_multiple_of(4096) && cancel.load(std::sync::atomic::Ordering::Relaxed) {
                self.cancel();
                return false;
            }
            if !selected(selection.as_ref(), i % width, i / width) {
                continue;
            }
            for c in &mut pixel[..3] {
                *c = 255 - *c;
            }
        }
        self.commit();
        true
    }
}

pub(crate) fn blend(dst: Pixel, src: Pixel, opacity: u8) -> Pixel {
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

fn blend_layer(dst: Pixel, src: Pixel, opacity: u8, mode: BlendMode) -> Pixel {
    if mode == BlendMode::Normal {
        return blend(dst, src, opacity);
    }
    let sa = f64::from(src[3]) * f64::from(opacity) / (255.0 * 255.0);
    let da = f64::from(dst[3]) / 255.0;
    if sa == 0.0 {
        return dst;
    }
    let alpha = sa + da - sa * da;
    let mut result = [0; 4];
    for c in 0..3 {
        let a = f64::from(dst[c]) / 255.0;
        let b = f64::from(src[c]) / 255.0;
        let mixed = match mode {
            BlendMode::Multiply => a * b,
            BlendMode::Screen => a + b - a * b,
            BlendMode::Overlay => {
                if a <= 0.5 {
                    2.0 * a * b
                } else {
                    1.0 - 2.0 * (1.0 - a) * (1.0 - b)
                }
            }
            BlendMode::Darken => a.min(b),
            BlendMode::Lighten => a.max(b),
            BlendMode::Difference => (a - b).abs(),
            BlendMode::Add => (a + b).min(1.0),
            BlendMode::Normal => b,
        };
        result[c] = (((1.0 - sa) * da * a + (1.0 - da) * sa * b + sa * da * mixed) / alpha * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8;
    }
    result[3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    result
}
