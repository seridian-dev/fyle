//! Bounded UTF-8 document loading and conflict-aware atomic saves.
use anyhow::{ensure, Context, Result};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    sync::mpsc,
};

pub const MAX_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub struct Document {
    path: PathBuf,
    original: Vec<u8>,
    text: String,
}

pub enum EditorTask {
    Loaded {
        path: PathBuf,
        result: Result<Document>,
    },
    Saved {
        path: PathBuf,
        result: Result<Document>,
    },
}

enum EditorRequest {
    Load(PathBuf),
    Save(Document),
}

/// One background worker for bounded editor reads and saves.
pub struct EditorWorker {
    requests: mpsc::Sender<EditorRequest>,
    results: mpsc::Receiver<EditorTask>,
}

impl Default for EditorWorker {
    fn default() -> Self {
        let (requests, incoming) = mpsc::channel::<EditorRequest>();
        let (outgoing, results) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(request) = incoming.recv() {
                let task = match request {
                    EditorRequest::Load(path) => EditorTask::Loaded {
                        result: Document::load(&path),
                        path,
                    },
                    EditorRequest::Save(mut document) => {
                        let path = document.path.clone();
                        let result = document.save().map(|()| document);
                        EditorTask::Saved { path, result }
                    }
                };
                if outgoing.send(task).is_err() {
                    break;
                }
            }
        });
        Self { requests, results }
    }
}

impl EditorWorker {
    pub fn load(&self, path: PathBuf) -> Result<()> {
        self.requests
            .send(EditorRequest::Load(path))
            .context("Editor worker stopped")
    }

    pub fn save(&self, document: Document) -> Result<()> {
        self.requests
            .send(EditorRequest::Save(document))
            .context("Editor worker stopped")
    }

    pub fn poll(&self) -> Vec<EditorTask> {
        self.results.try_iter().collect()
    }
}

impl Document {
    pub fn load(path: &Path) -> Result<Self> {
        let (bytes, _) = read_regular_file(path)?;
        let text = String::from_utf8(bytes.clone()).context("File is not valid UTF-8 text")?;
        ensure!(
            !text
                .chars()
                .any(|ch| ch == '\0' || (ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))),
            "File contains binary control bytes"
        );
        Ok(Self {
            path: path.to_path_buf(),
            original: bytes,
            text,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn text_mut(&mut self) -> &mut String {
        &mut self.text
    }

    pub fn set_text(&mut self, text: String) {
        self.text = text;
    }

    pub fn is_dirty(&self) -> bool {
        self.text.as_bytes() != self.original
    }

    /// Save edits only if the file still has the bytes this document opened.
    /// Writes a synced temporary sibling and atomically renames it into place.
    pub fn save(&mut self) -> Result<()> {
        ensure!(
            self.text.len() <= MAX_DOCUMENT_BYTES,
            "Document exceeds the 2 MiB editor limit"
        );
        let (_, metadata) = read_regular_file(&self.path)?;
        let parent = self.path.parent().context("File has no parent directory")?;
        let name = self
            .path
            .file_name()
            .context("File has no name")?
            .to_string_lossy();
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(
            ".{name}.fyle-save-{}-{sequence}",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            use std::os::unix::fs::PermissionsExt;
            options.mode(metadata.permissions().mode() & 0o777);
        }
        let mut file = options
            .open(&temporary)
            .context("Cannot create a temporary save file")?;
        let result = (|| -> Result<()> {
            file.write_all(self.text.as_bytes())?;
            file.sync_all()?;
            drop(file);
            let (current, _) = read_regular_file(&self.path)?;
            ensure!(
                current == self.original,
                "File changed on disk; reload before saving"
            );
            fs::rename(&temporary, &self.path).context("Cannot replace the original file")?;
            self.original = self.text.as_bytes().to_vec();
            #[cfg(unix)]
            File::open(parent)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

pub fn is_text_candidate(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if [
        "makefile",
        "dockerfile",
        "justfile",
        "license",
        "readme",
        ".gitignore",
        ".env",
    ]
    .contains(&name.as_str())
    {
        return true;
    }
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "txt"
            | "md"
            | "rst"
            | "org"
            | "log"
            | "rs"
            | "py"
            | "js"
            | "jsx"
            | "ts"
            | "tsx"
            | "go"
            | "java"
            | "c"
            | "h"
            | "cc"
            | "cpp"
            | "hpp"
            | "cs"
            | "sh"
            | "bash"
            | "zsh"
            | "fish"
            | "html"
            | "css"
            | "scss"
            | "json"
            | "jsonc"
            | "toml"
            | "yaml"
            | "yml"
            | "xml"
            | "ini"
            | "cfg"
            | "conf"
            | "sql"
            | "vue"
            | "svelte"
            | "php"
            | "rb"
            | "lua"
            | "swift"
            | "kt"
            | "gradle"
            | "nix"
            | "desktop"
            | "service"
            | "diff"
            | "patch"
    )
}

fn read_regular_file(path: &Path) -> Result<(Vec<u8>, fs::Metadata)> {
    let entry = fs::symlink_metadata(path).context("Cannot inspect file")?;
    ensure!(
        entry.is_file(),
        "Editor supports regular files only; links and special files are rejected"
    );
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut file = options
        .open(path)
        .context("Cannot open file without following links")?;
    let metadata = file.metadata()?;
    ensure!(metadata.is_file(), "Editor supports regular files only");
    ensure!(
        metadata.len() <= MAX_DOCUMENT_BYTES as u64,
        "Document exceeds the 2 MiB editor limit"
    );
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    Read::by_ref(&mut file)
        .take((MAX_DOCUMENT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= MAX_DOCUMENT_BYTES,
        "Document exceeds the 2 MiB editor limit"
    );
    Ok((bytes, metadata))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn reads_regular_utf8_and_marks_binary_content() {
        let dir = tempdir().unwrap();
        let text = dir.path().join("text.txt");
        fs::write(&text, "hello\n").unwrap();
        assert_eq!(Document::load(&text).unwrap().text(), "hello\n");
        let binary = dir.path().join("binary");
        fs::write(&binary, [0_u8, 1, 2]).unwrap();
        assert!(Document::load(&binary).is_err());
    }
}
