//! One directory worker per window. Superseded queued requests are skipped.
use super::load_directory_entries;
use crate::types::FileEntry;
use std::{collections::HashSet, path::PathBuf, sync::mpsc};

pub struct DirectoryLoader {
    requests: mpsc::Sender<(u64, PathBuf, bool)>,
    results: mpsc::Receiver<(u64, anyhow::Result<Vec<FileEntry>>)>,
    generation: u64,
}

impl Default for DirectoryLoader {
    fn default() -> Self {
        let (requests, incoming) = mpsc::channel::<(u64, PathBuf, bool)>();
        let (outgoing, results) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(mut request) = incoming.recv() {
                while let Ok(newer) = incoming.try_recv() {
                    request = newer;
                }
                let (generation, path, hidden) = request;
                let started = std::time::Instant::now();
                let result = load_directory_entries(&path, hidden, &HashSet::new());
                log::debug!(
                    "Directory scan generation {generation} finished in {:?}; success={}",
                    started.elapsed(),
                    result.is_ok()
                );
                if outgoing.send((generation, result)).is_err() {
                    break;
                }
            }
        });
        Self {
            requests,
            results,
            generation: 0,
        }
    }
}

impl DirectoryLoader {
    pub fn request(&mut self, path: PathBuf, show_hidden: bool) {
        self.generation = self.generation.wrapping_add(1);
        log::debug!(
            "Directory scan requested: generation={}, hidden={show_hidden}",
            self.generation
        );
        let _ = self.requests.send((self.generation, path, show_hidden));
    }

    /// A slow response from a previous folder can never replace the latest folder.
    pub fn poll(&self) -> Option<anyhow::Result<Vec<FileEntry>>> {
        let mut latest = None;
        while let Ok((generation, result)) = self.results.try_recv() {
            if generation == self.generation {
                latest = Some(result);
            }
        }
        latest
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_results_never_replace_current_folder() {
        let (requests, _incoming) = mpsc::channel();
        let (outgoing, results) = mpsc::channel();
        let mut loader = DirectoryLoader {
            requests,
            results,
            generation: 0,
        };
        loader.request("old".into(), false);
        loader.request("current".into(), false);
        outgoing
            .send((1, Err(anyhow::anyhow!("stale failure"))))
            .unwrap();
        assert!(loader.poll().is_none());
        outgoing.send((2, Ok(Vec::new()))).unwrap();
        assert!(loader.poll().unwrap().is_ok());
    }

    #[test]
    fn background_worker_loads_real_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("notes.txt"), "test").unwrap();
        let mut loader = DirectoryLoader::default();
        loader.request(dir.path().into(), false);
        let (generation, result) = loader
            .results
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert_eq!(generation, loader.generation);
        let entries = result.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "notes.txt");
    }
}
