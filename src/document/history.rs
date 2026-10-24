//! Session-only undo storage. Metadata stays in memory; large pixel buffers spill.
use super::{Pixel, Snapshot};
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use std::{
    collections::{HashMap, HashSet, hash_map::DefaultHasher},
    fs::File,
    hash::{Hash, Hasher},
    io::{self, Read, Seek, SeekFrom, Write},
    sync::{Arc, Mutex, Weak},
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

// Weak source references keep allocation identities distinct without retaining
// pixel bytes. Weak blob references do not prolong the life of history files.
type Cache<T> = HashMap<usize, (Weak<Vec<T>>, Weak<Blob>)>;

#[derive(Clone, Default)]
pub(super) struct Pool {
    pixels: Cache<Pixel>,
    masks: Cache<u8>,
}

impl Pool {
    fn prune(&mut self) {
        self.pixels
            .retain(|_, (source, blob)| source.strong_count() > 0 && blob.strong_count() > 0);
        self.masks
            .retain(|_, (source, blob)| source.strong_count() > 0 && blob.strong_count() > 0);
    }
}

fn cached<T>(cache: &Cache<T>, source: &Arc<Vec<T>>) -> Option<Arc<Blob>> {
    cache
        .get(&(Arc::as_ptr(source) as usize))
        .and_then(|(_, blob)| blob.upgrade())
}

fn register<T>(cache: &mut Cache<T>, source: &Arc<Vec<T>>, blob: &Arc<Blob>) {
    cache.insert(
        Arc::as_ptr(source) as usize,
        (Arc::downgrade(source), Arc::downgrade(blob)),
    );
}

#[derive(Clone)]
pub(super) enum Entry {
    Memory(Snapshot),
    Disk(Arc<Stored>),
}

pub(super) struct Stored {
    metadata: Snapshot,
    layers: Vec<Arc<Blob>>,
    mask: Option<Arc<Blob>>,
}

struct Blob {
    file: Mutex<File>,
    bytes: u64,
    decoded_bytes: usize,
    checksum: u64,
    decoded_pixels: Mutex<Weak<Vec<Pixel>>>,
    decoded_mask: Mutex<Weak<Vec<u8>>>,
}

impl Blob {
    fn write(data: &[u8], file: File) -> io::Result<Arc<Self>> {
        let mut encoder = ZlibEncoder::new(file, Compression::fast());
        encoder.write_all(data)?;
        let mut file = encoder.finish()?;
        file.flush()?;
        file.sync_data()?;
        let mut checksum = DefaultHasher::new();
        data.hash(&mut checksum);
        Ok(Arc::new(Self {
            bytes: file.metadata()?.len(),
            file: Mutex::new(file),
            decoded_bytes: data.len(),
            checksum: checksum.finish(),
            decoded_pixels: Mutex::new(Weak::new()),
            decoded_mask: Mutex::new(Weak::new()),
        }))
    }

    fn read(&self, output: &mut [u8]) -> io::Result<()> {
        if output.len() != self.decoded_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "History buffer dimensions do not match",
            ));
        }
        let mut file = self
            .file
            .lock()
            .map_err(|_| io::Error::other("History file lock was poisoned"))?;
        if file.metadata()?.len() != self.bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Temporary history file has an unexpected length",
            ));
        }
        file.seek(SeekFrom::Start(0))?;
        let mut decoder = ZlibDecoder::new(&mut *file);
        decoder.read_exact(output)?;
        if decoder.read(&mut [0u8; 1])? != 0 || decoder.total_in() != self.bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unexpected trailing history data",
            ));
        }
        let mut checksum = DefaultHasher::new();
        output.hash(&mut checksum);
        if checksum.finish() != self.checksum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Temporary history checksum mismatch",
            ));
        }
        Ok(())
    }
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

    pub(super) fn disk_bytes(&self, seen: &mut HashSet<usize>) -> u64 {
        match self {
            Self::Memory(_) => 0,
            Self::Disk(stored) => stored
                .layers
                .iter()
                .chain(stored.mask.iter())
                .filter(|blob| seen.insert(Arc::as_ptr(blob) as usize))
                .map(|blob| blob.bytes)
                .sum(),
        }
    }

    pub(super) fn spill(
        &mut self,
        pool: &mut Pool,
        mut create: impl FnMut() -> io::Result<File>,
    ) -> io::Result<()> {
        let Self::Memory(snapshot) = self else {
            return Ok(());
        };
        pool.prune();
        let mut layers = Vec::with_capacity(snapshot.layers.len());
        for layer in &snapshot.layers {
            let blob = if let Some(blob) = cached(&pool.pixels, &layer.pixels) {
                blob
            } else {
                let blob = Blob::write(layer.pixels.as_flattened(), create()?)?;
                *blob.decoded_pixels.lock().expect("new history buffer") =
                    Arc::downgrade(&layer.pixels);
                register(&mut pool.pixels, &layer.pixels, &blob);
                blob
            };
            layers.push(blob);
        }
        let mask = if let Some(mask) = snapshot.selection.as_ref().and_then(|s| s.mask.as_ref()) {
            Some(if let Some(blob) = cached(&pool.masks, &mask.pixels) {
                blob
            } else {
                let blob = Blob::write(&mask.pixels, create()?)?;
                *blob.decoded_mask.lock().expect("new history mask") = Arc::downgrade(&mask.pixels);
                register(&mut pool.masks, &mask.pixels, &blob);
                blob
            })
        } else {
            None
        };
        // No original buffers are released until every required file is durable.
        let mut metadata = snapshot.clone();
        for layer in &mut metadata.layers {
            layer.pixels = Arc::new(Vec::new());
        }
        if let Some(mask) = metadata.selection.as_mut().and_then(|s| s.mask.as_mut()) {
            mask.pixels = Arc::new(Vec::new());
        }
        *self = Self::Disk(Arc::new(Stored {
            metadata,
            layers,
            mask,
        }));
        Ok(())
    }

    pub(super) fn load(&mut self, pool: &mut Pool) -> io::Result<Snapshot> {
        let Self::Disk(stored) = self else {
            return Ok(self.memory().expect("memory entry").clone());
        };
        pool.prune();
        let mut snapshot = stored.metadata.clone();
        for (layer, blob) in snapshot.layers.iter_mut().zip(&stored.layers) {
            let mut cached = blob
                .decoded_pixels
                .lock()
                .map_err(|_| io::Error::other("Decoded history lock was poisoned"))?;
            let pixels = if let Some(pixels) = cached.upgrade() {
                pixels
            } else {
                let mut pixels = vec![[0; 4]; snapshot.width as usize * snapshot.height as usize];
                blob.read(pixels.as_flattened_mut())?;
                let pixels = Arc::new(pixels);
                *cached = Arc::downgrade(&pixels);
                pixels
            };
            register(&mut pool.pixels, &pixels, blob);
            layer.pixels = pixels;
        }
        if let Some(blob) = &stored.mask {
            let mut cached = blob
                .decoded_mask
                .lock()
                .map_err(|_| io::Error::other("Decoded mask lock was poisoned"))?;
            let pixels = if let Some(pixels) = cached.upgrade() {
                pixels
            } else {
                let mut pixels = vec![0; blob.decoded_bytes];
                blob.read(&mut pixels)?;
                let pixels = Arc::new(pixels);
                *cached = Arc::downgrade(&pixels);
                pixels
            };
            register(&mut pool.masks, &pixels, blob);
            snapshot
                .selection
                .as_mut()
                .and_then(|s| s.mask.as_mut())
                .expect("stored selection mask")
                .pixels = pixels;
        }
        Ok(snapshot)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{BlendMode, Document, Selection};

    #[test]
    fn unchanged_layers_and_masks_share_files_across_history_states() {
        let mut doc = Document::new(16, 16, [10, 20, 30, 255]).unwrap();
        doc.add_layer(false);
        let selection =
            crate::selection::polygon(16, 16, &[(0.0, 0.0), (15.0, 0.0), (0.0, 15.0)]).unwrap();
        doc.set_selection(Some(selection));
        doc.invert();
        doc.set_layer_style("Detail".into(), 120, BlendMode::Multiply);
        doc.set_visibility(0, false);
        let before = doc.snapshot();
        let mut files = 0;
        doc.spill_history(0, || {
            files += 1;
            tempfile::tempfile()
        });
        assert_eq!(files, 4, "three pixel buffers and one mask");
        let mut seen = HashSet::new();
        let expected: u64 = doc
            .undo
            .iter()
            .map(|entry| entry.disk_bytes(&mut seen))
            .sum();
        assert_eq!(seen.len(), 4);
        assert_eq!(doc.history_disk_bytes(), expected);
        for _ in 0..4 {
            assert!(doc.undo());
        }
        assert_eq!(doc.pixels()[0], [10, 20, 30, 255]);
        // Undoing every entry releases all files; restored snapshots still share
        // buffers, so re-spilling creates only the same four distinct blobs.
        assert_eq!(doc.history_disk_bytes(), 0);
        doc.spill_history(0, || {
            files += 1;
            tempfile::tempfile()
        });
        assert_eq!(files, 8);
        for _ in 0..4 {
            assert!(doc.redo());
        }
        assert!(doc.layers() == before.layers());
    }

    #[test]
    fn failure_after_one_buffer_keeps_snapshot_and_releases_partial_files() {
        let mut doc = Document::new(8, 8, [255; 4]).unwrap();
        doc.add_layer(false);
        doc.invert();
        let mut entry = doc.undo.pop_back().unwrap();
        let mut calls = 0;
        assert!(
            entry
                .spill(&mut doc.history_pool, || {
                    calls += 1;
                    if calls == 2 {
                        Err(io::Error::other("simulated full disk"))
                    } else {
                        tempfile::tempfile()
                    }
                })
                .is_err()
        );
        assert!(entry.memory().is_some());
        assert!(
            doc.history_pool
                .pixels
                .values()
                .all(|(_, blob)| blob.upgrade().is_none())
        );
        assert!(
            entry
                .spill(&mut doc.history_pool, tempfile::tempfile)
                .is_ok()
        );
    }

    #[test]
    fn obsolete_redo_files_are_released_even_when_pool_has_cached_references() {
        let mut doc = Document::new(8, 8, [255; 4]).unwrap();
        doc.invert();
        doc.undo();
        doc.spill_history(0, tempfile::tempfile);
        let blob = match &doc.redo[0] {
            Entry::Disk(stored) => Arc::downgrade(&stored.layers[0]),
            _ => panic!("disk entry"),
        };
        assert!(blob.upgrade().is_some());
        doc.invert();
        assert!(blob.upgrade().is_none());
        assert_eq!(doc.history_disk_bytes(), 0);
    }

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
            let mut file = stored.layers[0].file.lock().unwrap();
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
        stored.layers[0]
            .file
            .lock()
            .unwrap()
            .seek(SeekFrom::Start(0))
            .unwrap();
        stored.layers[0]
            .file
            .lock()
            .unwrap()
            .write_all(&original)
            .unwrap();
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
        stored.layers[0].file.lock().unwrap().set_len(1).unwrap();
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
