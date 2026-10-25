use super::{Interchange, MODES, Metadata};
use std::{
    io::Write,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
enum Field {
    Int(i64, u8),
    Ref(i32),
}
enum Node {
    #[cfg(test)]
    Bytes(Vec<u8>),
    Object(&'static str, i32, Vec<(&'static str, Field)>),
    Text(String),
    Array(Vec<i32>),
    Metadata(Vec<(i32, i32)>),
}
struct Graph {
    nodes: Vec<Node>,
    inline: i32,
}
const PAIR: &str = "System.Collections.Generic.KeyValuePair`2[[System.String, mscorlib, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089],[System.String, mscorlib, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089]]";
fn int(n: impl Into<i64>) -> Field {
    Field::Int(n.into(), 8)
}
fn flag(b: bool) -> Field {
    Field::Int(i64::from(b), 1)
}
impl Graph {
    fn add(&mut self, node: Node) -> i32 {
        self.nodes.push(node);
        self.nodes.len() as i32 + 2
    }
    fn object(
        &mut self,
        name: &'static str,
        library: i32,
        fields: Vec<(&'static str, Field)>,
    ) -> Field {
        Field::Ref(self.add(Node::Object(name, library, fields)))
    }
    fn text(&mut self, text: &str) -> Field {
        Field::Ref(self.add(Node::Text(text.into())))
    }
    fn metadata(&mut self, items: &Metadata) -> Result<Field, String> {
        if items.len() > 4096
            || items
                .iter()
                .any(|(k, v)| k.len() > 1024 * 1024 || v.len() > 1024 * 1024)
        {
            return Err("Metadata exceeds size limits".into());
        }
        let mut pairs = Vec::new();
        for (k, v) in items {
            let key = self.add(Node::Text(k.clone()));
            let value = self.add(Node::Text(v.clone()));
            pairs.push((key, value));
        }
        Ok(Field::Ref(self.add(Node::Metadata(pairs))))
    }
    fn build(data: &Interchange) -> Result<Self, String> {
        let doc = &data.document;
        let metadata_bytes: usize = std::iter::once(&data.metadata)
            .chain(data.layer_metadata.iter())
            .flat_map(|m| m.iter())
            .map(|(k, v)| k.len().saturating_add(v.len()).saturating_add(128))
            .sum();
        if metadata_bytes > 8 * 1024 * 1024 {
            return Err("Metadata exceeds encoded graph limit".into());
        }
        if data.layer_metadata.len() != doc.layers().len()
            || data.backgrounds.len() != doc.layers().len()
        {
            return Err("Layer metadata count mismatch".into());
        }
        let mut g = Self {
            nodes: vec![Node::Array(vec![])],
            inline: -1,
        };
        let mut layers = Vec::new();
        for (index, layer) in doc.layers().iter().enumerate() {
            let memory = g.object(
                "PaintDotNet.MemoryBlock",
                2,
                vec![
                    (
                        "length64",
                        Field::Int(i64::from(doc.width) * i64::from(doc.height) * 4, 9),
                    ),
                    ("hasParent", flag(false)),
                    ("deferred", flag(true)),
                ],
            );
            let surface = g.object(
                "PaintDotNet.Surface",
                2,
                vec![
                    ("width", int(doc.width)),
                    ("height", int(doc.height)),
                    ("stride", int(doc.width * 4)),
                    ("scan0", memory),
                ],
            );
            let legacy = g.object("PaintDotNet.BitmapLayer+BitmapLayerProperties", 1, vec![]);
            let mode = MODES
                .iter()
                .position(|m| *m == layer.blend_mode)
                .ok_or("Unsupported blend mode")?;
            let mode = g.object(
                "PaintDotNet.LayerBlendMode",
                1,
                vec![("value__", int(mode as i32))],
            );
            let name = g.text(&layer.name);
            let metadata = g.metadata(&data.layer_metadata[index])?;
            let properties = g.object(
                "PaintDotNet.Layer+LayerProperties",
                1,
                vec![
                    ("name", name),
                    ("visible", flag(layer.visible)),
                    ("isBackground", flag(data.backgrounds[index])),
                    ("opacity", Field::Int(i64::from(layer.opacity), 2)),
                    ("blendMode", mode),
                    ("userMetadataItems", metadata),
                ],
            );
            let layer = g.add(Node::Object(
                "PaintDotNet.BitmapLayer",
                1,
                vec![
                    ("properties", legacy),
                    ("surface", surface),
                    ("Layer+isDisposed", flag(false)),
                    ("Layer+width", int(doc.width)),
                    ("Layer+height", int(doc.height)),
                    ("Layer+properties", properties),
                ],
            ));
            layers.push(layer);
        }
        let array = Field::Ref(g.add(Node::Array(layers)));
        let list = g.object(
            "PaintDotNet.LayerList",
            1,
            vec![
                ("parent", Field::Ref(3)),
                ("ArrayList+_items", array),
                ("ArrayList+_size", int(doc.layers().len() as i32)),
                ("ArrayList+_version", int(doc.layers().len() as i32)),
            ],
        );
        let version = g.object(
            "System.Version",
            0,
            vec![
                ("_Major", int(5)),
                ("_Minor", int(0)),
                ("_Build", int(0)),
                ("_Revision", int(0)),
            ],
        );
        let metadata = g.metadata(&data.metadata)?;
        g.nodes[0] = Node::Object(
            "PaintDotNet.Document",
            1,
            vec![
                ("isDisposed", flag(false)),
                ("layers", list),
                ("width", int(doc.width)),
                ("height", int(doc.height)),
                ("savedWith", version),
                ("userMetadataItems", metadata),
            ],
        );
        Ok(g)
    }
    fn write(mut self, w: &mut impl Write) -> std::io::Result<()> {
        w.write_all(&[0])?;
        i32write(w, 3)?;
        i32write(w, -1)?;
        i32write(w, 1)?;
        i32write(w, 0)?;
        for (id, name) in [(1, "PaintDotNet.Data"), (2, "PaintDotNet.Core")] {
            w.write_all(&[12])?;
            i32write(w, id)?;
            string(
                w,
                &format!("{name}, Version=5.0.0.0, Culture=neutral, PublicKeyToken=null"),
            )?;
        }
        for (index, node) in self.nodes.iter().enumerate() {
            let id = index as i32 + 3;
            match node {
                #[cfg(test)]
                Node::Bytes(bytes) => {
                    w.write_all(&[15])?;
                    i32write(w, id)?;
                    i32write(w, bytes.len() as i32)?;
                    w.write_all(&[2])?;
                    w.write_all(bytes)?;
                }
                Node::Object(name, library, fields) => object(w, id, name, *library, fields)?,
                Node::Text(text) => {
                    w.write_all(&[6])?;
                    i32write(w, id)?;
                    string(w, text)?;
                }
                Node::Array(items) => {
                    w.write_all(&[16])?;
                    i32write(w, id)?;
                    i32write(w, items.len() as i32)?;
                    for id in items {
                        field(w, &Field::Ref(*id))?;
                    }
                }
                Node::Metadata(pairs) => {
                    w.write_all(&[7])?;
                    i32write(w, id)?;
                    w.write_all(&[0])?;
                    i32write(w, 1)?;
                    i32write(w, pairs.len() as i32)?;
                    w.write_all(&[3])?;
                    string(w, PAIR)?;
                    for (key, value) in pairs {
                        object(
                            w,
                            self.inline,
                            PAIR,
                            0,
                            &[("key", Field::Ref(*key)), ("value", Field::Ref(*value))],
                        )?;
                        self.inline -= 1;
                    }
                }
            }
        }
        w.write_all(&[11])
    }
}
fn i32write(w: &mut impl Write, n: i32) -> std::io::Result<()> {
    w.write_all(&n.to_le_bytes())
}
fn string(w: &mut impl Write, text: &str) -> std::io::Result<()> {
    let mut length = text.len();
    while length >= 128 {
        w.write_all(&[(length as u8 & 127) | 128])?;
        length >>= 7;
    }
    w.write_all(&[length as u8])?;
    w.write_all(text.as_bytes())
}
fn field(w: &mut impl Write, value: &Field) -> std::io::Result<()> {
    match value {
        Field::Ref(id) => {
            w.write_all(&[9])?;
            i32write(w, *id)
        }
        Field::Int(n, kind) => match kind {
            1 | 2 => w.write_all(&[*n as u8]),
            8 => i32write(w, *n as i32),
            9 => w.write_all(&n.to_le_bytes()),
            _ => unreachable!(),
        },
    }
}
fn object(
    w: &mut impl Write,
    id: i32,
    name: &str,
    library: i32,
    fields: &[(&str, Field)],
) -> std::io::Result<()> {
    w.write_all(&[if library == 0 { 4 } else { 5 }])?;
    i32write(w, id)?;
    string(w, name)?;
    i32write(w, fields.len() as i32)?;
    for (name, _) in fields {
        string(w, name)?;
    }
    for (_, value) in fields {
        w.write_all(&[if matches!(value, Field::Int(..)) {
            0
        } else {
            2
        }])?;
    }
    for (_, value) in fields {
        if let Field::Int(_, kind) = value {
            w.write_all(&[*kind])?;
        }
    }
    if library != 0 {
        i32write(w, library)?;
    }
    for (_, value) in fields {
        field(w, value)?;
    }
    Ok(())
}
struct Checked<'a, W> {
    inner: W,
    cancel: &'a AtomicBool,
    count: usize,
}
impl<W: Write> Write for Checked<'_, W> {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::other(crate::files::CANCELLED));
        }
        if self.count.saturating_add(b.len()) > 512 * 1024 * 1024 {
            return Err(std::io::Error::other("Encoded document exceeds size limit"));
        }
        let n = self.inner.write(b)?;
        self.count += n;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
