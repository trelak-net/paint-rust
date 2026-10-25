use crate::document::{BlendMode, Document, Layer};
use image::{
    ImageDecoder, ImageEncoder, ImageReader,
    codecs::{
        bmp::BmpEncoder, gif::GifEncoder, jpeg::JpegEncoder, png::PngEncoder, tga::TgaEncoder,
        tiff::TiffEncoder, webp::WebPEncoder,
    },
};
use std::{
    fs::File,
    io::{BufReader, BufWriter, Cursor, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

const PROJECT_MAGIC: &[u8; 8] = b"PRUST\0\x02\0";

pub fn open(path: &Path) -> Result<Document, String> {
    open_cancellable(path, &AtomicBool::new(false))
}

pub fn open_cancellable(path: &Path, cancel: &AtomicBool) -> Result<Document, String> {
    check_cancel(cancel)?;
    let mut header = [0; 8];
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let read = file.read(&mut header).map_err(|e| e.to_string())?;
    if (read >= 3 && &header[..3] == b"PDN")
        || path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("pdn"))
    {
        return crate::pdn::read_document(
            BufReader::new(File::open(path).map_err(|e| e.to_string())?),
            cancel,
        );
    }
    if (read >= 5 && &header[..5] == b"PRUST")
        || path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("prust"))
    {
        return open_project_cancellable(path, cancel);
    }
    if (read >= 4 && &header[..4] == b"DDS ")
        || path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("dds"))
    {
        return crate::dds::decode(
            CancellableReader {
                inner: File::open(path).map_err(|e| e.to_string())?,
                cancel,
            },
            cancel,
        );
    }
    let dimensions = raster_reader(path, cancel)?
        .into_dimensions()
        .map_err(|e| e.to_string())?;
    Document::validate_size(dimensions.0, dimensions.1)?;
    let mut reader = raster_reader(path, cancel)?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    reader.limits(limits);
    let is_tiff = reader.format() == Some(image::ImageFormat::Tiff);
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    if decoder.total_bytes() > 256 * 1024 * 1024 {
        return Err("Decoded image exceeds the memory limit.".into());
    }
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let profile = if is_tiff {
        // Read metadata with an independent bound: the image wrapper limits TIFF
        // tag reads to the pixel buffer size, which excludes profiles on tiny images.
        let mut metadata = tiff::decoder::Decoder::new(BufReader::new(CancellableReader {
            inner: File::open(path).map_err(|e| e.to_string())?,
            cancel,
        }))
        .map_err(|e| e.to_string())?
        .with_limits(tiff_metadata_limits());
        metadata
            .find_tag(tiff::tags::Tag::IccProfile)
            .map_err(|e| e.to_string())?
            .map(|value| value.into_u8_vec().map_err(|e| e.to_string()))
            .transpose()?
    } else {
        decoder.icc_profile().map_err(|e| e.to_string())?
    };
    check_cancel(cancel)?;
    let image = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let (mut width, mut height) = image.dimensions();
    let mut pixels = collect_pixels(&image, cancel)?;
    drop(image);
    if let Some(profile) = profile {
        crate::color_management::convert_to_srgb(&mut pixels, &profile, cancel)?;
    }
    use crate::transform::Orientation as O;
    use image::metadata::Orientation as E;
    let operations: &[O] = match orientation {
        E::NoTransforms => &[],
        E::Rotate90 => &[O::Clockwise],
        E::Rotate180 => &[O::HalfTurn],
        E::Rotate270 => &[O::Counterclockwise],
        E::FlipHorizontal => &[O::FlipHorizontal],
        E::FlipVertical => &[O::FlipVertical],
        E::Rotate90FlipH => &[O::Clockwise, O::FlipHorizontal],
        E::Rotate270FlipH => &[O::Counterclockwise, O::FlipHorizontal],
    };
    for operation in operations {
        pixels = crate::transform::orient_cancellable(&pixels, width, height, *operation, cancel)
            .ok_or(CANCELLED)?;
        if matches!(operation, O::Clockwise | O::Counterclockwise) {
            std::mem::swap(&mut width, &mut height);
        }
    }
    check_cancel(cancel)?;
    Document::from_pixels(width, height, pixels)
}

