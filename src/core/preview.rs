//! Bounded plain-text previews. Decline symlink entries and never execute content.
use anyhow::{ensure, Context, Result};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::mpsc,
};

pub const MAX_PREVIEW_BYTES: usize = 32 * 1024;
#[cfg(feature = "image-preview")]
const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;
#[cfg(feature = "image-preview")]
const MAX_IMAGE_DIMENSION: u32 = 4096;
#[derive(Debug)]
pub enum Preview {
    Text {
        text: String,
        truncated: bool,
    },
    Image {
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    },
    Audio,
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
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "aac"
                    | "aif"
                    | "aifc"
                    | "aiff"
                    | "caf"
                    | "flac"
                    | "m4a"
                    | "mp3"
                    | "oga"
                    | "ogg"
                    | "opus"
                    | "wav"
                    | "wave"
            )
        })
    {
        return Ok(Preview::Audio);
    }
    let mut bytes = Vec::new();
    (&file).take(32).read_to_end(&mut bytes)?;
    #[cfg(feature = "image-preview")]
    if let Ok(format) = image::guess_format(&bytes) {
        if matches!(
            format,
            image::ImageFormat::Bmp
                | image::ImageFormat::Gif
                | image::ImageFormat::Ico
                | image::ImageFormat::Jpeg
                | image::ImageFormat::Png
                | image::ImageFormat::Pnm
                | image::ImageFormat::Qoi
                | image::ImageFormat::Tga
                | image::ImageFormat::Tiff
                | image::ImageFormat::WebP
        ) {
            if bytes.len() < MAX_IMAGE_BYTES {
                (&file)
                    .take((MAX_IMAGE_BYTES + 1 - bytes.len()) as u64)
                    .read_to_end(&mut bytes)?;
            }
            if bytes.len() > MAX_IMAGE_BYTES {
                return Ok(Preview::Unsupported);
            }
            let mut reader = image::io::Reader::with_format(std::io::Cursor::new(bytes), format);
            let mut limits = image::io::Limits::default();
            limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
            limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
            limits.max_alloc = Some(128 * 1024 * 1024);
            reader.limits(limits);
            let decoded = match reader.decode() {
                Ok(image) => image.to_rgba8(),
                Err(_) => return Ok(Preview::Unsupported),
            };
            return Ok(Preview::Image {
                width: decoded.width(),
                height: decoded.height(),
                rgba: decoded.into_raw(),
            });
        }
    }
    if bytes.len() < MAX_PREVIEW_BYTES + 1 {
        (&file)
            .take((MAX_PREVIEW_BYTES + 1 - bytes.len()) as u64)
            .read_to_end(&mut bytes)?;
    }
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

    #[cfg(feature = "image-preview")]
    #[test]
    fn image_preview_decodes_a_bounded_rgba_thumbnail_payload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.png");
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer
                .write_image_data(&[255, 0, 0, 255, 0, 255, 0, 255])
                .unwrap();
        }
        fs::write(&path, bytes).unwrap();

        match read_preview(&path).unwrap() {
            Preview::Image {
                width,
                height,
                rgba,
            } => {
                assert_eq!((width, height), (2, 1));
                assert_eq!(rgba, [255, 0, 0, 255, 0, 255, 0, 255]);
            }
            other => panic!("expected image preview, got {other:?}"),
        }
    }

    #[test]
    fn audio_files_are_routed_to_the_player_without_text_decoding() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recording.wav");
        fs::write(&path, b"audio payload is decoded by the playback worker").unwrap();
        assert!(matches!(read_preview(&path).unwrap(), Preview::Audio));
    }
}
