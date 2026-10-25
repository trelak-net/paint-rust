//! Read and write physical resolution without copying descriptive source metadata.
use crate::resolution::Dpi;
use std::io::{self, Read, Seek, SeekFrom, Write};
fn valid(x: f64, y: f64) -> Option<Dpi> {
    let dpi = Dpi { x, y };
    dpi.valid().then_some(dpi)
}
fn bytes<const N: usize>(r: &mut impl Read) -> Option<[u8; N]> {
    let mut b = [0; N];
    r.read_exact(&mut b).ok()?;
    Some(b)
}

pub fn read(mut reader: impl Read + Seek) -> Option<Dpi> {
    let signature = bytes::<8>(&mut reader)?;
    if &signature == b"\x89PNG\r\n\x1a\n" {
        let mut scanned = 8u64;
        while scanned < 16 * 1024 * 1024 {
            let header = bytes::<8>(&mut reader)?;
            let length = u32::from_be_bytes(header[..4].try_into().ok()?) as u64;
            if &header[4..] == b"pHYs" && length == 9 {
                let data = bytes::<9>(&mut reader)?;
                if data[8] != 1 {
                    return None;
                }
                return valid(
                    f64::from(u32::from_be_bytes(data[..4].try_into().ok()?)) * 0.0254,
                    f64::from(u32::from_be_bytes(data[4..8].try_into().ok()?)) * 0.0254,
                );
            }
            if &header[4..] == b"IDAT" || length > 16 * 1024 * 1024 {
                return None;
            }
            reader.seek(SeekFrom::Current((length + 4) as i64)).ok()?;
            scanned += length + 12;
        }
    } else if &signature[..2] == b"BM" {
        reader.seek(SeekFrom::Start(14)).ok()?;
        if u32::from_le_bytes(bytes::<4>(&mut reader)?) < 40 {
            return None;
        }
        reader.seek(SeekFrom::Start(38)).ok()?;
        let x = i32::from_le_bytes(bytes::<4>(&mut reader)?);
        let y = i32::from_le_bytes(bytes::<4>(&mut reader)?);
        return valid(f64::from(x) * 0.0254, f64::from(y) * 0.0254);
    } else if signature[..2] == [255, 216] {
        reader.seek(SeekFrom::Start(2)).ok()?;
        let mut scanned = 2u64;
        while scanned < 16 * 1024 * 1024 {
            if bytes::<1>(&mut reader)?[0] != 255 {
                return None;
            }
            let mut marker = bytes::<1>(&mut reader)?[0];
            while marker == 255 {
                marker = bytes::<1>(&mut reader)?[0];
                scanned += 1;
                if scanned > 16 * 1024 * 1024 {
                    return None;
                }
            }
            if matches!(marker, 0xda | 0xd9) {
                return None;
            }
            if matches!(marker, 0x01 | 0xd0..=0xd7) {
                scanned += 2;
                continue;
            }
            let length = u16::from_be_bytes(bytes::<2>(&mut reader)?) as usize;
            if length < 2 {
                return None;
            }
            if marker == 0xe0 && length >= 16 {
                let data = bytes::<14>(&mut reader)?;
                if &data[..5] == b"JFIF\0" {
                    let factor = match data[7] {
                        1 => 1.0,
                        2 => 2.54,
                        _ => return None,
                    };
                    return valid(
                        f64::from(u16::from_be_bytes([data[8], data[9]])) * factor,
                        f64::from(u16::from_be_bytes([data[10], data[11]])) * factor,
                    );
                }
                reader.seek(SeekFrom::Current((length - 16) as i64)).ok()?;
            } else {
                reader.seek(SeekFrom::Current((length - 2) as i64)).ok()?;
            }
            scanned += length as u64 + 2;
        }
    } else if &signature[..2] == b"II" || &signature[..2] == b"MM" {
        reader.seek(SeekFrom::Start(0)).ok()?;
        let mut limits = tiff::decoder::Limits::default();
        limits.ifd_value_size = 1024 * 1024;
        let mut decoder = tiff::decoder::Decoder::new(reader)
            .ok()?
            .with_limits(limits);
        use tiff::tags::Tag;
        let unit = decoder
            .find_tag(Tag::ResolutionUnit)
            .ok()??
            .into_u16()
            .ok()?;
        let factor = match unit {
            2 => 1.0,
            3 => 2.54,
            _ => return None,
        };
        let rational = |v: tiff::decoder::ifd::Value| -> Option<f64> {
            let pair = v.into_u32_vec().ok()?;
            if pair.len() != 2 || pair[1] == 0 {
                None
            } else {
                Some(f64::from(pair[0]) / f64::from(pair[1]))
            }
        };
        return valid(
            rational(decoder.find_tag(Tag::XResolution).ok()??)? * factor,
            rational(decoder.find_tag(Tag::YResolution).ok()??)? * factor,
        );
    }
    None
}