fn tiff_metadata_limits() -> tiff::decoder::Limits {
    let mut limits = tiff::decoder::Limits::default();
    limits.decoding_buffer_size = 4 * 1024 * 1024;
    limits.intermediate_buffer_size = 4 * 1024 * 1024;
    limits.ifd_value_size = 4 * 1024 * 1024;
    limits
}

fn raster_reader<'a>(
    path: &Path,
    cancel: &'a AtomicBool,
) -> Result<ImageReader<BufReader<CancellableReader<'a, File>>>, String> {
    let mut reader = ImageReader::new(BufReader::new(CancellableReader {
        inner: File::open(path).map_err(|e| e.to_string())?,
        cancel,
    }));
    // Formats without a signature, such as TGA, need an extension hint.
    if let Ok(format) = image::ImageFormat::from_path(path) {
        reader.set_format(format);
    }
    reader.with_guessed_format().map_err(|e| e.to_string())
}

pub const CANCELLED: &str = "Operation cancelled";

fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err(CANCELLED.into())
    } else {
        Ok(())
    }
}

// Bound each read so cancellation is checked even for large payload requests.
struct CancellableReader<'a, R> {
    inner: R,
    cancel: &'a AtomicBool,
}
impl<R: Read> Read for CancellableReader<'_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::other(CANCELLED));
        }
        let length = buffer.len().min(64 * 1024);
        self.inner.read(&mut buffer[..length])
    }
}
impl<R: Seek> Seek for CancellableReader<'_, R> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::other(CANCELLED));
        }
        self.inner.seek(position)
    }
}

fn collect_pixels(image: &image::RgbaImage, cancel: &AtomicBool) -> Result<Vec<[u8; 4]>, String> {
    check_cancel(cancel)?;
    let mut pixels = Vec::with_capacity(image.width() as usize * image.height() as usize);
    for row in image.rows() {
        check_cancel(cancel)?;
        pixels.extend(row.map(|pixel| pixel.0));
    }
    check_cancel(cancel)?;
    Ok(pixels)
}

struct CancellableWriter<'a, W> {
    inner: W,
    cancel: &'a AtomicBool,
}
impl<W: Write> Write for CancellableWriter<'_, W> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::other(CANCELLED));
        }
        self.inner.write(&buffer[..buffer.len().min(64 * 1024)])
    }
    fn flush(&mut self) -> std::io::Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::other(CANCELLED));
        }
        self.inner.flush()
    }
}

impl<W: Seek> Seek for CancellableWriter<'_, W> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::other(CANCELLED));
        }
        self.inner.seek(position)
    }
}

