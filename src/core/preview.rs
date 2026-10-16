//! Bounded plain-text previews. Decline symlink entries and never execute content.
use anyhow::{ensure, Context, Result};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::mpsc,
};

pub const MAX_PREVIEW_BYTES: usize = 32 * 1024;
#[derive(Debug)]
pub enum Preview {
    Text { text: String, truncated: bool },
    Unsupported,
}

pub fn read_preview(path: &Path) -> Result<Preview> {
    let metadata = fs::symlink_metadata(path).context("Cannot inspect file")?;
    ensure!(
        metadata.is_file(),
        "Preview supports regular files, not links or special files"
    );
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).context("Cannot read file")?;
    ensure!(
        file.metadata()?.is_file(),
        "Preview supports regular files only"
    );
    let mut bytes = Vec::new();
    file.take((MAX_PREVIEW_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    let truncated = bytes.len() > MAX_PREVIEW_BYTES;
    bytes.truncate(MAX_PREVIEW_BYTES);
    if bytes
        .iter()
        .any(|&b| b == 0 || (b < 32 && !matches!(b, b'\t' | b'\r' | b'\n')))
    {
        return Ok(Preview::Unsupported);
    }
    // An otherwise valid UTF-8 file may end with a partial character at the cap.
    let text = match std::str::from_utf8(&bytes) {
        Ok(text) => text,
        Err(error) if truncated && error.error_len().is_none() => {
            std::str::from_utf8(&bytes[..error.valid_up_to()])?
        }
        Err(_) => return Ok(Preview::Unsupported),
    };
    Ok(Preview::Text {
        text: text.into(),
        truncated,
    })
}

pub struct PreviewLoader {
    requests: mpsc::Sender<(u64, PathBuf)>,
    results: mpsc::Receiver<(u64, Result<Preview>)>,
    generation: u64,
}
impl Default for PreviewLoader {
    fn default() -> Self {
        let (requests, incoming) = mpsc::channel::<(u64, PathBuf)>();
        let (outgoing, results) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(mut request) = incoming.recv() {
                while let Ok(newer) = incoming.try_recv() {
                    request = newer;
                }
                if outgoing
                    .send((request.0, read_preview(&request.1)))
                    .is_err()
                {
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
impl PreviewLoader {
    pub fn clear(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }
    pub fn request(&mut self, path: PathBuf) {
        self.clear();
        let _ = self.requests.send((self.generation, path));
    }
    pub fn poll(&self) -> Option<Result<Preview>> {
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
    fn clearing_selection_discards_in_flight_previews() {
        let (requests, _) = mpsc::channel();
        let (outgoing, results) = mpsc::channel();
        let mut loader = PreviewLoader {
            requests,
            results,
            generation: 1,
        };
        loader.clear();
        outgoing.send((1, Ok(Preview::Unsupported))).unwrap();
        assert!(loader.poll().is_none());
        outgoing.send((2, Ok(Preview::Unsupported))).unwrap();
        assert!(loader.poll().unwrap().is_ok());
    }
}