pub fn exif(dpi: Dpi) -> Vec<u8> {
    let mut data = b"II\x2a\0\x08\0\0\0".to_vec();
    data.extend(3u16.to_le_bytes());
    for (tag, kind, value) in [(282u16, 5u16, 50u32), (283, 5, 58), (296, 3, 2)] {
        data.extend(tag.to_le_bytes());
        data.extend(kind.to_le_bytes());
        data.extend(1u32.to_le_bytes());
        data.extend(value.to_le_bytes());
    }
    data.extend(0u32.to_le_bytes());
    for dpi in [dpi.x, dpi.y] {
        data.extend(((dpi * 10000.0).round() as u32).to_le_bytes());
        data.extend(10000u32.to_le_bytes());
    }
    data
}
pub fn read_exif(bytes: &[u8]) -> Option<Dpi> {
    let data = bytes.strip_prefix(b"Exif\0\0").unwrap_or(bytes);
    if data.len() > 4 * 1024 * 1024 {
        return None;
    }
    let little = match data.get(..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let short = |offset: usize| -> Option<u16> {
        let b = data.get(offset..offset.checked_add(2)?)?.try_into().ok()?;
        Some(if little {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })
    };
    let word = |offset: usize| -> Option<u32> {
        let b = data.get(offset..offset.checked_add(4)?)?.try_into().ok()?;
        Some(if little {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    };
    if short(2)? != 42 {
        return None;
    }
    let start = word(4)? as usize;
    let count = short(start)? as usize;
    if count > 4096 {
        return None;
    }
    let mut x = None;
    let mut y = None;
    let mut unit = 2;
    for i in 0..count {
        let p = start.checked_add(2 + i * 12)?;
        if word(p + 4)? != 1 {
            continue;
        }
        match (short(p)?, short(p + 2)?) {
            (id @ (282 | 283), 5) => {
                let at = word(p + 8)? as usize;
                let n = word(at)?;
                let d = word(at + 4)?;
                if d != 0 {
                    if id == 282 {
                        x = Some(f64::from(n) / f64::from(d));
                    } else {
                        y = Some(f64::from(n) / f64::from(d));
                    }
                }
            }
            (296, 3) => unit = short(p + 8)?,
            _ => {}
        }
    }
    let factor = match unit {
        2 => 1.0,
        3 => 2.54,
        _ => return None,
    };
    valid(x? * factor, y? * factor)
}

pub struct HeaderWriter<W> {
    inner: W,
    prefix: Vec<u8>,
    dpi: Dpi,
    png: bool,
    ready: bool,
}
impl<W: Write> HeaderWriter<W> {
    pub fn new(inner: W, dpi: Dpi, png: bool) -> Self {
        Self {
            inner,
            prefix: Vec::new(),
            dpi,
            png,
            ready: false,
        }
    }
}
impl<W: Write> Write for HeaderWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.ready {
            return self.inner.write(bytes);
        }
        let required = if self.png { 33 } else { 46 };
        let n = bytes.len().min(required - self.prefix.len());
        self.prefix.extend_from_slice(&bytes[..n]);
        if self.prefix.len() == required {
            let x = (self.dpi.x / 0.0254).round() as u32;
            let y = (self.dpi.y / 0.0254).round() as u32;
            if !self.png {
                self.prefix[38..42].copy_from_slice(&x.to_le_bytes());
                self.prefix[42..46].copy_from_slice(&y.to_le_bytes());
            }
            self.inner.write_all(&self.prefix)?;
            if self.png {
                let mut chunk = b"pHYs".to_vec();
                chunk.extend(x.to_be_bytes());
                chunk.extend(y.to_be_bytes());
                chunk.push(1);
                let mut crc = 0xffffffffu32;
                for byte in &chunk {
                    crc ^= u32::from(*byte);
                    for _ in 0..8 {
                        crc = (crc >> 1) ^ if crc & 1 != 0 { 0xedb88320 } else { 0 };
                    }
                }
                self.inner.write_all(&9u32.to_be_bytes())?;
                self.inner.write_all(&chunk)?;
                self.inner.write_all(&(!crc).to_be_bytes())?;
            }
            self.ready = true;
            self.inner.write_all(&bytes[n..])?;
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
