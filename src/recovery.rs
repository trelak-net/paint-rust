use crate::{document::Document, files};
use fs2::FileExt;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc,
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub struct Candidate {
    pub path: PathBuf,
    pub modified: SystemTime,
    _lock: Arc<File>,
}
impl Candidate {
    pub fn discard(self) -> Result<(), String> {
        fs::remove_dir_all(self.path.parent().ok_or("Invalid recovery path")?)
            .map_err(|e| e.to_string())
    }
}

struct Completion {
    epoch: u64,
    revision: u64,
    result: Result<(), String>,
}

pub struct Recovery {
    session: PathBuf,
    lock: Arc<File>,
    epoch: u64,
    pending: Option<Receiver<Completion>>,
    written_revision: Option<u64>,
    has_work: bool,
    last_attempt: Instant,
    adopted: Vec<Candidate>,
}

impl Recovery {
    pub fn default_root() -> Result<PathBuf, String> {
        if let Some(path) = std::env::var_os("XDG_STATE_HOME").filter(|p| !p.is_empty()) {
            return Ok(PathBuf::from(path).join("paint-rust/recovery"));
        }
        #[cfg(target_os = "windows")]
        if let Some(path) = std::env::var_os("LOCALAPPDATA") {
            return Ok(PathBuf::from(path).join("PaintRust/recovery"));
        }
        std::env::var_os("HOME")
            .map(|path| PathBuf::from(path).join(".local/state/paint-rust/recovery"))
            .ok_or_else(|| "No writable user state directory was found".into())
    }

    pub fn new(root: &Path) -> Result<Self, String> {
        fs::create_dir_all(root).map_err(|e| e.to_string())?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let session = root.join(format!("session-{}-{stamp}", std::process::id()));
        fs::create_dir(&session).map_err(|e| e.to_string())?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(session.join("lock"))
            .map_err(|e| e.to_string())?;
        FileExt::try_lock_exclusive(&lock).map_err(|e| e.to_string())?;
        Ok(Self {
            session,
            lock: Arc::new(lock),
            epoch: 0,
            pending: None,
            written_revision: None,
            has_work: false,
            last_attempt: Instant::now(),
            adopted: Vec::new(),
        })
    }

    pub fn sibling(&self) -> Result<Self, String> {
        Self::new(self.session.parent().ok_or("Invalid recovery root")?)
    }

    pub fn discover(root: &Path) -> Result<Vec<Candidate>, String> {
        let entries = match fs::read_dir(root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.to_string()),
        };
        let mut result = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.file_name().to_string_lossy().starts_with("session-")
                || !entry.file_type().map_err(|e| e.to_string())?.is_dir()
            {
                continue;
            }
            let Ok(lock) = OpenOptions::new()
                .read(true)
                .write(true)
                .open(entry.path().join("lock"))
            else {
                continue;
            };
            if FileExt::try_lock_exclusive(&lock).is_err() {
                continue;
            }
            let lock = Arc::new(lock);
            for group in fs::read_dir(entry.path()).map_err(|e| e.to_string())? {
                let group = group.map_err(|e| e.to_string())?;
                if !group.file_type().map_err(|e| e.to_string())?.is_dir()
                    || !group.file_name().to_string_lossy().starts_with("doc-")
                {
                    continue;
                }
                if group.path().join("discarded").exists() {
                    // The session lock proves no worker can still write here.
                    fs::remove_dir_all(group.path()).map_err(|e| e.to_string())?;
                    continue;
                }
                let path = group.path().join("snapshot.prust");
                if let Ok(metadata) = fs::metadata(&path) {
                    result.push(Candidate {
                        path,
                        modified: metadata.modified().unwrap_or(UNIX_EPOCH),
                        _lock: lock.clone(),
                    });
                }
            }
        }
        result.sort_by_key(|item| std::cmp::Reverse(item.modified));
        Ok(result)
    }

    pub fn adopt(&mut self, candidate: Candidate) {
        self.adopted.push(candidate);
    }

    fn group(&self, epoch: u64) -> PathBuf {
        self.session.join(format!("doc-{epoch}"))
    }

    pub fn clear(&mut self) -> Result<(), String> {
        if !self.has_work && self.adopted.is_empty() {
            return Ok(());
        }
        let group = self.group(self.epoch);
        fs::create_dir_all(&group).map_err(|e| e.to_string())?;
        let mut marker = File::create(group.join("discarded")).map_err(|e| e.to_string())?;
        marker
            .write_all(b"Intentionally saved or discarded\n")
            .map_err(|e| e.to_string())?;
        marker.sync_all().map_err(|e| e.to_string())?;
        self.epoch += 1;
        self.written_revision = None;
        self.has_work = false;
        self.last_attempt = Instant::now();
        // An older worker may still finish; its epoch remains tombstoned.
        if self.pending.is_none() {
            fs::remove_dir_all(&group).map_err(|e| e.to_string())?;
        }
        for candidate in self.adopted.drain(..) {
            candidate.discard()?;
        }
        Ok(())
    }

    pub fn poll(&mut self) -> Result<(), String> {
        let completion = match self.pending.as_ref().map(|receiver| receiver.try_recv()) {
            Some(Ok(completion)) => Some(completion),
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                self.pending = None;
                return Err("Recovery worker stopped unexpectedly".into());
            }
            _ => None,
        };
        if let Some(completion) = completion {
            self.pending = None;
            completion.result?;
            if completion.epoch == self.epoch {
                self.written_revision = Some(completion.revision);
                for candidate in self.adopted.drain(..) {
                    candidate.discard()?;
                }
            } else {
                let _ = fs::remove_dir_all(self.group(completion.epoch));
            }
        }
        Ok(())
    }

    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }

    pub fn schedule(
        &mut self,
        document: &Document,
        wake: impl FnOnce() + Send + 'static,
    ) -> Result<bool, String> {
        self.poll()?;
        if self.pending.is_some()
            || document.editing()
            || !document.dirty()
            || self.written_revision == Some(document.revision())
        {
            return Ok(false);
        }
        let group = self.group(self.epoch);
        fs::create_dir_all(&group).map_err(|e| e.to_string())?;
        let document_snapshot = document.snapshot();
        let revision = document.revision();
        let epoch = self.epoch;
        let lock = self.lock.clone();
        let (sender, receiver) = mpsc::channel();
        self.pending = Some(receiver);
        self.has_work = true;
        self.last_attempt = Instant::now();
        std::thread::spawn(move || {
            let _lock = lock;
            let result = files::save_project(&document_snapshot, &group.join("snapshot.prust"));
            drop(_lock);
            let _ = sender.send(Completion {
                epoch,
                revision,
                result,
            });
            wake();
        });
        Ok(true)
    }

    pub fn tick(
        &mut self,
        document: &Document,
        wake: impl FnOnce() + Send + 'static,
    ) -> Result<(), String> {
        self.poll()?;
        if !document.dirty() && self.has_work {
            self.clear()?;
        }
        if self.last_attempt.elapsed() >= Duration::from_secs(15) {
            self.schedule(document, wake)?;
        }
        Ok(())
    }
}
