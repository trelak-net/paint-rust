//! Session-only undo storage. Metadata stays in memory; large pixel buffers spill.
use super::{Pixel, Snapshot};
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use std::{
    collections::hash_map::DefaultHasher,
    fs::File,
    hash::{Hash, Hasher},
    io::{self, Read, Seek, SeekFrom, Write},
    sync::{Arc, Mutex},
};

/// Use the application cache instead of /tmp, which commonly lives on tmpfs.
pub(super) fn temporary_file() -> io::Result<File> {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from);
    #[cfg(not(target_os = "windows"))]
    let base = std::env::var_os("XDG_CACHE_HOME")
        .filter(|value| std::path::Path::new(value).is_absolute())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".cache"))
        });
    let base = base.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "No application cache directory is available",
        )
    })?;
    let directory = base.join("net.trelak.PaintRust").join("history");
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&directory)?;
    tempfile::tempfile_in(directory)
}

#[derive(Clone)]
pub(super) enum Entry {
    Memory(Snapshot),
    Disk(Arc<Stored>),
}

pub(super) struct Stored {
    metadata: Snapshot,
    lengths: Vec<usize>,
    mask_length: Option<usize>,
    file: Mutex<File>,
    bytes: u64,
    checksum: u64,
}

impl Entry {
    pub(super) fn label(&self) -> &str {
        match self {
            Self::Memory(snapshot) => &snapshot.label,
            Self::Disk(stored) => &stored.metadata.label,
        }
    }

    pub(super) fn memory(&self) -> Option<&Snapshot> {
        match self {
            Self::Memory(snapshot) => Some(snapshot),
            Self::Disk(_) => None,
        }
    }

    pub(super) fn disk_bytes(&self) -> u64 {
        match self {
            Self::Memory(_) => 0,
            Self::Disk(stored) => stored.bytes,
        }
    }

    pub(super) fn spill(&mut self, create: impl FnOnce() -> io::Result<File>) -> io::Result<()> {
        let Self::Memory(snapshot) = self else {
            return Ok(());
        };
        // Keep the original entry until every write and flush has succeeded.
        let mut encoder = ZlibEncoder::new(create()?, Compression::fast());
        let mut checksum = DefaultHasher::new();
        let mut lengths = Vec::new();
        for layer in &snapshot.layers {
            let data = layer.pixels.as_flattened();
            encoder.write_all(data)?;
            data.hash(&mut checksum);
            lengths.push(layer.pixels.len());
        }
        let mask = snapshot.selection.as_ref().and_then(|s| s.mask.as_ref());
        let mask_length = if let Some(mask) = mask {
            encoder.write_all(&mask.pixels)?;
            mask.pixels.as_slice().hash(&mut checksum);
            Some(mask.pixels.len())
        } else {
            None
        };
        let mut file = encoder.finish()?;
        file.flush()?;
        let bytes = file.metadata()?.len();
        file.sync_data()?;
        let mut metadata = snapshot.clone();
        for layer in &mut metadata.layers {
            layer.pixels = Arc::new(Vec::new());
        }
        if let Some(mask) = metadata.selection.as_mut().and_then(|s| s.mask.as_mut()) {
            mask.pixels = Arc::new(Vec::new());
        }
        *self = Self::Disk(Arc::new(Stored {
            metadata,
            lengths,
            mask_length,
            file: Mutex::new(file),
            bytes,
            checksum: checksum.finish(),
        }));
        Ok(())
    }

