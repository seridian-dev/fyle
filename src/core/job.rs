//! Serializable intent at the UI boundary; one background mutation per window.
use super::*;
use std::{path::PathBuf, sync::mpsc};

pub enum FileOperation {
    Copy(Vec<PathBuf>, PathBuf),
    Move(Vec<PathBuf>, PathBuf),
    Remove(Vec<PathBuf>),
    CreateFolder(PathBuf, String),
    CreateFile(PathBuf, String),
    Rename(PathBuf, String),
}
impl FileOperation {
    pub fn is_move(&self) -> bool {
        matches!(self, Self::Move(..))
    }
    pub fn execute(self) -> anyhow::Result<()> {
        match self {
            Self::Copy(paths, dest) => copy_files(&paths, &dest),
            Self::Move(paths, dest) => move_files(&paths, &dest),
            Self::Remove(paths) => {
                #[cfg(feature = "trash")]
                {
                    trash_files(&paths)
                }
                #[cfg(not(feature = "trash"))]
                {
                    delete_files(&paths)
                }
            }
            Self::CreateFolder(parent, name) => create_directory(&parent, &name).map(|_| ()),
            Self::CreateFile(parent, name) => create_file(&parent, &name).map(|_| ()),
            Self::Rename(path, name) => rename_file(&path, &name).map(|_| ()),
        }
    }
}

pub struct OperationJob {
    result: mpsc::Receiver<anyhow::Result<()>>,
    pub clears_cut_clipboard: bool,
}
impl OperationJob {
    pub fn start(operation: FileOperation) -> Self {
        let clears_cut_clipboard = operation.is_move();
        let (sender, result) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(operation.execute());
        });
        Self {
            result,
            clears_cut_clipboard,
        }
    }
    pub fn poll(&self) -> Option<anyhow::Result<()>> {
        match self.result.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(anyhow::anyhow!(
                "File operation worker stopped unexpectedly"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn background_job_preserves_errors_and_move_intent() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("folder");
        std::fs::create_dir(&folder).unwrap();
        let job = OperationJob::start(FileOperation::CreateFile(folder, "../escape".into()));
        assert!(!job.clears_cut_clipboard);
        assert!(job
            .result
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
            .is_err());
        assert!(FileOperation::Move(Vec::new(), dir.path().into()).is_move());
        assert!(!dir.path().join("escape").exists());
    }
    #[test]
    fn background_job_copies_file() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("dest");
        std::fs::create_dir(&dest).unwrap();
        let source = dir.path().join("notes");
        std::fs::write(&source, "valuable").unwrap();
        let job = OperationJob::start(FileOperation::Copy(vec![source], dest.clone()));
        job.result
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(dest.join("notes")).unwrap(),
            "valuable"
        );
    }
}
