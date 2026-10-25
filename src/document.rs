mod history;
use history::Entry;

use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
};

pub type Pixel = [u8; 4];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Line,
    Rectangle,
    Ellipse,
    RoundedRectangle,
    Triangle,
    RightTriangle,
    Diamond,
    Pentagon,
    Hexagon,
    Octagon,
    Star,
    ArrowRight,
    ArrowLeft,
    ArrowUp,
    ArrowDown,
    Trapezoid,
    Parallelogram,
    Heart,
}
impl Shape {
    pub const ALL: [Self; 18] = [
        Self::Line,
        Self::Rectangle,
        Self::Ellipse,
        Self::RoundedRectangle,
        Self::Triangle,
        Self::RightTriangle,
        Self::Diamond,
        Self::Pentagon,
        Self::Hexagon,
        Self::Octagon,
        Self::Star,
        Self::ArrowRight,
        Self::ArrowLeft,
        Self::ArrowUp,
        Self::ArrowDown,
        Self::Trapezoid,
        Self::Parallelogram,
        Self::Heart,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Line => "Line",
            Self::Rectangle => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::RoundedRectangle => "Rounded rectangle",
            Self::Triangle => "Triangle",
            Self::RightTriangle => "Right triangle",
            Self::Diamond => "Diamond",
            Self::Pentagon => "Pentagon",
            Self::Hexagon => "Hexagon",
            Self::Octagon => "Octagon",
            Self::Star => "Star",
            Self::ArrowRight => "Right arrow",
            Self::ArrowLeft => "Left arrow",
            Self::ArrowUp => "Up arrow",
            Self::ArrowDown => "Down arrow",
            Self::Trapezoid => "Trapezoid",
            Self::Parallelogram => "Parallelogram",
            Self::Heart => "Heart",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeStyle {
    pub paint_mode: Option<BlendMode>,
    pub antialias: bool,
    pub outline: Option<Pixel>,
    pub color: Pixel,
    pub width: f32,
    pub filled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientStyle {
    pub blend_mode: BlendMode,
    pub start: Pixel,
    pub end: Pixel,
    pub shape: crate::gradient::GradientShape,
    pub mode: crate::gradient::GradientMode,
}

pub const MAX_PIXELS: usize = 4096 * 4096;
const HISTORY_BYTES: usize = 128 * 1024 * 1024;

pub use crate::selection::{EllipseSelection, Selection};

fn selected(selection: Option<&Selection>, x: usize, y: usize) -> bool {
    selection.is_none_or(|rect| rect.contains(x, y))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    Difference,
    Add,
    ColorBurn,
    ColorDodge,
    Reflect,
    Glow,
    Negation,
    Xor,
}
impl BlendMode {
    pub const ALL: [Self; 14] = [
        Self::Normal,
        Self::Multiply,
        Self::Screen,
        Self::Overlay,
        Self::Darken,
        Self::Lighten,
        Self::Difference,
        Self::Add,
        Self::ColorBurn,
        Self::ColorDodge,
        Self::Reflect,
        Self::Glow,
        Self::Negation,
        Self::Xor,
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
            Self::ColorBurn => "Color burn",
            Self::ColorDodge => "Color dodge",
            Self::Reflect => "Reflect",
            Self::Glow => "Glow",
            Self::Negation => "Negation",
            Self::Xor => "XOR",
        }
    }
}

#[derive(Clone)]
pub struct Layer {
    session_id: u64,
    pub metadata: std::collections::BTreeMap<String, String>,
    pub is_background: bool,
    pub name: String,
    pub visible: bool,
    pub opacity: u8,
    pub blend_mode: BlendMode,
    pixels: Arc<Vec<Pixel>>,
}

impl PartialEq for Layer {
    fn eq(&self, other: &Self) -> bool {
        self.metadata == other.metadata
            && self.is_background == other.is_background
            && self.name == other.name
            && self.visible == other.visible
            && self.opacity == other.opacity
            && self.blend_mode == other.blend_mode
            && self.pixels == other.pixels
    }
}
fn next_layer_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
impl Layer {
    /// Session identity survives edits, reordering and history restoration.
    pub fn session_id(&self) -> u64 {
        self.session_id
    }
    pub fn new(name: String, pixels: Vec<Pixel>) -> Self {
        Self {
            session_id: next_layer_id(),
            name,
            metadata: Default::default(),
            is_background: false,
            visible: true,
            opacity: 255,
            blend_mode: BlendMode::Normal,
            pixels: Arc::new(pixels),
        }
    }
    pub fn pixels(&self) -> &[Pixel] {
        &self.pixels
    }
    pub fn shared_pixels(&self) -> Arc<Vec<Pixel>> {
        self.pixels.clone()
    }
}

#[derive(Clone)]
struct Snapshot {
    metadata: std::collections::BTreeMap<String, String>,
    width: u32,
    height: u32,
    selection: Option<Selection>,
    layers: Vec<Layer>,
    active: usize,
    revision: u64,
    label: String,
}

#[derive(Clone)]
pub struct Document {
    pub metadata: std::collections::BTreeMap<String, String>,
    pub width: u32,
    pub height: u32,
    selection: Option<Selection>,
    layers: Vec<Layer>,
    active: usize,
    undo: VecDeque<Entry>,
    redo: Vec<Entry>,
    pending: Option<Snapshot>,
    brush_coverage: Option<Vec<u8>>,
    clone_pixels: Option<Arc<Vec<Pixel>>>,
    history_error: Option<String>,
    deferred_history: bool,
    history_pool: history::Pool,
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
            metadata: Default::default(),
            width,
            height,
            selection: None,
            layers,
            active,
            undo: VecDeque::new(),
            redo: Vec::new(),
            pending: None,
            brush_coverage: None,
            clone_pixels: None,
            history_error: None,
            deferred_history: false,
            history_pool: history::Pool::default(),
            revision: 0,
            next_revision: 1,
            saved_revision: 0,
        })
    }

    pub fn resolution(&self) -> crate::resolution::Dpi {
        crate::resolution::read(&self.metadata)
    }
    pub fn set_resolution(&mut self, dpi: crate::resolution::Dpi) -> Result<bool, String> {
        if self.editing() {
            return Err("Finish the current edit first".into());
        }
        if !dpi.valid() {
            return Err("Resolution must be between 1 and 10000 pixels per inch".into());
        }
        if self.resolution() == dpi {
            return Ok(false);
        }
        let mut metadata = self.metadata.clone();
        crate::resolution::write(&mut metadata, dpi)?;
        self.begin("Image resolution");
        self.metadata = metadata;
        Ok(self.commit())
    }

    pub fn selection(&self) -> Option<Selection> {
        self.selection.clone()
    }
    /// Record a boundary change without changing the saved image revision.
    pub fn set_selection_with_history(
        &mut self,
        selection: Option<Selection>,
        label: &str,
    ) -> bool {
        if self.editing() {
            return false;
        }
        let previous = self.selection.clone();
        self.set_selection(selection);
        let next = std::mem::replace(&mut self.selection, previous);
        self.begin(label);
        self.selection = next;
        self.commit()
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
        self.clear_pixels_cancellable(&std::sync::atomic::AtomicBool::new(false));
    }

    pub fn clear_pixels_cancellable(&mut self, cancel: &std::sync::atomic::AtomicBool) -> bool {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return false;
        }
        self.begin("Delete pixels");
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
            if selected(selection.as_ref(), i % width, i / width) {
                *pixel = [0; 4];
            }
        }
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            self.cancel();
            return false;
        }
        self.commit();
        true
    }
    pub fn crop_to_selection(&mut self) -> bool {
        self.crop_to_selection_cancellable(&std::sync::atomic::AtomicBool::new(false))
            .unwrap_or(false)
    }

    pub fn crop_to_selection_cancellable(
        &mut self,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<bool, String> {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(crate::files::CANCELLED.into());
        }
        let Some(rect) = self.selection.clone() else {
            return Ok(false);
        };
        if self.editing()
            || rect.width == 0
            || rect.height == 0
            || (rect.x == 0
                && rect.y == 0
                && rect.width == self.width
                && rect.height == self.height
                && rect.mask.is_none()
                && rect.ellipse.is_none())
        {
            return Ok(false);
        }
        let width = self.width as usize;
        self.transform_layers(
            "Crop to selection",
            rect.width,
            rect.height,
            cancel,
            |source| {
                let mut pixels = Vec::with_capacity(rect.width as usize * rect.height as usize);
                for y in rect.y..rect.y + rect.height {
                    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                        return None;
                    }
                    let start = y as usize * width + rect.x as usize;
                    if rect.mask.is_none() && rect.ellipse.is_none() {
                        pixels.extend_from_slice(&source[start..start + rect.width as usize]);
                    } else {
                        pixels.extend((0..rect.width as usize).map(|x| {
                            if rect.contains(rect.x as usize + x, y as usize) {
                                source[start + x]
                            } else {
                                [0; 4]
                            }
                        }));
                    }
                }
                Some(pixels)
            },
        )
    }

    /// Publish a computed image as one edit while retaining this document's history.
    pub fn apply_transformed(&mut self, replacement: Self, label: &str) -> Result<bool, String> {
        if self.editing() {
            return Err("Finish the current edit first.".into());
        }
        if self.width == replacement.width
            && self.height == replacement.height
            && self.layers == replacement.layers
            && self.metadata == replacement.metadata
        {
            return Ok(false);
        }
        self.begin(label);
        self.width = replacement.width;
        self.height = replacement.height;
        self.metadata = replacement.metadata;
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
        cancel: &std::sync::atomic::AtomicBool,
        transform: impl Fn(&[Pixel]) -> Option<Vec<Pixel>>,
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
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                self.cancel();
                return Err(crate::files::CANCELLED.into());
            }
            let Some(pixels) = transform(&layer.pixels) else {
                self.cancel();
                return Err(crate::files::CANCELLED.into());
            };
            layer.pixels = Arc::new(pixels);
        }
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            self.cancel();
            return Err(crate::files::CANCELLED.into());
        }
        self.width = width;
        self.height = height;
        self.selection = None;
        Ok(self.commit())
    }
    pub fn resize_image(&mut self, width: u32, height: u32, smooth: bool) -> Result<bool, String> {
        self.resize_image_cancellable(
            width,
            height,
            smooth,
            &std::sync::atomic::AtomicBool::new(false),
        )
    }

    pub fn resize_image_cancellable(
        &mut self,
        width: u32,
        height: u32,
        smooth: bool,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<bool, String> {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(crate::files::CANCELLED.into());
        }
        if (width, height) == (self.width, self.height) {
            return Ok(false);
        }
        let old = (self.width, self.height);
        self.transform_layers("Resize image", width, height, cancel, |pixels| {
            crate::transform::resize_cancellable(pixels, old, (width, height), smooth, cancel)
        })
    }
    pub fn resize_image_with_options_cancellable(
        &mut self,
        width: u32,
        height: u32,
        options: crate::resample::Options,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<bool, String> {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(crate::files::CANCELLED.into());
        }
        if (width, height) == (self.width, self.height) {
            return Ok(false);
        }
        let old = (self.width, self.height);
        self.transform_layers("Resize image", width, height, cancel, |pixels| {
            crate::resample::resize(pixels, old, (width, height), options, cancel).ok()
        })
    }

    pub fn resize_canvas(
        &mut self,
        width: u32,
        height: u32,
        anchor: usize,
    ) -> Result<bool, String> {
        self.resize_canvas_cancellable(
            width,
            height,
            anchor,
            &std::sync::atomic::AtomicBool::new(false),
        )
    }

    pub fn resize_canvas_cancellable(
        &mut self,
        width: u32,
        height: u32,
        anchor: usize,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<bool, String> {
        self.resize_canvas_filled_cancellable(width, height, anchor, [0; 4], cancel)
    }

    pub fn resize_canvas_filled_cancellable(
        &mut self,
        width: u32,
        height: u32,
        anchor: usize,
        fill: Pixel,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<bool, String> {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(crate::files::CANCELLED.into());
        }
        if anchor > 8 {
            return Err("Invalid canvas anchor.".into());
        }
        if (width, height) == (self.width, self.height) {
            return Ok(false);
        }
        let old = (self.width, self.height);
        self.transform_layers("Resize canvas", width, height, cancel, |pixels| {
            crate::transform::canvas_filled_cancellable(
                pixels,
                old,
                (width, height),
                anchor,
                fill,
                cancel,
            )
        })
    }
    pub fn reorient(&mut self, operation: crate::transform::Orientation) -> Result<bool, String> {
        self.reorient_cancellable(operation, &std::sync::atomic::AtomicBool::new(false))
    }

    pub fn reorient_cancellable(
        &mut self,
        operation: crate::transform::Orientation,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<bool, String> {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(crate::files::CANCELLED.into());
        }
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
        self.transform_layers(label, new.0, new.1, cancel, |pixels| {
            crate::transform::orient_cancellable(pixels, width, height, operation, cancel)
        })
    }

    /// Flip or half-turn only the active layer; canvas and selection remain fixed.
    pub fn reorient_active_cancellable(
        &mut self,
        operation: crate::transform::Orientation,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<bool, String> {
        use crate::transform::Orientation;
        if self.editing() {
            return Err("Finish the current edit first.".into());
        }
        let label = match operation {
            Orientation::FlipHorizontal => "Flip layer horizontal",
            Orientation::FlipVertical => "Flip layer vertical",
            Orientation::HalfTurn => "Rotate layer 180 degrees",
            _ => return Err("This layer operation requires a flip or half-turn.".into()),
        };
        let pixels = crate::transform::orient_cancellable(
            self.pixels(),
            self.width,
            self.height,
            operation,
            cancel,
        )
        .ok_or(crate::files::CANCELLED)?;
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(crate::files::CANCELLED.into());
        }
        self.begin(label);
        self.layers[self.active].pixels = Arc::new(pixels);
        Ok(self.commit())
    }

    pub fn fill_selection_cancellable(
        &mut self,
        color: Pixel,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<bool, String> {
        use std::sync::atomic::Ordering;
        if self.editing() {
            return Err("Finish the current edit first.".into());
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(crate::files::CANCELLED.into());
        }
        self.begin("Fill selection");
        let width = self.width as usize;
        let selection = self.selection.clone();
        for (i, pixel) in Arc::make_mut(&mut self.layers[self.active].pixels)
            .iter_mut()
            .enumerate()
        {
            if i.is_multiple_of(4096) && cancel.load(Ordering::Relaxed) {
                self.cancel();
                return Err(crate::files::CANCELLED.into());
            }
            if selected(selection.as_ref(), i % width, i / width) {
                *pixel = color;
            }
        }
        if cancel.load(Ordering::Relaxed) {
            self.cancel();
            return Err(crate::files::CANCELLED.into());
        }
        Ok(self.commit())
    }

    pub fn copy_pixels(&self, merged: bool) -> (u32, u32, Vec<Pixel>) {
        self.copy_pixels_cancellable(merged, &std::sync::atomic::AtomicBool::new(false))
            .expect("Uncancelled copy")
    }

    pub fn copy_pixels_cancellable(
        &self,
        merged: bool,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Option<(u32, u32, Vec<Pixel>)> {
        use std::sync::atomic::Ordering;
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let rect = self.selection.clone().unwrap_or(Selection {
            mask: None,
            ellipse: None,
            x: 0,
            y: 0,
            width: self.width,
            height: self.height,
        });
        if rect.width == 0 || rect.height == 0 {
            return Some((0, 0, Vec::new()));
        }
        let flattened = if merged {
            Some(self.composite_region(rect.x, rect.y, rect.width, rect.height, cancel)?)
        } else {
            None
        };
        let source = flattened.as_deref().unwrap_or_else(|| self.pixels());
        let mut pixels = Vec::with_capacity(rect.width as usize * rect.height as usize);
        for y in rect.y..rect.y + rect.height {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            let start = if merged {
                (y - rect.y) as usize * rect.width as usize
            } else {
                y as usize * self.width as usize + rect.x as usize
            };
            for x in 0..rect.width as usize {
                if x.is_multiple_of(4096) && cancel.load(Ordering::Relaxed) {
                    return None;
                }
                pixels.push(if rect.contains(rect.x as usize + x, y as usize) {
                    source[start + x]
                } else {
                    [0; 4]
                });
            }
        }
        (!cancel.load(Ordering::Relaxed)).then_some((rect.width, rect.height, pixels))
    }

    pub fn paste_active_cancellable(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[Pixel],
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<bool, String> {
        use std::sync::atomic::Ordering;
        Self::validate_size(width, height)?;
        if self.editing() || pixels.len() != width as usize * height as usize {
            return Err("Invalid or busy paste destination".into());
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(crate::files::CANCELLED.into());
        }
        let (x, y) = self.selection.as_ref().map(|s| (s.x, s.y)).unwrap_or((
            self.width.saturating_sub(width) / 2,
            self.height.saturating_sub(height) / 2,
        ));
        let clipped_width = width.min(self.width - x);
        let clipped_height = height.min(self.height - y);
        if clipped_width == 0 || clipped_height == 0 {
            return Err("Paste origin is outside the canvas".into());
        }
        self.begin("Paste pixels");
        let destination = Arc::make_mut(&mut self.layers[self.active].pixels);
        for row in 0..clipped_height {
            if cancel.load(Ordering::Relaxed) {
                self.cancel();
                return Err(crate::files::CANCELLED.into());
            }
            let source = row as usize * width as usize;
            let target = (row + y) as usize * self.width as usize + x as usize;
            destination[target..target + clipped_width as usize]
                .copy_from_slice(&pixels[source..source + clipped_width as usize]);
        }
        if cancel.load(Ordering::Relaxed) {
            self.cancel();
            return Err(crate::files::CANCELLED.into());
        }
        self.selection = Some(Selection {
            x,
            y,
            width: clipped_width,
            height: clipped_height,
            mask: None,
            ellipse: None,
        });
        Ok(self.commit())
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
        let mut snapshot =
            Self::from_layers(self.width, self.height, self.layers.clone(), self.active)
                .expect("valid document snapshot");
        snapshot.metadata = self.metadata.clone();
        snapshot.deferred_history = true;
        snapshot
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
            layer.session_id = next_layer_id();
            while layer.name.len() > 4091 {
                layer.name.pop();
            }
            layer.name.push_str(" copy");
            layer
        } else {
            Layer {
                session_id: next_layer_id(),
                name: format!("Layer {}", self.layers.len() + 1),
                metadata: Default::default(),
                is_background: false,
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

    pub fn can_merge_down(&self) -> bool {
        !self.editing() && self.active > 0
    }

    /// Composite the upper layer into the raw lower buffer, retaining lower properties.
    pub fn merge_down_cancellable(
        &mut self,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Option<bool> {
        use std::sync::atomic::Ordering;
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        if !self.can_merge_down() {
            return Some(false);
        }
        let top = &self.layers[self.active];
        let bottom = &self.layers[self.active - 1];
        let mut pixels = Vec::with_capacity(bottom.pixels.len());
        for (below, above) in bottom.pixels.chunks(4096).zip(top.pixels.chunks(4096)) {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            pixels.extend(
                below
                    .iter()
                    .zip(above)
                    .map(|(dst, src)| merge_pixel(*dst, *src, top.opacity, top.blend_mode)),
            );
        }
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        self.begin("Merge layer down");
        self.layers[self.active - 1].pixels = Arc::new(pixels);
        self.layers.remove(self.active);
        self.active -= 1;
        Some(self.commit())
    }

    pub fn move_layer(&mut self, up: bool) -> bool {
        let next = if up {
            self.active + 1
        } else {
            self.active.saturating_sub(1)
        };
        self.reorder_layer(self.active, next)
    }

    /// Move a layer to its final stack index, retaining the selected layer's identity.
    pub fn reorder_layer(&mut self, source: usize, target: usize) -> bool {
        if self.editing()
            || source == target
            || source >= self.layers.len()
            || target >= self.layers.len()
        {
            return false;
        }
        self.begin("Reorder layer");
        let layer = self.layers.remove(source);
        self.layers.insert(target, layer);
        self.active = if self.active == source {
            target
        } else if source < self.active && self.active <= target {
            self.active - 1
        } else if target <= self.active && self.active < source {
            self.active + 1
        } else {
            self.active
        };
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
        self.composite_region(0, 0, self.width, self.height, cancel)
    }

    /// Composite a bounded rectangle using the same blending as a full image.
    pub fn composite_region(
        &self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Option<Vec<Pixel>> {
        use std::sync::atomic::Ordering;
        if cancel.load(Ordering::Relaxed)
            || width == 0
            || height == 0
            || x.checked_add(width)? > self.width
            || y.checked_add(height)? > self.height
        {
            return None;
        }
        let mut pixels = vec![[0; 4]; width as usize * height as usize];
        let mut first = true;
        for layer in self.layers.iter().filter(|l| l.visible && l.opacity != 0) {
            for row in 0..height as usize {
                let start = (y as usize + row) * self.width as usize + x as usize;
                let source = &layer.pixels[start..start + width as usize];
                let output = &mut pixels[row * width as usize..(row + 1) * width as usize];
                for (output, source) in output.chunks_mut(4096).zip(source.chunks(4096)) {
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
        self.undo.iter().rev().map(Entry::label)
    }
    pub fn redo_history(&self) -> impl Iterator<Item = &str> {
        self.redo.iter().rev().map(Entry::label)
    }

    pub fn history_bytes(&self) -> usize {
        let mut buffers = HashSet::new();
        let mut masks = HashSet::new();
        let mask_bytes: usize = self
            .undo
            .iter()
            .chain(self.redo.iter())
            .filter_map(Entry::memory)
            .filter_map(|s| s.selection.as_ref().and_then(|s| s.mask.as_ref()))
            .filter(|mask| masks.insert(Arc::as_ptr(&mask.pixels)))
            .map(|mask| mask.pixels.len())
            .sum();
        self.undo
            .iter()
            .chain(self.redo.iter())
            .filter_map(Entry::memory)
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
                metadata: self.metadata.clone(),
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
        self.brush_coverage = None;
        self.clone_pixels = None;
        let Some(before) = self.pending.take() else {
            return false;
        };
        let image_changed = before.metadata != self.metadata
            || before.layers != self.layers
            || before.width != self.width
            || before.height != self.height;
        if !image_changed && before.selection == self.selection {
            return false;
        }
        self.undo.push_back(Entry::Memory(before));
        self.redo.clear();
        if image_changed {
            self.revision = self.next_revision;
            self.next_revision += 1;
        }
        self.trim_history();
        true
    }

    pub fn history_disk_bytes(&self) -> u64 {
        let mut seen = HashSet::new();
        self.undo
            .iter()
            .chain(self.redo.iter())
            .map(|entry| entry.disk_bytes(&mut seen))
            .sum()
    }

    pub fn history_error(&self) -> Option<&str> {
        self.history_error.as_deref()
    }

    /// Defer file writes so a GUI can call `store_history` on a worker clone.
    pub fn defer_history_storage(&mut self, deferred: bool) {
        self.deferred_history = deferred;
    }

    pub fn history_needs_storage(&self) -> bool {
        self.history_bytes() > HISTORY_BYTES
    }

    pub fn history_navigation_needs_io(&self, steps: usize, redo: bool) -> bool {
        if redo {
            self.redo
                .iter()
                .rev()
                .take(steps)
                .any(|e| e.memory().is_none())
        } else {
            self.undo
                .iter()
                .rev()
                .take(steps)
                .any(|e| e.memory().is_none())
        }
    }

    pub fn store_history(&mut self) {
        self.spill_history(HISTORY_BYTES, history::temporary_file);
    }

    pub fn store_history_cancellable(&mut self, cancel: &std::sync::atomic::AtomicBool) -> bool {
        self.store_history_with_budget_cancellable(HISTORY_BYTES, cancel)
    }

    /// Store older undo/redo states without discarding them, using a caller's RAM budget.
    /// Storage failures retain the remaining states in RAM and set `history_error`.
    pub fn store_history_with_budget_cancellable(
        &mut self,
        budget: usize,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> bool {
        self.spill_history_cancellable(budget, history::temporary_file, cancel, u64::MAX)
    }

    /// Limit compressed history payload bytes without dropping undo states.
    pub fn store_history_with_limits_cancellable(
        &mut self,
        ram_budget: usize,
        disk_budget: u64,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> bool {
        let available = disk_budget.saturating_sub(self.history_disk_bytes());
        self.spill_history_cancellable(ram_budget, history::temporary_file, cancel, available)
    }

    fn trim_history(&mut self) {
        if !self.deferred_history {
            self.store_history();
        }
    }

    fn spill_history(
        &mut self,
        budget: usize,
        create: impl FnMut() -> std::io::Result<std::fs::File>,
    ) {
        self.spill_history_cancellable(
            budget,
            create,
            &std::sync::atomic::AtomicBool::new(false),
            u64::MAX,
        );
    }

    fn spill_history_cancellable(
        &mut self,
        budget: usize,
        mut create: impl FnMut() -> std::io::Result<std::fs::File>,
        cancel: &std::sync::atomic::AtomicBool,
        mut disk_remaining: u64,
    ) -> bool {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return false;
        }
        self.history_error = None;
        while self.history_bytes() > budget {
            let entry = self
                .undo
                .iter_mut()
                .chain(self.redo.iter_mut())
                .find(|entry| entry.memory().is_some());
            let Some(entry) = entry else {
                break;
            };
            if let Err(error) = entry.spill_limited(
                &mut self.history_pool,
                &mut create,
                cancel,
                &mut disk_remaining,
            ) {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return false;
                }
                self.history_error = Some(format!(
                    "Could not store undo history: {error}. History remains in RAM above its normal limit."
                ));
                break;
            }
        }
        !cancel.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn cancel(&mut self) {
        self.brush_coverage = None;
        self.clone_pixels = None;
        if let Some(before) = self.pending.take() {
            self.width = before.width;
            self.height = before.height;
            self.selection = before.selection.clone();
            self.metadata = before.metadata;
            self.layers = before.layers;
            self.active = before.active;
        }
    }

    pub fn undo(&mut self) -> bool {
        self.undo_cancellable(&std::sync::atomic::AtomicBool::new(false))
    }

    pub fn redo(&mut self) -> bool {
        self.redo_cancellable(&std::sync::atomic::AtomicBool::new(false))
    }

    pub fn undo_cancellable(&mut self, cancel: &std::sync::atomic::AtomicBool) -> bool {
        self.navigate_history(false, cancel)
    }
    pub fn redo_cancellable(&mut self, cancel: &std::sync::atomic::AtomicBool) -> bool {
        self.navigate_history(true, cancel)
    }

    fn navigate_history(&mut self, redo: bool, cancel: &std::sync::atomic::AtomicBool) -> bool {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return false;
        }
        let entry = if redo {
            self.redo.last_mut()
        } else {
            self.undo.back_mut()
        };
        let Some(entry) = entry else {
            self.cancel();
            return false;
        };
        // Load and validate before touching the current document or either stack.
        let target = match entry.load(&mut self.history_pool, cancel) {
            Ok(target) => target,
            Err(error) => {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return false;
                }
                self.history_error = Some(format!(
                    "Could not restore undo history: {error}. The image and history were kept unchanged."
                ));
                return false;
            }
        };
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return false;
        }
        self.cancel();
        let current = Snapshot {
            metadata: self.metadata.clone(),
            width: std::mem::replace(&mut self.width, target.width),
            height: std::mem::replace(&mut self.height, target.height),
            selection: std::mem::replace(&mut self.selection, target.selection),
            layers: std::mem::replace(&mut self.layers, target.layers),
            active: std::mem::replace(&mut self.active, target.active),
            revision: self.revision,
            label: target.label,
        };
        self.metadata = target.metadata.clone();
        self.revision = target.revision;
        self.history_error = None;
        if redo {
            self.redo.pop();
            self.undo.push_back(Entry::Memory(current));
        } else {
            self.undo.pop_back();
            self.redo.push(Entry::Memory(current));
        }
        self.trim_history();
        true
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
        crate::shapes::stroke(
            pixels,
            (self.width, self.height),
            selection.as_ref(),
            crate::shapes::Stroke {
                from,
                to,
                diameter,
                color,
                erase,
            },
            &std::sync::atomic::AtomicBool::new(false),
        );
    }

    pub fn brush_segment(
        &mut self,
        from: (f32, f32),
        to: (f32, f32),
        diameter: f32,
        color: Pixel,
        options: crate::brush::Options,
        erase: bool,
    ) {
        self.paint_segment(
            from,
            to,
            diameter,
            if erase {
                crate::brush::Paint::Erase
            } else {
                crate::brush::Paint::Color(color)
            },
            options,
        );
    }

    /// Freeze clone sampling separately from the destination at stroke start.
    pub fn prepare_clone_source(&mut self, layer_id: u64, merged: bool) -> Result<(), String> {
        if self.editing() {
            return Err("Finish the current stroke first".into());
        }
        let layer = self
            .layers
            .iter()
            .find(|layer| layer.session_id == layer_id)
            .ok_or("The clone source layer is no longer available")?;
        self.clone_pixels = Some(if merged {
            Arc::new(self.composite())
        } else {
            layer.shared_pixels()
        });
        Ok(())
    }

    pub fn paint_segment(
        &mut self,
        from: (f32, f32),
        to: (f32, f32),
        diameter: f32,
        paint: crate::brush::Paint,
        options: crate::brush::Options,
    ) {
        let before = self
            .pending
            .as_ref()
            .expect("Begin a transaction before drawing");
        let source = &before.layers[self.active].pixels;
        let coverage = self
            .brush_coverage
            .get_or_insert_with(|| vec![0; source.len()]);
        crate::brush::render(
            Arc::make_mut(&mut self.layers[self.active].pixels).as_mut_slice(),
            source,
            coverage,
            (self.width, self.height),
            self.selection.as_ref(),
            crate::brush::Segment {
                clone_source: self.clone_pixels.as_deref().map(Vec::as_slice),
                from,
                to,
                diameter,
                paint,
                options,
            },
            &std::sync::atomic::AtomicBool::new(false),
        );
    }

    /// Draw a single-pixel path between pointer samples without antialiasing.
    pub fn pencil(&mut self, from: (f32, f32), to: (f32, f32), color: Pixel) {
        self.pencil_with_mode(from, to, color, None);
    }

    pub fn pencil_with_mode(
        &mut self,
        from: (f32, f32),
        to: (f32, f32),
        color: Pixel,
        mode: Option<BlendMode>,
    ) {
        assert!(self.editing(), "Begin a transaction before drawing");
        if ![from.0, from.1, to.0, to.1].iter().all(|v| v.is_finite()) {
            return;
        }
        let (mut x0, mut y0) = (f64::from(from.0).floor(), f64::from(from.1).floor());
        let (mut x1, mut y1) = (f64::from(to.0).floor(), f64::from(to.1).floor());
        let steep = (y1 - y0).abs() > (x1 - x0).abs();
        if steep {
            std::mem::swap(&mut x0, &mut y0);
            std::mem::swap(&mut x1, &mut y1);
        }
        if x0 > x1 {
            std::mem::swap(&mut x0, &mut x1);
            std::mem::swap(&mut y0, &mut y1);
        }
        let major_limit = if steep { self.height } else { self.width };
        let minor_limit = if steep { self.width } else { self.height };
        // Clip the iteration range, so off-canvas drags never cause unbounded work.
        let start = x0.max(0.0);
        let end = x1.min(f64::from(major_limit) - 1.0);
        if start > end {
            return;
        }
        let original = self.pending.as_ref().unwrap().layers[self.active]
            .pixels
            .clone();
        let pixels = Arc::make_mut(&mut self.layers[self.active].pixels);
        for major in start as u32..=end as u32 {
            let minor = if x0 == x1 {
                y0
            } else {
                (y0 + (f64::from(major) - x0) / (x1 - x0) * (y1 - y0)).round()
            };
            if minor < 0.0 || minor >= f64::from(minor_limit) {
                continue;
            }
            let (x, y) = if steep {
                (minor as usize, major as usize)
            } else {
                (major as usize, minor as usize)
            };
            if self.selection.as_ref().is_none_or(|s| s.contains(x, y)) {
                let index = y * self.width as usize + x;
                pixels[index] =
                    mode.map_or(color, |mode| blend_layer(original[index], color, 255, mode));
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
        if let Some(pixels) = crate::gradient::render(
            &before.layers[before.active].pixels,
            self.width,
            self.height,
            self.selection.as_ref(),
            crate::gradient::Gradient { from, to, style },
            &std::sync::atomic::AtomicBool::new(false),
        ) {
            self.publish_preview(pixels)
                .expect("Gradient transaction is active");
        }
    }

    pub fn publish_move_preview(
        &mut self,
        pixels: Vec<Pixel>,
        dx: i32,
        dy: i32,
    ) -> Result<(), String> {
        let before = self
            .pending
            .as_ref()
            .ok_or("Begin a move transaction first")?;
        let selection = crate::movement::translated_selection(
            self.width,
            self.height,
            before.selection.as_ref(),
            dx,
            dy,
        );
        self.publish_preview(pixels)?;
        self.selection = selection;
        Ok(())
    }

    pub fn preview_move(&mut self, dx: i32, dy: i32, copy: bool, selection_only: bool) {
        let before = self
            .pending
            .as_ref()
            .expect("Begin a move transaction first");
        if selection_only {
            self.layers = before.layers.clone();
            self.active = before.active;
            self.selection = crate::movement::translated_selection(
                self.width,
                self.height,
                before.selection.as_ref(),
                dx,
                dy,
            );
        } else if let Some(pixels) = crate::movement::render(
            &before.layers[before.active].pixels,
            self.width,
            self.height,
            before.selection.as_ref(),
            crate::movement::Parameters { dx, dy, copy },
            &std::sync::atomic::AtomicBool::new(false),
        ) {
            self.publish_move_preview(pixels, dx, dy)
                .expect("Move transaction is active");
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
        if let Some(pixels) = crate::shapes::render(
            &before.layers[before.active].pixels,
            self.width,
            self.height,
            self.selection.as_ref(),
            crate::shapes::Parameters {
                shape,
                from,
                to,
                style,
            },
            &std::sync::atomic::AtomicBool::new(false),
        ) {
            self.publish_preview(pixels)
                .expect("Shape transaction is active");
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

    /// Fill matching pixels on the active layer; cancellation leaves the transaction pending.
    pub fn fill_sampled_cancellable(
        &mut self,
        seed: (i32, i32),
        color: Pixel,
        options: crate::selection::ColorMatch,
        merged: bool,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<bool, String> {
        self.fill_sampled_with_mode_cancellable(seed, color, options, merged, None, cancel)
    }

    pub fn fill_sampled_with_mode_cancellable(
        &mut self,
        seed: (i32, i32),
        color: Pixel,
        options: crate::selection::ColorMatch,
        merged: bool,
        mode: Option<BlendMode>,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<bool, String> {
        use std::sync::atomic::Ordering;
        assert!(self.editing(), "Begin a transaction before filling");
        if cancel.load(Ordering::Relaxed) {
            return Err(crate::files::CANCELLED.into());
        }
        if self.index(seed.0, seed.1).is_none() {
            return Ok(false);
        }
        let composite = if merged {
            Some(
                self.composite_cancellable(cancel)
                    .ok_or(crate::files::CANCELLED)?,
            )
        } else {
            None
        };
        let source = composite.as_deref().unwrap_or_else(|| self.pixels());
        let region = crate::selection::color_region_cancellable(
            self.width,
            self.height,
            source,
            (seed.0 as u32, seed.1 as u32),
            options,
            self.selection.as_ref(),
            cancel,
        )?;
        if region.width == 0 || region.height == 0 {
            return Ok(false);
        }
        let pixels = Arc::make_mut(&mut self.layers[self.active].pixels);
        let mut changed = false;
        for y in region.y..region.y + region.height {
            for x in region.x..region.x + region.width {
                if x.is_multiple_of(4096) && cancel.load(Ordering::Relaxed) {
                    return Err(crate::files::CANCELLED.into());
                }
                if region.contains(x as usize, y as usize) {
                    let pixel = &mut pixels[y as usize * self.width as usize + x as usize];
                    let result = mode.map_or(color, |mode| blend_layer(*pixel, color, 255, mode));
                    changed |= *pixel != result;
                    *pixel = result;
                }
            }
            if cancel.load(Ordering::Relaxed) {
                return Err(crate::files::CANCELLED.into());
            }
        }
        Ok(changed)
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

// Destructive layer merging quantizes alpha and mixed channels before normalization.
// Keep this separate from the continuous-precision display compositor.
pub(crate) fn merge_pixel(dst: Pixel, src: Pixel, opacity: u8, mode: BlendMode) -> Pixel {
    let scale = |a: u32, b: u32| (a * b + 127) / 255;
    let source_alpha = scale(u32::from(src[3]), u32::from(opacity));
    if source_alpha == 0 {
        return dst;
    }
    let backdrop_alpha = u32::from(dst[3]);
    let remaining = scale(backdrop_alpha, 255 - source_alpha);
    let overlap = scale(backdrop_alpha, source_alpha);
    let uncovered = source_alpha - overlap;
    let alpha = source_alpha + remaining;
    let mut result = [0; 4];
    for c in 0..3 {
        let (a, b) = (u32::from(dst[c]), u32::from(src[c]));
        let mixed = match mode {
            BlendMode::Normal => b,
            BlendMode::Multiply => scale(a, b),
            BlendMode::Screen => a + b - scale(a, b),
            BlendMode::Overlay => {
                if a < 128 {
                    scale(2 * a, b)
                } else {
                    255 - scale(2 * (255 - a), 255 - b)
                }
            }
            BlendMode::Darken => a.min(b),
            BlendMode::Lighten => a.max(b),
            BlendMode::Difference => a.abs_diff(b),
            BlendMode::Add => (a + b).min(255),
            BlendMode::ColorBurn => ((255 - a) * 255)
                .checked_div(b)
                .map_or(0, |value| 255_u32.saturating_sub(value)),
            BlendMode::ColorDodge => {
                if b == 255 {
                    255
                } else {
                    (a * 255 / (255 - b)).min(255)
                }
            }
            BlendMode::Reflect => {
                if b == 255 {
                    255
                } else {
                    (a * a / (255 - b)).min(255)
                }
            }
            BlendMode::Glow => {
                if a == 255 {
                    255
                } else {
                    (b * b / (255 - a)).min(255)
                }
            }
            BlendMode::Negation => 255 - 255_u32.abs_diff(a + b),
            BlendMode::Xor => a ^ b,
        };
        result[c] = ((a * remaining + b * uncovered + mixed * overlap) / alpha) as u8;
    }
    result[3] = alpha as u8;
    result
}

pub(crate) fn blend_layer(dst: Pixel, src: Pixel, opacity: u8, mode: BlendMode) -> Pixel {
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
            BlendMode::ColorBurn => {
                if b == 0.0 {
                    0.0
                } else {
                    (1.0 - (1.0 - a) / b).max(0.0)
                }
            }
            BlendMode::ColorDodge => {
                if b == 1.0 {
                    1.0
                } else {
                    (a / (1.0 - b)).min(1.0)
                }
            }
            BlendMode::Reflect => {
                if b == 1.0 {
                    1.0
                } else {
                    (a * a / (1.0 - b)).min(1.0)
                }
            }
            BlendMode::Glow => {
                if a == 1.0 {
                    1.0
                } else {
                    (b * b / (1.0 - a)).min(1.0)
                }
            }
            BlendMode::Negation => 1.0 - (1.0 - a - b).abs(),
            BlendMode::Xor => f64::from(dst[c] ^ src[c]) / 255.0,
            BlendMode::Normal => b,
        };
        result[c] = (((1.0 - sa) * da * a + (1.0 - da) * sa * b + sa * da * mixed) / alpha * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8;
    }
    result[3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    result
}

#[cfg(test)]
mod transform_tests {
    use super::*;
    use std::{
        cell::Cell,
        sync::atomic::{AtomicBool, Ordering},
    };

    #[test]
    fn cancellation_after_first_layer_restores_the_entire_transaction() {
        let mut doc = Document::new(3, 2, [10, 20, 30, 255]).unwrap();
        doc.add_layer(true);
        doc.set_selection(Some(Selection::elliptical(0, 0, 2, 2)));
        doc.mark_saved();
        let before = doc.snapshot();
        let selection = doc.selection();
        let history = doc.history().count();
        let cancel = AtomicBool::new(false);
        let calls = Cell::new(0);
        let result = doc.transform_layers("Resize", 4, 3, &cancel, |_| {
            calls.set(calls.get() + 1);
            cancel.store(true, Ordering::Relaxed);
            Some(vec![[0; 4]; 12])
        });
        assert_eq!(result, Err(crate::files::CANCELLED.into()));
        assert_eq!(calls.get(), 1);
        assert_eq!((doc.width, doc.height), (3, 2));
        assert!(doc.layers() == before.layers());
        assert_eq!(doc.selection(), selection);
        assert_eq!(doc.history().count(), history);
        assert!(!doc.editing());
        assert!(!doc.dirty());
    }
}
