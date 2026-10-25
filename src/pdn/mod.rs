//! Bounded PDN3 interchange. Object records are interpreted as data only.
mod memory;
mod nrbf;
mod write;
use crate::document::{BlendMode, Document, Layer};
use nrbf::{Graph, Value};
use std::{
    collections::BTreeMap,
    io::Read,
    sync::atomic::{AtomicBool, Ordering},
};
pub use write::write;

pub type Metadata = BTreeMap<String, String>;
pub struct Interchange {
    pub document: Document,
    pub metadata: Metadata,
    pub layer_metadata: Vec<Metadata>,
    pub backgrounds: Vec<bool>,
}
const MAX_BYTES: usize = 256 * 1024 * 1024;
const MODES: [BlendMode; 14] = [
    BlendMode::Normal,
    BlendMode::Multiply,
    BlendMode::Add,
    BlendMode::ColorBurn,
    BlendMode::ColorDodge,
    BlendMode::Reflect,
    BlendMode::Glow,
    BlendMode::Overlay,
    BlendMode::Difference,
    BlendMode::Negation,
    BlendMode::Lighten,
    BlendMode::Darken,
    BlendMode::Screen,
    BlendMode::Xor,
];
fn number(g: &Graph, v: &Value, field: &str) -> Result<i64, String> {
    g.field(v, field)?.number()
}
fn boolean(g: &Graph, v: &Value, field: &str) -> Result<bool, String> {
    match number(g, v, field)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err("Invalid boolean".into()),
    }
}
fn metadata(g: &Graph, v: &Value) -> Result<Metadata, String> {
    let mut result = Metadata::new();
    if let Some(items) = g.optional(v, "userMetadataItems")? {
        let items = items.array()?;
        if items.len() > 4096 {
            return Err("Too many metadata entries".into());
        }
        for pair in items {
            result.insert(
                g.field(pair, "key")?.text()?.into(),
                g.field(pair, "value")?.text()?.into(),
            );
        }
    } else if let Some(collection) = g.optional(v, "userMetaData")?
        && !matches!(collection, Value::Null)
    {
        g.require_type(
            collection,
            "System.Collections.Specialized.NameValueCollection",
        )?;
        let count = number(g, collection, "Count")?;
        let keys = g.field(collection, "Keys")?.array()?;
        let values = g.field(collection, "Values")?.array()?;
        if !(0..=4096).contains(&count)
            || keys.len() != count as usize
            || values.len() != keys.len()
        {
            return Err("Invalid metadata collection".into());
        }
        for (key, value) in keys.iter().zip(values) {
            let key = g.resolve(key)?;
            if matches!(key, Value::Null) {
                continue;
            }
            let count = number(g, value, "_size")?;
            let items = g.field(value, "_items")?.array()?;
            if count < 0 || count as usize > items.len() || count > 4096 {
                return Err("Invalid metadata values".into());
            }
            let mut text = Vec::new();
            for item in &items[..count as usize] {
                let item = g.resolve(item)?;
                if !matches!(item, Value::Null) {
                    text.push(item.text()?);
                }
            }
            if !text.is_empty() {
                result.insert(key.text()?.into(), text.join(","));
            }
        }
    }
    Ok(result)
}
struct Checked<'a, R> {
    inner: R,
    cancel: &'a AtomicBool,
}
impl<R: Read> Read for Checked<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::other(crate::files::CANCELLED));
        }
        let count = buf.len().min(65536);
        self.inner.read(&mut buf[..count])
    }
}
fn bytes<const N: usize>(r: &mut impl Read) -> Result<[u8; N], String> {
    let mut b = [0; N];
    r.read_exact(&mut b).map_err(|e| e.to_string())?;
    Ok(b)
}
fn be(r: &mut impl Read) -> Result<usize, String> {
    Ok(u32::from_be_bytes(bytes(r)?) as usize)
}
fn pixels(r: &mut impl Read, length: usize, cancel: &AtomicBool) -> Result<Vec<u8>, String> {
    let version = bytes::<1>(r)?[0];
    let size = be(r)?;
    if version > 1 || size == 0 || size > MAX_BYTES {
        return Err("Invalid pixel block format".into());
    }
    let count = length.div_ceil(size);
    if count > 65536 {
        return Err("Too many pixel blocks".into());
    }
    let mut result = vec![0; length];
    let mut seen = vec![false; count];
    for _ in 0..count {
        if cancel.load(Ordering::Relaxed) {
            return Err(crate::files::CANCELLED.into());
        }
        let index = be(r)?;
        let compressed = be(r)?;
        if index >= count || seen[index] || compressed > MAX_BYTES {
            return Err("Invalid pixel block".into());
        }
        seen[index] = true;
        let offset = index * size;
        let expected = size.min(length - offset);
        let target = &mut result[offset..offset + expected];
        if version == 1 {
            if compressed != expected {
                return Err("Invalid raw block length".into());
            }
            r.read_exact(target).map_err(|e| e.to_string())?;
        } else {
            let mut limited = std::io::BufReader::new(r.take(compressed as u64));
            {
                let gzip = flate2::bufread::GzDecoder::new(&mut limited);
                let mut gzip = Checked {
                    inner: gzip,
                    cancel,
                };
                gzip.read_exact(target).map_err(|e| e.to_string())?;
                if gzip.read(&mut [0]).map_err(|e| e.to_string())? != 0 {
                    return Err("Expanded block exceeds expected length".into());
                }
            }
            if !limited.buffer().is_empty() || limited.get_ref().limit() != 0 {
                return Err("Trailing data in pixel block".into());
            }
        }
    }
    Ok(result)
}
pub fn read(input: impl Read, cancel: &AtomicBool) -> Result<Interchange, String> {
    let mut r = Checked {
        inner: input,
        cancel,
    };
    let signature = bytes::<4>(&mut r)?;
    if signature[..2] == [31, 139] {
        return read_gzip(std::io::Cursor::new(signature).chain(r), cancel);
    }
    if signature != *b"PDN3" {
        return Err("Unsupported PDN signature".into());
    }
    let h = bytes::<3>(&mut r)?;
    let header = usize::from(h[0]) | usize::from(h[1]) << 8 | usize::from(h[2]) << 16;
    if header > 1024 * 1024 {
        return Err("Header exceeds limit".into());
    }
    let skipped = std::io::copy(&mut r.by_ref().take(header as u64), &mut std::io::sink())
        .map_err(|e| e.to_string())?;
    if skipped != header as u64 {
        return Err("Truncated PDN header".into());
    }
    let marker = bytes::<2>(&mut r)?;
    match marker {
        [0, 1] => read_graph(r, cancel, true),
        [31, 139] => read_gzip(std::io::Cursor::new(marker).chain(r), cancel),
        _ => Err("Unsupported PDN container variant".into()),
    }
}
fn read_gzip(input: impl Read, cancel: &AtomicBool) -> Result<Interchange, String> {
    let mut gzip = Checked {
        inner: flate2::read::GzDecoder::new(input).take((MAX_BYTES + 16 * 1024 * 1024 + 1) as u64),
        cancel,
    };
    let data = read_graph(&mut gzip, cancel, false)?;
    if gzip.read(&mut [0]).map_err(|e| e.to_string())? != 0 {
        return Err("Trailing compressed graph data".into());
    }
    Ok(data)
}
fn read_graph(
    mut r: impl Read,
    cancel: &AtomicBool,
    allow_deferred: bool,
) -> Result<Interchange, String> {
    let g = nrbf::read(&mut r)?;
    let root = g.root()?;
    g.require_type(root, "PaintDotNet.Document")?;
    let width = u32::try_from(number(&g, root, "width")?).map_err(|_| "Invalid width")?;
    let height = u32::try_from(number(&g, root, "height")?).map_err(|_| "Invalid height")?;
    Document::validate_size(width, height)?;
    let list = g.field(root, "layers")?;
    g.require_type(list, "PaintDotNet.LayerList")?;
    let count = number(&g, list, "ArrayList+_size")?;
    let records = g.field(list, "ArrayList+_items")?.array()?;
    if !(1..=32).contains(&count)
        || count as usize > records.len()
        || records.len() > 256
        || width as usize * height as usize * 4 * count as usize > MAX_BYTES
    {
        return Err("Document exceeds layer or memory limits".into());
    }
    let mut layers = Vec::new();
    let mut layer_metadata = Vec::new();
    let mut backgrounds = Vec::new();
    let mut total = 0usize;
    let mut memories = memory::Reader::new(&g, &mut r, cancel, allow_deferred);
    for record in &records[..count as usize] {
        g.require_type(record, "PaintDotNet.BitmapLayer")?;
        let surface = g.field(record, "surface")?;
        g.require_type(surface, "PaintDotNet.Surface")?;
        if number(&g, surface, "width")? != i64::from(width)
            || number(&g, surface, "height")? != i64::from(height)
            || number(&g, record, "Layer+width")? != i64::from(width)
            || number(&g, record, "Layer+height")? != i64::from(height)
        {
            return Err("Layer dimensions differ from document".into());
        }
        let stride =
            usize::try_from(number(&g, surface, "stride")?).map_err(|_| "Invalid row stride")?;
        let length = stride
            .checked_mul(height as usize)
            .ok_or("Pixel size overflow")?;
        total = total.checked_add(length).ok_or("Pixel size overflow")?;
        if stride < width as usize * 4 || !stride.is_multiple_of(4) || total > MAX_BYTES {
            return Err("Pixel data exceeds limit".into());
        }
        let memory = g.field(surface, "scan0")?;
        let raw = memories.read(memory)?;
        if raw.len() != length {
            return Err("Pixel buffer length differs from surface".into());
        }
        let mut rgba = Vec::with_capacity(width as usize * height as usize);
        for row in raw.chunks_exact(stride) {
            if cancel.load(Ordering::Relaxed) {
                return Err(crate::files::CANCELLED.into());
            }
            rgba.extend(
                row[..width as usize * 4]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|p| [p[2], p[1], p[0], p[3]]),
            );
        }
        let properties = g.field(record, "Layer+properties")?;
        let mut layer = Layer::new(g.field(properties, "name")?.text()?.into(), rgba);
        layer.visible = boolean(&g, properties, "visible")?;
        backgrounds.push(boolean(&g, properties, "isBackground")?);
        layer.opacity = u8::try_from(
            g.optional(properties, "opacity")?
                .map(Value::number)
                .transpose()?
                .unwrap_or(255),
        )
        .map_err(|_| "Invalid opacity")?;
        let mut mode = g
            .optional(properties, "blendMode")?
            .map(|v| number(&g, v, "value__"))
            .transpose()?
            .unwrap_or(0);
        let legacy = g.field(record, "properties")?;
        if let Some(opacity) = g.optional(legacy, "opacity")?
            && opacity.number()? >= 0
        {
            layer.opacity = u8::try_from(opacity.number()?).map_err(|_| "Invalid opacity")?;
        }
        if let Some(Value::Object(name, _)) = g.optional(legacy, "blendOp")? {
            let name = name
                .rsplit('+')
                .next()
                .unwrap_or(name)
                .trim_end_matches("BlendOp");
            mode = [
                "Normal",
                "Multiply",
                "Additive",
                "ColorBurn",
                "ColorDodge",
                "Reflect",
                "Glow",
                "Overlay",
                "Difference",
                "Negation",
                "Lighten",
                "Darken",
                "Screen",
                "Xor",
            ]
            .iter()
            .position(|n| n.eq_ignore_ascii_case(name))
            .ok_or("Unsupported blend operator")? as i64;
        }
        layer.blend_mode = *MODES
            .get(usize::try_from(mode).map_err(|_| "Invalid blend mode")?)
            .ok_or("Unsupported blend mode")?;
        layer_metadata.push(metadata(&g, properties)?);
        layers.push(layer);
    }
    if r.read(&mut [0]).map_err(|e| e.to_string())? != 0 {
        return Err("Trailing document data".into());
    }
    Ok(Interchange {
        document: Document::from_layers(width, height, layers, count as usize - 1)?,
        metadata: metadata(&g, root)?,
        layer_metadata,
        backgrounds,
    })
}

pub fn read_document(input: impl Read, cancel: &AtomicBool) -> Result<Document, String> {
    let data = read(input, cancel)?;
    let mut layers = data.document.layers().to_vec();
    for (index, layer) in layers.iter_mut().enumerate() {
        layer.metadata = data.layer_metadata[index].clone();
        layer.is_background = data.backgrounds[index];
    }
    let mut doc = Document::from_layers(
        data.document.width,
        data.document.height,
        layers,
        data.document.active_layer(),
    )?;
    doc.metadata = data.metadata;
    Ok(doc)
}

pub fn write_document(
    document: &Document,
    output: impl std::io::Write,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let data = Interchange {
        document: document.snapshot(),
        metadata: document.metadata.clone(),
        layer_metadata: document
            .layers()
            .iter()
            .map(|l| l.metadata.clone())
            .collect(),
        backgrounds: document.layers().iter().map(|l| l.is_background).collect(),
    };
    write(&data, output, cancel)
}