    pub(super) fn load(&mut self) -> io::Result<Snapshot> {
        let Self::Disk(stored) = self else {
            return Ok(self.memory().expect("memory entry").clone());
        };
        let mut file = stored
            .file
            .lock()
            .map_err(|_| io::Error::other("History file lock was poisoned"))?;
        if file.metadata()?.len() != stored.bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Temporary history file has an unexpected length",
            ));
        }
        file.seek(SeekFrom::Start(0))?;
        let mut decoder = ZlibDecoder::new(&mut *file);
        let mut snapshot = stored.metadata.clone();
        let mut checksum = DefaultHasher::new();
        for (layer, &length) in snapshot.layers.iter_mut().zip(&stored.lengths) {
            let mut pixels: Vec<Pixel> = vec![[0; 4]; length];
            decoder.read_exact(pixels.as_flattened_mut())?;
            pixels.as_flattened().hash(&mut checksum);
            layer.pixels = Arc::new(pixels);
        }
        if let Some(length) = stored.mask_length {
            let mut pixels = vec![0; length];
            decoder.read_exact(&mut pixels)?;
            pixels.as_slice().hash(&mut checksum);
            snapshot
                .selection
                .as_mut()
                .and_then(|s| s.mask.as_mut())
                .expect("stored selection mask")
                .pixels = Arc::new(pixels);
        }
        // Force trailer validation and reject more decoded bytes than metadata allows.
        if decoder.read(&mut [0u8; 1])? != 0 || decoder.total_in() != stored.bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unexpected trailing history data",
            ));
        }
        if checksum.finish() != stored.checksum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Temporary history checksum mismatch",
            ));
        }
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{BlendMode, Document, Selection};

    #[test]
    fn compressed_history_preserves_varied_pixels_and_reduces_uniform_images() {
        let mut seed = 42u32;
        let pixels: Vec<_> = (0..128 * 128)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                seed.to_le_bytes()
            })
            .collect();
        let mut doc = Document::from_pixels(128, 128, pixels.clone()).unwrap();
        doc.invert();
        doc.spill_history(0, tempfile::tempfile);
        assert!(doc.undo());
        assert_eq!(doc.pixels(), pixels);
        let mut uniform = Document::new(512, 512, [255; 4]).unwrap();
        uniform.invert();
        uniform.spill_history(0, tempfile::tempfile);
        assert!(uniform.history_disk_bytes() < 1024 * 1024 / 10);
        assert!(uniform.undo());
        assert!(uniform.pixels().iter().all(|p| *p == [255; 4]));
    }

    #[test]
    fn worker_clones_can_read_shared_disk_history_without_changing_source() {
        let mut source = Document::new(32, 32, [12, 24, 36, 48]).unwrap();
        source.invert();
        source.spill_history(0, tempfile::tempfile);
        let workers: Vec<_> = (0..2)
            .map(|_| {
                let mut document = source.clone();
                std::thread::spawn(move || {
                    assert!(document.undo());
                    assert!(document.pixels().iter().all(|p| *p == [12, 24, 36, 48]));
                    assert!(document.redo());
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(source.history().count(), 1);
        assert_eq!(source.pixels()[0], [243, 231, 219, 48]);
        assert!(source.undo());
    }

    #[test]
    fn disk_history_restores_layers_masks_revisions_and_redo() {
        let mut doc = Document::new(8, 8, [13, 27, 39, 0]).unwrap();
        doc.add_layer(true);
        doc.set_layer_style("Multiply".into(), 123, BlendMode::Multiply);
        doc.set_visibility(0, false);
        let mask = crate::selection::polygon(8, 8, &[(0.0, 0.0), (7.0, 0.0), (0.0, 7.0)]).unwrap();
        doc.set_selection(Some(mask.clone()));
        doc.mark_saved();
        let before = doc.snapshot();
        doc.invert();
        let after = doc.snapshot();
        let count = doc.history().count();
        doc.spill_history(0, tempfile::tempfile);
        assert_eq!(doc.history_bytes(), 0);
        assert!(doc.history_disk_bytes() > 0);
        assert_eq!(doc.history().count(), count);
        assert!(doc.undo());
        assert!(doc.layers() == before.layers());
        assert_eq!(doc.selection(), Some(mask.clone()));
        assert!(!doc.dirty());
        doc.spill_history(0, tempfile::tempfile);
        assert!(doc.redo());
        assert!(doc.layers() == after.layers());
        assert_eq!(doc.selection(), Some(mask));
        assert!(doc.dirty());
    }

    #[test]
    fn disk_history_restores_dimensions_and_ellipse_geometry() {
        let mut doc = Document::new(8, 8, [11, 22, 33, 44]).unwrap();
        let selection = Selection::elliptical(1, 2, 4, 3);
        doc.set_selection(Some(selection.clone()));
        doc.crop_to_selection();
        doc.spill_history(0, tempfile::tempfile);
        assert!(doc.undo());
        assert_eq!((doc.width, doc.height), (8, 8));
        assert_eq!(doc.selection(), Some(selection));
        doc.spill_history(0, tempfile::tempfile);
        assert!(doc.redo());
        assert_eq!((doc.width, doc.height), (4, 3));
    }

    #[test]
    fn write_failure_keeps_all_history_in_ram_and_reports_the_problem() {
        let mut doc = Document::new(8, 8, [255; 4]).unwrap();
        doc.invert();
        let count = doc.history().count();
        let file = tempfile::NamedTempFile::new().unwrap();
        doc.spill_history(0, || File::open(file.path()));
        assert!(doc.history_error().is_some());
        assert_eq!(doc.history().count(), count);
        assert_eq!(doc.history_bytes(), 256);
        assert_eq!(doc.history_disk_bytes(), 0);
        assert!(doc.undo());
        assert!(doc.pixels().iter().all(|p| *p == [255; 4]));
    }

    #[test]
    fn corrupted_history_is_not_applied_or_removed_and_can_be_retried() {
        let mut doc = Document::new(8, 8, [255; 4]).unwrap();
        doc.invert();
        doc.spill_history(0, tempfile::tempfile);
        let Entry::Disk(stored) = doc.undo.back_mut().unwrap() else {
            panic!("disk entry");
        };
        let original = {
            let mut file = stored.file.lock().unwrap();
            file.seek(SeekFrom::Start(0)).unwrap();
            let mut original = [0];
            file.read_exact(&mut original).unwrap();
            file.seek(SeekFrom::Start(0)).unwrap();
            file.write_all(&[original[0] ^ 255]).unwrap();
            original
        };
        let revision = doc.revision();
        assert!(!doc.undo());
        assert_eq!(doc.revision(), revision);
        assert!(doc.pixels().iter().all(|p| *p == [0, 0, 0, 255]));
        assert_eq!(doc.history().count(), 1);
        assert!(!doc.can_redo());
        assert!(doc.history_error().unwrap().contains("Could not restore"));
        let Entry::Disk(stored) = doc.undo.back_mut().unwrap() else {
            unreachable!();
        };
        stored
            .file
            .lock()
            .unwrap()
            .seek(SeekFrom::Start(0))
            .unwrap();
        stored.file.lock().unwrap().write_all(&original).unwrap();
        assert!(doc.undo());
        assert!(doc.pixels().iter().all(|p| *p == [255; 4]));
    }

    #[test]
    fn truncated_history_preserves_current_edit_and_history() {
        let mut doc = Document::new(8, 8, [255; 4]).unwrap();
        doc.invert();
        doc.spill_history(0, tempfile::tempfile);
        let Entry::Disk(stored) = doc.undo.back_mut().unwrap() else {
            panic!("disk entry");
        };
        stored.file.lock().unwrap().set_len(1).unwrap();
        doc.begin("Pending edit");
        assert!(!doc.undo());
        assert!(doc.editing());
        assert_eq!(doc.history().count(), 1);
        assert!(doc.history_error().unwrap().contains("length"));
    }

    #[test]
    fn new_edit_releases_disk_redo_entries() {
        let mut doc = Document::new(8, 8, [255; 4]).unwrap();
        doc.invert();
        doc.undo();
        doc.spill_history(0, tempfile::tempfile);
        assert!(doc.history_disk_bytes() > 0);
        doc.invert();
        assert!(!doc.can_redo());
        assert_eq!(doc.history_disk_bytes(), 0);
    }
}