pub fn write(data: &Interchange, output: impl Write, cancel: &AtomicBool) -> Result<(), String> {
    let graph = Graph::build(data)?;
    let mut output = Checked {
        inner: output,
        cancel,
        count: 0,
    };
    let encode = |output: &mut Checked<'_, _>| -> std::io::Result<()> {
        let doc = &data.document;
        let header = format!(
            "<pdnImage width=\"{}\" height=\"{}\" layers=\"{}\"><custom /></pdnImage>",
            doc.width,
            doc.height,
            doc.layers().len()
        );
        output.write_all(b"PDN3")?;
        output.write_all(&(header.len() as u32).to_le_bytes()[..3])?;
        output.write_all(header.as_bytes())?;
        output.write_all(&[0, 1])?;
        graph.write(output)?;
        for layer in doc.layers() {
            output.write_all(&[0])?;
            output.write_all(&262144u32.to_be_bytes())?;
            for (index, pixels) in layer.pixels().chunks(65536).enumerate() {
                if cancel.load(Ordering::Relaxed) {
                    return Err(std::io::Error::other(crate::files::CANCELLED));
                }
                let mut raw = Vec::with_capacity(pixels.len() * 4);
                for pixel in pixels {
                    raw.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
                }
                let mut gzip =
                    flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
                gzip.write_all(&raw)?;
                let compressed = gzip.finish()?;
                output.write_all(&(index as u32).to_be_bytes())?;
                output.write_all(&(compressed.len() as u32).to_be_bytes())?;
                output.write_all(&compressed)?;
            }
        }
        output.flush()
    };
    encode(&mut output).map_err(|e| e.to_string())
}

