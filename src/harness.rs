//! Opt-in, app-scoped automation. No desktop-wide input or screen capture.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};

const MAX_HEADER: u64 = 2 * 1024 * 1024;
const MAX_PNG: usize = 64 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub pid: u32,
    pub address: String,
    pub token: String,
    pub version: String,
    pub revision: String,
}
pub struct Request {
    pub command: Value,
    pub respond: mpsc::SyncSender<Reply>,
}
pub struct Reply {
    pub data: Value,
    pub png: Vec<u8>,
}
impl Reply {
    pub fn ok(data: Value) -> Self {
        Self {
            data,
            png: Vec::new(),
        }
    }
    pub fn error(message: impl ToString) -> Self {
        Self::ok(json!({"error":message.to_string()}))
    }
}
pub struct Server {
    pub session: Session,
    incoming: mpsc::Receiver<Request>,
    stopped: Arc<AtomicBool>,
    descriptor: PathBuf,
}
pub fn directory() -> PathBuf {
    std::env::var_os("FYLE_HARNESS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::data_local_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join("fyle/harness")
        })
}
pub fn random_id() -> Result<String> {
    let mut bytes = [0; 24];
    getrandom::getrandom(&mut bytes)
        .map_err(|error| anyhow::anyhow!("Cannot create session token: {error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
pub fn session_path(root: &Path, id: &str) -> Result<PathBuf> {
    anyhow::ensure!(
        !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_hexdigit()),
        "Invalid session identifier"
    );
    Ok(root.join(format!("session-{id}.json")))
}
impl Server {
    pub fn start(root: PathBuf, wakeup: impl Fn() + Send + 'static) -> Result<Self> {
        fs::create_dir_all(&root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        }
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let id = std::env::var("FYLE_HARNESS_ID").unwrap_or(random_id()?);
        let descriptor = session_path(&root, &id)?;
        let session = Session {
            id,
            pid: std::process::id(),
            address: listener.local_addr()?.to_string(),
            token: random_id()?,
            version: env!("CARGO_PKG_VERSION").into(),
            revision: crate::diagnostics::revision().into(),
        };
        crate::diagnostics::write_bundle(&descriptor, &serde_json::to_value(&session)?)?;
        let (sender, incoming) = mpsc::sync_channel(1);
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = stopped.clone();
        let token = session.token.clone();
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                        let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
                        let result = (|| -> Result<Reply> {
                            let mut line = String::new();
                            {
                                let reader = BufReader::new(&mut stream);
                                reader.take(64 * 1024).read_line(&mut line)?;
                            }
                            let input: Value = serde_json::from_str(&line)?;
                            let provided = input["token"].as_str().unwrap_or("");
                            let valid = provided.len() == token.len()
                                && provided
                                    .bytes()
                                    .zip(token.bytes())
                                    .fold(0, |diff, (a, b)| diff | (a ^ b))
                                    == 0;
                            anyhow::ensure!(valid, "Session authentication failed");
                            if input["command"]["action"] == "ping" {
                                return Ok(Reply::ok(json!({"alive":true})));
                            }
                            let (respond, response) = mpsc::sync_channel(1);
                            sender
                                .try_send(Request {
                                    command: input["command"].clone(),
                                    respond,
                                })
                                .context("GUI request queue is busy")?;
                            wakeup();
                            response
                                .recv_timeout(Duration::from_secs(12))
                                .context("GUI did not respond; it may be blocked")
                        })()
                        .unwrap_or_else(Reply::error);
                        let _ = write_reply(&mut stream, result);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            session,
            incoming,
            stopped,
            descriptor,
        })
    }
    pub fn try_receive(&self) -> Option<Request> {
        self.incoming.try_recv().ok()
    }
    pub fn receive_timeout(&self, timeout: Duration) -> Option<Request> {
        self.incoming.recv_timeout(timeout).ok()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        let _ = fs::remove_file(&self.descriptor);
    }
}
fn write_reply(stream: &mut TcpStream, reply: Reply) -> Result<()> {
    let header = json!({"data":reply.data, "png_bytes":reply.png.len()});
    serde_json::to_writer(&mut *stream, &header)?;
    stream.write_all(b"\n")?;
    stream.write_all(&reply.png)?;
    Ok(())
}
pub fn request(session: &Session, command: Value) -> Result<Reply> {
    let address: std::net::SocketAddr = session.address.parse()?;
    anyhow::ensure!(
        address.ip().is_loopback(),
        "Session address must be loopback"
    );
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(15)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    serde_json::to_writer(
        &mut stream,
        &json!({"token":session.token,"command":command}),
    )?;
    stream.write_all(b"\n")?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    Read::by_ref(&mut reader)
        .take(MAX_HEADER)
        .read_line(&mut line)?;
    let header: Value = serde_json::from_str(&line)?;
    let data = header["data"].clone();
    if let Some(error) = data["error"].as_str() {
        bail!("{error}");
    }
    let length = usize::try_from(
        header["png_bytes"]
            .as_u64()
            .context("Invalid image length")?,
    )?;
    anyhow::ensure!(length <= MAX_PNG, "Screenshot is too large");
    let mut png = vec![0; length];
    reader.read_exact(&mut png)?;
    Ok(Reply { data, png })
}
pub fn load_session(root: &Path, id: &str) -> Result<Session> {
    let path = session_path(root, id)?;
    let session: Session = serde_json::from_reader(fs::File::open(path)?)?;
    anyhow::ensure!(session.id == id, "Session identifier mismatch");
    Ok(session)
}
pub fn sessions(root: &Path) -> Result<Vec<Session>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut sessions = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry.file_name().to_string_lossy().starts_with("session-")
        {
            if let Ok(session) =
                serde_json::from_reader::<_, Session>(fs::File::open(entry.path())?)
            {
                sessions.push(session);
            }
        }
    }
    Ok(sessions)
}
pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .context("Invalid image dimensions")?;
    anyhow::ensure!(
        pixels > 0 && pixels <= 16 * 1024 * 1024 && rgba.len() == pixels * 4,
        "Invalid or oversized screenshot"
    );
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(rgba)?;
    }
    Ok(bytes)
}