fn write_atomic(
    path: &Path,
    cancel: &AtomicBool,
    write: impl FnOnce(&mut File) -> Result<(), String>,
) -> Result<(), String> {
    check_cancel(cancel)?;
    if let Ok(metadata) = std::fs::metadata(path)
        && metadata.permissions().readonly()
    {
        return Err("The destination is read-only.".into());
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    write(temp.as_file_mut())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    check_cancel(cancel)?;
    temp.persist(path).map_err(|e| e.error.to_string())?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RasterFormat {
    Dds,
    Gif,
    Tiff,
    Tga,
    Jpeg,
    Bmp,
    Png,
    WebP,
}
impl RasterFormat {
    pub const ALL: [Self; 8] = [
        Self::Png,
        Self::Jpeg,
        Self::Bmp,
        Self::WebP,
        Self::Tiff,
        Self::Tga,
        Self::Gif,
        Self::Dds,
    ];
    pub fn from_path(path: &Path) -> Option<Self> {
        let extension = path.extension()?;
        Self::ALL.into_iter().find(|format| {
            format
                .extensions()
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Gif => "gif",
            Self::Dds => "dds",
            Self::Tiff => "tif",
            Self::Tga => "tga",
            Self::Jpeg => "jpg",
            Self::Bmp => "bmp",
            Self::Png => "png",
            Self::WebP => "webp",
        }
    }
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Gif => &["gif"],
            Self::Dds => &["dds"],
            Self::Tiff => &["tif", "tiff"],
            Self::Tga => &["tga"],
            Self::Jpeg => &["jpg", "jpeg"],
            Self::Bmp => &["bmp"],
            Self::Png => &["png"],
            Self::WebP => &["webp"],
        }
    }
    pub fn image_format(self) -> image::ImageFormat {
        match self {
            Self::Gif => image::ImageFormat::Gif,
            Self::Dds => image::ImageFormat::Dds,
            Self::Tiff => image::ImageFormat::Tiff,
            Self::Tga => image::ImageFormat::Tga,
            Self::Jpeg => image::ImageFormat::Jpeg,
            Self::Bmp => image::ImageFormat::Bmp,
            Self::Png => image::ImageFormat::Png,
            Self::WebP => image::ImageFormat::WebP,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Gif => "GIF (single frame)",
            Self::Dds => "DDS (RGBA8)",
            Self::Tiff => "TIFF",
            Self::Tga => "TGA",
            Self::Jpeg => "JPEG",
            Self::Bmp => "BMP",
            Self::Png => "PNG",
            Self::WebP => "WebP (lossless)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RasterOptions {
    pub format: RasterFormat,
    pub quality: u8,
    pub matte: [u8; 3],
}
impl RasterOptions {
    pub fn new(format: RasterFormat) -> Self {
        Self {
            format,
            quality: 90,
            matte: [255; 3],
        }
    }
}

pub fn encode_raster_bytes(
    pixels: &[[u8; 4]],
    size: (u32, u32),
    options: RasterOptions,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    encode_raster_options(pixels, size, &mut bytes, cancel, options)?;
    check_cancel(cancel)?;
    Ok(bytes)
}

pub fn decode_raster_bytes(
    bytes: &[u8],
    format: RasterFormat,
    cancel: &AtomicBool,
) -> Result<Document, String> {
    check_cancel(cancel)?;
    if format == RasterFormat::Dds {
        return crate::dds::decode(Cursor::new(bytes), cancel);
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format.image_format());
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    reader.limits(limits);
    let decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let (width, height) = decoder.dimensions();
    Document::validate_size(width, height)?;
    let image = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let pixels = collect_pixels(&image, cancel)?;
    check_cancel(cancel)?;
    Document::from_pixels(width, height, pixels)
}

pub fn save_encoded_raster(bytes: &[u8], path: &Path, cancel: &AtomicBool) -> Result<(), String> {
    write_atomic(path, cancel, |file| {
        let mut writer = CancellableWriter {
            inner: file,
            cancel,
        };
        writer.write_all(bytes).map_err(|error| error.to_string())
    })
}

fn encode_png(
    pixels: &[[u8; 4]],
    width: u32,
    height: u32,
    writer: impl Write,
    cancel: &AtomicBool,
) -> Result<(), String> {
    encode_raster(pixels, (width, height), writer, cancel, RasterFormat::Png)
}

fn encode_raster(
    pixels: &[[u8; 4]],
    size: (u32, u32),
    writer: impl Write,
    cancel: &AtomicBool,
    format: RasterFormat,
) -> Result<(), String> {
    encode_raster_options(pixels, size, writer, cancel, RasterOptions::new(format))
}

fn encode_raster_options(
    pixels: &[[u8; 4]],
    size: (u32, u32),
    writer: impl Write,
    cancel: &AtomicBool,
    options: RasterOptions,
) -> Result<(), String> {
    check_cancel(cancel)?;
    Document::validate_size(size.0, size.1)?;
    if pixels.len() != size.0 as usize * size.1 as usize || !(1..=100).contains(&options.quality) {
        return Err("Invalid raster export pixels or quality.".into());
    }
    if options.format == RasterFormat::Dds {
        return crate::dds::encode(
            pixels,
            size,
            CancellableWriter {
                inner: writer,
                cancel,
            },
            cancel,
        );
    }
    let opaque = matches!(options.format, RasterFormat::Jpeg | RasterFormat::Bmp);
    let mut bytes = Vec::with_capacity(pixels.len() * if opaque { 3 } else { 4 });
    for chunk in pixels.chunks(4096) {
        check_cancel(cancel)?;
        if options.format == RasterFormat::Gif {
            for pixel in chunk {
                if pixel[3] < 128 {
                    bytes.extend([0; 4]);
                } else {
                    let alpha = u32::from(pixel[3]);
                    bytes.extend((0..3).map(|c| {
                        ((u32::from(pixel[c]) * alpha
                            + u32::from(options.matte[c]) * (255 - alpha)
                            + 127)
                            / 255) as u8
                    }));
                    bytes.push(255);
                }
            }
        } else if opaque {
            for pixel in chunk {
                let alpha = u32::from(pixel[3]);
                bytes.extend((0..3).map(|c| {
                    ((u32::from(pixel[c]) * alpha
                        + u32::from(options.matte[c]) * (255 - alpha)
                        + 127)
                        / 255) as u8
                }));
            }
        } else {
            bytes.extend(chunk.iter().flatten().copied());
        }
    }
    let mut writer = CancellableWriter {
        inner: writer,
        cancel,
    };
    match options.format {
        RasterFormat::Dds => unreachable!("DDS encoded above"),
        RasterFormat::Gif => {
            GifEncoder::new(writer).encode(&bytes, size.0, size.1, image::ExtendedColorType::Rgba8)
        }
        RasterFormat::Tiff => {
            let mut encoded = Cursor::new(Vec::new());
            with_srgb_profile(TiffEncoder::new(CancellableWriter {
                inner: &mut encoded,
                cancel,
            }))?
            .write_image(&bytes, size.0, size.1, image::ExtendedColorType::Rgba8)
            .map_err(|e| e.to_string())?;
            check_cancel(cancel)?;
            return writer
                .write_all(encoded.get_ref())
                .map_err(|e| e.to_string());
        }
        RasterFormat::Tga => TgaEncoder::new(writer).write_image(
            &bytes,
            size.0,
            size.1,
            image::ExtendedColorType::Rgba8,
        ),
        RasterFormat::Jpeg => with_srgb_profile(JpegEncoder::new_with_quality(
            writer,
            options.quality,
        ))?
        .encode(&bytes, size.0, size.1, image::ExtendedColorType::Rgb8),
        RasterFormat::Bmp => BmpEncoder::new(&mut writer).encode(
            &bytes,
            size.0,
            size.1,
            image::ExtendedColorType::Rgb8,
        ),
        RasterFormat::Png => with_srgb_profile(PngEncoder::new(writer))?.write_image(
            &bytes,
            size.0,
            size.1,
            image::ExtendedColorType::Rgba8,
        ),
        RasterFormat::WebP => with_srgb_profile(WebPEncoder::new_lossless(writer))?.write_image(
            &bytes,
            size.0,
            size.1,
            image::ExtendedColorType::Rgba8,
        ),
    }
    .map_err(|e| e.to_string())
}

fn with_srgb_profile<E: ImageEncoder>(mut encoder: E) -> Result<E, String> {
    encoder
        .set_icc_profile(crate::color_management::srgb_profile()?)
        .map_err(|e| e.to_string())?;
    Ok(encoder)
}

pub fn save_png(document: &Document, path: &Path) -> Result<(), String> {
    save_png_cancellable(document, path, &AtomicBool::new(false))
}

pub fn save_png_cancellable(
    document: &Document,
    path: &Path,
    cancel: &AtomicBool,
) -> Result<(), String> {
    save_raster_cancellable(document, path, RasterFormat::Png, cancel)
}

pub fn save_raster_cancellable(
    document: &Document,
    path: &Path,
    format: RasterFormat,
    cancel: &AtomicBool,
) -> Result<(), String> {
    write_atomic(path, cancel, |file| {
        let mut writer = BufWriter::new(CancellableWriter {
            inner: file,
            cancel,
        });
        encode_raster(
            &document.composite_cancellable(cancel).ok_or(CANCELLED)?,
            (document.width, document.height),
            &mut writer,
            cancel,
            format,
        )?;
        writer.flush().map_err(|e| e.to_string())
    })
}

pub fn save_project(document: &Document, path: &Path) -> Result<(), String> {
    save_project_cancellable(document, path, &AtomicBool::new(false))
}

pub fn save_project_cancellable(
    document: &Document,
    path: &Path,
    cancel: &AtomicBool,
) -> Result<(), String> {
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdn"))
    {
        return write_atomic(path, cancel, |file| {
            crate::pdn::write_document(document, file, cancel)
        });
    }
    let has_metadata = !document.metadata.is_empty()
        || document
            .layers()
            .iter()
            .any(|l| l.is_background || !l.metadata.is_empty());
    write_atomic(path, cancel, |file| {
        let mut writer = BufWriter::new(CancellableWriter {
            inner: file,
            cancel,
        });
        let mut magic = *PROJECT_MAGIC;
        if has_metadata {
            magic[6] = 3;
        }
        writer.write_all(&magic).map_err(|e| e.to_string())?;
        for value in [
            document.width,
            document.height,
            document.layers().len() as u32,
            document.active_layer() as u32,
        ] {
            writer
                .write_all(&value.to_le_bytes())
                .map_err(|e| e.to_string())?;
        }
        for layer in document.layers() {
            check_cancel(cancel)?;
            writer
                .write_all(&(layer.name.len() as u32).to_le_bytes())
                .map_err(|e| e.to_string())?;
            writer
                .write_all(layer.name.as_bytes())
                .map_err(|e| e.to_string())?;
            writer
                .write_all(&[
                    u8::from(layer.visible),
                    layer.opacity,
                    layer.blend_mode as u8,
                ])
                .map_err(|e| e.to_string())?;
            let mut png = Vec::new();
            encode_png(
                layer.pixels(),
                document.width,
                document.height,
                &mut png,
                cancel,
            )?;
            writer
                .write_all(&(png.len() as u64).to_le_bytes())
                .map_err(|e| e.to_string())?;
            writer.write_all(&png).map_err(|e| e.to_string())?;
        }
        if has_metadata {
            let mut budget = 16 * 1024 * 1024;
            write_metadata(&mut writer, &document.metadata, &mut budget)?;
            for layer in document.layers() {
                writer
                    .write_all(&[u8::from(layer.is_background)])
                    .map_err(|e| e.to_string())?;
                write_metadata(&mut writer, &layer.metadata, &mut budget)?;
            }
        }
        writer.flush().map_err(|e| e.to_string())
    })
}

fn read_array<const N: usize>(reader: &mut impl Read) -> Result<[u8; N], String> {
    let mut bytes = [0; N];
    reader.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

pub fn open_project(path: &Path) -> Result<Document, String> {
    open_project_cancellable(path, &AtomicBool::new(false))
}

pub fn open_project_cancellable(path: &Path, cancel: &AtomicBool) -> Result<Document, String> {
    check_cancel(cancel)?;
    let mut reader = BufReader::new(CancellableReader {
        inner: File::open(path).map_err(|e| e.to_string())?,
        cancel,
    });
    let magic = read_array::<8>(&mut reader)?;
    if magic[..6] != PROJECT_MAGIC[..6] || magic[7] != 0 || !matches!(magic[6], 1..=3) {
        return Err("Unsupported project signature or version.".into());
    }
    let width = u32::from_le_bytes(read_array(&mut reader)?);
    let height = u32::from_le_bytes(read_array(&mut reader)?);
    let count = u32::from_le_bytes(read_array(&mut reader)?) as usize;
    let active = u32::from_le_bytes(read_array(&mut reader)?) as usize;
    Document::validate_size(width, height)?;
    if count == 0
        || count > 32
        || active >= count
        || count as u64 * u64::from(width) * u64::from(height) * 4 > 256 * 1024 * 1024
    {
        return Err("Invalid layer count, active layer or document memory limit.".into());
    }
    let mut layers = Vec::new();
    for _ in 0..count {
        check_cancel(cancel)?;
        let length = u32::from_le_bytes(read_array(&mut reader)?) as usize;
        if length > 4096 {
            return Err("Layer name is too long.".into());
        }
        let mut name = vec![0; length];
        reader.read_exact(&mut name).map_err(|e| e.to_string())?;
        let name = String::from_utf8(name).map_err(|e| e.to_string())?;
        let flags = read_array::<2>(&mut reader)?;
        if flags[0] > 1 {
            return Err("Invalid layer visibility.".into());
        }
        let mode = if magic[6] >= 2 {
            let index = read_array::<1>(&mut reader)?[0] as usize;
            *BlendMode::ALL
                .get(index)
                .ok_or("Unsupported layer blend mode")?
        } else {
            BlendMode::Normal
        };
        let size = u64::from_le_bytes(read_array(&mut reader)?);
        if size > 128 * 1024 * 1024 {
            return Err("Layer payload exceeds the memory limit.".into());
        }
        let mut png = Vec::new();
        (&mut reader)
            .take(size)
            .read_to_end(&mut png)
            .map_err(|e| e.to_string())?;
        if png.len() as u64 != size {
            return Err("Truncated layer payload.".into());
        }
        let dimensions = ImageReader::with_format(Cursor::new(&png), image::ImageFormat::Png)
            .into_dimensions()
            .map_err(|e| e.to_string())?;
        if dimensions != (width, height) {
            return Err("Layer dimensions do not match the document.".into());
        }
        let mut image_reader = ImageReader::with_format(
            BufReader::new(CancellableReader {
                inner: Cursor::new(png),
                cancel,
            }),
            image::ImageFormat::Png,
        );
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(128 * 1024 * 1024);
        image_reader.limits(limits);
        let image = image_reader.decode().map_err(|e| e.to_string())?.to_rgba8();
        let mut layer = Layer::new(name, collect_pixels(&image, cancel)?);
        layer.visible = flags[0] != 0;
        layer.opacity = flags[1];
        layer.blend_mode = mode;
        layers.push(layer);
    }
    let metadata = if magic[6] == 3 {
        let mut budget = 16 * 1024 * 1024;
        let metadata = read_metadata(&mut reader, &mut budget)?;
        for layer in &mut layers {
            layer.is_background = match read_array::<1>(&mut reader)?[0] {
                0 => false,
                1 => true,
                _ => return Err("Invalid background flag".into()),
            };
            layer.metadata = read_metadata(&mut reader, &mut budget)?;
        }
        metadata
    } else {
        Default::default()
    };
    if reader.read(&mut [0; 1]).map_err(|e| e.to_string())? != 0 {
        return Err("Unexpected data after the project.".into());
    }
    check_cancel(cancel)?;
    let mut document = Document::from_layers(width, height, layers, active)?;
    document.metadata = metadata;
    Ok(document)
}

fn write_metadata(
    writer: &mut impl Write,
    metadata: &std::collections::BTreeMap<String, String>,
    budget: &mut usize,
) -> Result<(), String> {
    if metadata.len() > 4096 {
        return Err("Too many metadata entries".into());
    }
    writer
        .write_all(&(metadata.len() as u32).to_le_bytes())
        .map_err(|e| e.to_string())?;
    for (key, value) in metadata {
        for text in [key, value] {
            if text.len() > 1024 * 1024 {
                return Err("Metadata string too long".into());
            }
            *budget = budget
                .checked_sub(text.len() + 4)
                .ok_or("Metadata exceeds limit")?;
            writer
                .write_all(&(text.len() as u32).to_le_bytes())
                .map_err(|e| e.to_string())?;
            writer
                .write_all(text.as_bytes())
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
fn read_metadata(
    reader: &mut impl Read,
    budget: &mut usize,
) -> Result<std::collections::BTreeMap<String, String>, String> {
    let count = u32::from_le_bytes(read_array(reader)?) as usize;
    if count > 4096 {
        return Err("Too many metadata entries".into());
    }
    let mut result = std::collections::BTreeMap::new();
    for _ in 0..count {
        let mut pair = Vec::new();
        for _ in 0..2 {
            let len = u32::from_le_bytes(read_array(reader)?) as usize;
            if len > 1024 * 1024 {
                return Err("Metadata string too long".into());
            }
            *budget = budget
                .checked_sub(len + 4)
                .ok_or("Metadata exceeds limit")?;
            let mut bytes = vec![0; len];
            reader.read_exact(&mut bytes).map_err(|e| e.to_string())?;
            pair.push(String::from_utf8(bytes).map_err(|e| e.to_string())?);
        }
        let value = pair.pop().unwrap();
        let key = pair.pop().unwrap();
        if result.insert(key, value).is_some() {
            return Err("Duplicate metadata key".into());
        }
    }
    Ok(result)
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;
    #[test]
    fn cancelled_open_does_not_access_missing_input() {
        let cancel = AtomicBool::new(true);
        let path = Path::new("missing-cancelled-input.prust");
        assert_eq!(open_cancellable(path, &cancel).err().unwrap(), CANCELLED);
        assert_eq!(
            open_project_cancellable(path, &cancel).err().unwrap(),
            CANCELLED
        );
    }

    #[test]
    fn cancellation_reader_stops_between_chunks_and_rejects_seeking() {
        struct Trigger<'a> {
            cancel: &'a AtomicBool,
            bytes: Cursor<Vec<u8>>,
        }
        impl Read for Trigger<'_> {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                let read = self.bytes.read(buffer)?;
                self.cancel.store(true, Ordering::Relaxed);
                Ok(read)
            }
        }
        let cancel = AtomicBool::new(false);
        let mut reader = CancellableReader {
            inner: Trigger {
                cancel: &cancel,
                bytes: Cursor::new(vec![42; 128 * 1024]),
            },
            cancel: &cancel,
        };
        let mut output = vec![0; 128 * 1024];
        assert_eq!(reader.read(&mut output).unwrap(), 64 * 1024);
        assert_eq!(reader.read(&mut output).unwrap_err().to_string(), CANCELLED);
        let mut reader = CancellableReader {
            inner: Cursor::new(output),
            cancel: &cancel,
        };
        assert_eq!(
            reader.seek(SeekFrom::Start(0)).unwrap_err().to_string(),
            CANCELLED
        );
    }

    #[test]
    fn cancellable_open_preserves_project_and_raster_pixels() {
        let dir = tempfile::tempdir().unwrap();
        let doc = Document::from_pixels(2, 1, vec![[1, 2, 3, 4], [250, 0, 128, 255]]).unwrap();
        let cancel = AtomicBool::new(false);
        let path = dir.path().join("image.prust");
        save_project_cancellable(&doc, &path, &cancel).unwrap();
        assert_eq!(
            open_cancellable(&path, &cancel).unwrap().pixels(),
            doc.pixels()
        );
        let path = dir.path().join("image.png");
        image::save_buffer(
            &path,
            &[1, 2, 3, 4, 250, 0, 128, 255],
            2,
            1,
            image::ColorType::Rgba8,
        )
        .unwrap();
        assert_eq!(
            open_cancellable(&path, &cancel).unwrap().pixels(),
            doc.pixels()
        );
    }

    #[test]
    fn cancellation_before_atomic_replace_keeps_old_file_and_removes_temporary() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("saved.prust");
        std::fs::write(&path, b"previous version").unwrap();
        let cancel = AtomicBool::new(false);
        let result = write_atomic(&path, &cancel, |file| {
            file.write_all(b"replacement version").unwrap();
            cancel.store(true, Ordering::Relaxed);
            Ok(())
        });
        assert_eq!(result.unwrap_err(), CANCELLED);
        assert_eq!(std::fs::read(&path).unwrap(), b"previous version");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn cancellation_writer_ends_write_all_without_retrying_interrupted_writes() {
        let cancel = AtomicBool::new(true);
        let mut buffer = Vec::new();
        let mut writer = CancellableWriter {
            inner: &mut buffer,
            cancel: &cancel,
        };
        assert!(writer.write_all(b"data").is_err());
        assert!(buffer.is_empty());
    }
}