#[cfg(test)]
mod legacy_tests {
    use super::*;
    fn fixture(kind: u8, header: bool) -> (Interchange, Vec<u8>) {
        let document = crate::document::Document::from_pixels(
            3,
            2,
            vec![
                [10, 20, 30, 0],
                [40, 50, 60, 100],
                [70, 80, 90, 255],
                [100, 110, 120, 1],
                [130, 140, 150, 128],
                [160, 170, 180, 254],
            ],
        )
        .unwrap();
        let data = Interchange {
            document,
            metadata: Metadata::new(),
            layer_metadata: vec![Metadata::new()],
            backgrounds: vec![false],
        };
        let mut graph = Graph::build(&data).unwrap();
        let index = graph
            .nodes
            .iter()
            .position(|n| matches!(n, Node::Object("PaintDotNet.MemoryBlock", _, _)))
            .unwrap();
        let raw: Vec<_> = data
            .document
            .pixels()
            .iter()
            .flat_map(|p| [p[2], p[1], p[0], p[3]])
            .collect();
        let fields = if kind == 1 {
            let a = graph.add(Node::Bytes(raw[..12].to_vec()));
            let b = graph.add(Node::Bytes(raw[12..].to_vec()));
            vec![
                ("length", int(24)),
                ("hasParent", flag(false)),
                ("chunkCount", Field::Int(2, 8)),
                ("chunkFormat", int(0)),
                ("chunk0", Field::Ref(a)),
                ("chunk1", Field::Ref(b)),
            ]
        } else if kind >= 2 {
            let mut parent = vec![99; 8];
            parent.extend(&raw);
            parent.extend([99; 8]);
            let bytes = graph.add(Node::Bytes(parent));
            let parent = graph.object(
                "PaintDotNet.MemoryBlock",
                2,
                vec![
                    ("length64", Field::Int(40, 9)),
                    ("hasParent", flag(false)),
                    ("pointerData", Field::Ref(bytes)),
                ],
            );
            if kind == 4
                && let Field::Ref(id) = &parent
            {
                graph.nodes[*id as usize - 3] = Node::Object(
                    "PaintDotNet.MemoryBlock",
                    2,
                    vec![
                        ("length64", Field::Int(40, 9)),
                        ("hasParent", flag(true)),
                        ("parentBlock", Field::Ref(index as i32 + 3)),
                        ("parentOffset64", Field::Int(0, 9)),
                    ],
                );
            }
            vec![
                ("length64", Field::Int(24, 9)),
                ("hasParent", flag(true)),
                ("parentBlock", parent),
                (
                    "parentOffset64",
                    Field::Int(if kind == 3 { 20 } else { 8 }, 9),
                ),
            ]
        } else {
            let bytes = graph.add(Node::Bytes(raw));
            vec![
                ("length", int(24)),
                ("hasParent", flag(false)),
                ("pointerData", Field::Ref(bytes)),
            ]
        };
        graph.nodes[index] = Node::Object("PaintDotNet.MemoryBlock", 2, fields);
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        graph.write(&mut gzip).unwrap();
        let compressed = gzip.finish().unwrap();
        let mut bytes = Vec::new();
        if header {
            let xml = b"<pdnImage width=\"3\" height=\"2\" layers=\"1\"><custom /></pdnImage>";
            bytes.extend(b"PDN3");
            bytes.extend(&(xml.len() as u32).to_le_bytes()[..3]);
            bytes.extend(xml);
        }
        bytes.extend(compressed);
        (data, bytes)
    }
    #[test]
    fn compressed_inline_and_parent_backed_graphs_roundtrip() {
        for kind in 0..3 {
            for header in [true, false] {
                let (data, bytes) = fixture(kind, header);
                let loaded = super::super::read(bytes.as_slice(), &AtomicBool::new(false)).unwrap();
                assert!(loaded.document.layers() == data.document.layers());
                if let Some(root) = std::env::var_os("PAINT_RUST_LEGACY_FIXTURES") {
                    std::fs::create_dir_all(&root).unwrap();
                    std::fs::write(
                        std::path::Path::new(&root).join(format!("legacy-{kind}-{header}.pdn")),
                        &bytes,
                    )
                    .unwrap();
                }
                let mut corrupt = bytes.clone();
                let index = corrupt.len() - 8;
                corrupt[index] ^= 1;
                assert!(super::super::read(corrupt.as_slice(), &AtomicBool::new(false)).is_err());
                assert!(
                    super::super::read(&bytes[..bytes.len() - 1], &AtomicBool::new(false)).is_err()
                );
                assert!(super::super::read(bytes.as_slice(), &AtomicBool::new(true)).is_err());
            }
        }
        let (_, cycle) = fixture(4, true);
        assert!(
            super::super::read(cycle.as_slice(), &AtomicBool::new(false))
                .err()
                .unwrap()
                .contains("Cyclic")
        );
        let (_, bytes) = fixture(3, true);
        assert!(
            super::super::read(bytes.as_slice(), &AtomicBool::new(false))
                .err()
                .unwrap()
                .contains("parent buffer")
        );
    }
}
