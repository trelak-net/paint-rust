use super::*;

#[derive(Default)]
pub(super) struct Batch {
    pub pending: std::collections::VecDeque<PathBuf>,
    pub errors: Vec<String>,
    pub restored: usize,
    pub cancelled: bool,
    pub preferred: Option<u64>,
}

impl Editor {
    pub(super) fn recover_candidate(&mut self, index: usize) {
        let path = self.recoverable[index].path.clone();
        self.start_cancellable_file_job("recover project", move |cancel| {
            Ok(Outcome::Recovered(
                Box::new(files::open_cancellable(&path, cancel)?),
                index,
            ))
        });
    }
    pub(super) fn start_recovery_batch(&mut self) {
        if self.file_job.is_some() || self.recovery_batch.is_some() {
            return;
        }
        self.recovery_batch = Some(Batch {
            pending: self
                .recoverable
                .iter()
                .map(|candidate| candidate.path.clone())
                .collect(),
            ..Default::default()
        });
        self.advance_recovery_batch();
    }
    pub(super) fn advance_recovery_batch(&mut self) {
        let Some(batch) = &mut self.recovery_batch else {
            return;
        };
        while let Some(path) = batch.pending.pop_front() {
            if let Some(index) = self
                .recoverable
                .iter()
                .position(|candidate| candidate.path == path)
            {
                self.recover_candidate(index);
                return;
            }
        }
        let batch = self.recovery_batch.take().unwrap();
        if let Some(id) = batch.preferred {
            self.activate_document(id);
        }
        self.recovery_prompt = !self.recoverable.is_empty();
        self.status = format!(
            "Recovered {} document(s); {} failed{}",
            batch.restored,
            batch.errors.len(),
            if batch.cancelled {
                "; remaining recovery cancelled"
            } else {
                ""
            }
        );
        if let Some(error) = batch.errors.first() {
            self.status.push_str(&format!(". First error: {error}"));
        }
    }
}
