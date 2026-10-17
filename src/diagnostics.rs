//! Local-only diagnostics. Collection is bounded and exports are explicitly requested.
use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

const MAX_LOG: u64 = 1024 * 1024;
const MAX_REPORT: usize = 256 * 1024;
const MAX_TAIL: usize = 16 * 1024;

#[derive(Clone)]
pub struct DiagnosticsStore {
    pub root: PathBuf,
}

#[derive(Serialize)]
pub struct LogExcerpt {
    pub name: String,
    pub text: String,
}

impl Default for DiagnosticsStore {
    fn default() -> Self {
        Self::new(
            std::env::var_os("FYLE_DIAGNOSTICS_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    dirs::data_local_dir()
                        .unwrap_or_else(std::env::temp_dir)
                        .join("fyle/diagnostics")
                }),
        )
    }
}
impl DiagnosticsStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
    fn prepare(&self) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }
    pub fn session_writer(&self, prefix: &str, mirror: bool) -> Result<RollingWriter> {
        anyhow::ensure!(
            !prefix.is_empty()
                && prefix.len() <= 64
                && prefix
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "Invalid log prefix"
        );
        self.prepare()?;
        let path = self
            .root
            .join(format!("{prefix}-{}-{}.log", stamp(), std::process::id()));
        let mut writer = RollingWriter::new(path, MAX_LOG)?;
        writer.mirror = mirror;
        self.prune("", ".log", 20);
        Ok(writer)
    }
    pub fn record_crash(&self, kind: &str, message: &str, backtrace: &str) -> Result<PathBuf> {
        self.prepare()?;
        let path = self
            .root
            .join(format!("crash-{}-{}.json", stamp(), std::process::id()));
        let report = json!({
            "schema_version": 1, "kind": bounded(kind, 64), "timestamp": chrono::Utc::now().to_rfc3339(),
            "version": env!("CARGO_PKG_VERSION"), "revision": revision(), "pid": std::process::id(),
            "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
            "message": bounded(message, 16 * 1024), "backtrace": bounded(backtrace, 16 * 1024),
        });
        write_bundle(&path, &report)?;
        self.prune("crash-", ".json", 20);
        Ok(path)
    }
    pub fn crashes(&self) -> Result<Vec<Value>> {
        Ok(self
            .files("crash-", ".json")?
            .into_iter()
            .take(20)
            .filter_map(|path| read_bounded(&path, MAX_REPORT).ok())
            .filter_map(|text| serde_json::from_str(&text).ok())
            .collect())
    }
    pub fn logs(&self, lines: usize) -> Result<Vec<LogExcerpt>> {
        Ok(self
            .files("", ".log")?
            .into_iter()
            .take(5)
            .filter_map(|path| {
                tail(&path, MAX_TAIL).ok().map(|text| LogExcerpt {
                    name: path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    text: last_lines(&text, lines.min(1000)),
                })
            })
            .collect())
    }
    pub fn bundle(&self, home: Option<&Path>) -> Result<Value> {
        let root = managed_root();
        let update_status = read_bounded(&root.join("status.json"), MAX_REPORT)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok());
        let mut bundle = json!({
            "schema_version": 1, "created_at": chrono::Utc::now().to_rfc3339(),
            "doctor": doctor(), "crashes": self.crashes()?.into_iter().take(10).collect::<Vec<_>>(),
            "logs": self.logs(200)?, "update_status": update_status,
            "update_log_tail": tail(&root.join("update.log"), MAX_TAIL).ok(),
            "collection": "Local Fyle diagnostics only; no document contents or full environment collected",
        });
        redact_value(&mut bundle, home);
        Ok(bundle)
    }
    fn files(&self, prefix: &str, suffix: &str) -> Result<Vec<PathBuf>> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut paths = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(prefix) && name.ends_with(suffix) && entry.file_type()?.is_file() {
                let modified = entry
                    .metadata()
                    .and_then(|metadata| metadata.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);
                paths.push((modified, entry.path()));
            }
        }
        paths.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
        Ok(paths.into_iter().map(|(_, path)| path).collect())
    }
    fn prune(&self, prefix: &str, suffix: &str, keep: usize) {
        if let Ok(paths) = self.files(prefix, suffix) {
            for path in paths.into_iter().skip(keep) {
                let _ = fs::remove_file(path);
            }
        }
    }
}

pub fn revision() -> &'static str {
    option_env!("FYLE_BUILD_REVISION").unwrap_or("unknown")
}
pub fn managed_root() -> PathBuf {
    std::env::var_os("FYLE_MANAGED_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::data_local_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join("fyle")
        })
}
pub fn doctor() -> Value {
    let present = |key| std::env::var_os(key).is_some_and(|value| !value.is_empty());
    let pid1 = read_bounded(Path::new("/proc/1/comm"), 128)
        .ok()
        .map(|s| s.trim().to_owned());
    let openrc_marker = Path::new("/run/openrc").exists();
    let init_process = pid1.unwrap_or_else(|| "unknown".to_owned());
    let init_system = crate::desktop::detect_init_system(openrc_marker, &init_process);
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let session_desktop = std::env::var("XDG_SESSION_DESKTOP").unwrap_or_default();
    let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let wayland_present = present("WAYLAND_DISPLAY");
    let display_present = present("DISPLAY");
    let session = crate::desktop::detect_session(
        &desktop,
        &session_desktop,
        &session_type,
        present("HYPRLAND_INSTANCE_SIGNATURE"),
        present("SWAYSOCK"),
        wayland_present,
        display_present,
    );
    json!({
        "version": env!("CARGO_PKG_VERSION"), "revision": revision(),
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "build": if cfg!(debug_assertions) { "debug" } else { "release" },
        "features": { "gui": cfg!(feature = "gui"), "trash": cfg!(feature = "trash") },
        "desktop": desktop, "session_desktop": session_desktop,
        "desktop_environment": session.environment.as_str(),
        "session_type": session_type, "display_backend": session.display.as_str(),
        "display_present": display_present, "wayland_present": wayland_present,
        "dbus_session_present": present("DBUS_SESSION_BUS_ADDRESS"),
        "init_system": init_system, "init_process": init_process,
        "terminal": crate::core::get_system_terminal(),
        "xdg_open_available": crate::core::is_application_available("xdg-open"),
        "diagnostics_directory": DiagnosticsStore::default().root,
        "kernel": read_bounded(Path::new("/proc/sys/kernel/osrelease"), 256).ok().map(|s| s.trim().to_owned()),
    })
}

/// Store explicit exports exclusively, so an existing user file is never truncated.
pub fn write_bundle(path: &Path, bundle: &Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(bundle)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut output = options
        .open(path)
        .with_context(|| format!("Cannot create {}", path.display()))?;
    if let Err(error) = output.write_all(&bytes).and_then(|_| output.sync_all()) {
        drop(output);
        let _ = fs::remove_file(path);
        return Err(error.into());
    }
    Ok(())
}

pub fn init_gui() {
    let store = DiagnosticsStore::default();
    let mut logger = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn,fyle=info"),
    );
    logger.write_style(env_logger::WriteStyle::Never);
    if store.prepare().is_ok() {
        let path = store
            .root
            .join(format!("gui-{}-{}.log", stamp(), std::process::id()));
        if let Ok(writer) = RollingWriter::new(path, MAX_LOG) {
            store.prune("", ".log", 20);
            logger.target(env_logger::Target::Pipe(Box::new(writer)));
        }
    }
    let _ = logger.try_init();
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let trace = std::backtrace::Backtrace::force_capture().to_string();
        if let Ok(path) = store.record_crash("panic", &panic.to_string(), &trace) {
            eprintln!("Fyle crash report: {}", path.display());
        }
        previous(panic);
    }));
}

/// One session per process, with a bounded previous segment. Also mirrors to stderr.
pub struct RollingWriter {
    path: PathBuf,
    file: fs::File,
    size: u64,
    limit: u64,
    mirror: bool,
}
impl RollingWriter {
    pub fn new(path: PathBuf, limit: u64) -> std::io::Result<Self> {
        let mut options = fs::OpenOptions::new();
        options.create(true).read(true).write(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&path)?;
        let size = file.metadata()?.len();
        Ok(Self {
            path,
            file,
            size,
            limit: limit.max(1),
            mirror: true,
        })
    }
}
impl Write for RollingWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let original = bytes.len();
        if self.size + bytes.len() as u64 > self.limit {
            // Copy/truncate keeps the writer open and is portable across Unix/Windows.
            let previous = self.path.with_extension("previous.log");
            fs::copy(&self.path, previous)?;
            self.file.set_len(0)?;
            self.size = 0;
        }
        let bytes = &bytes[bytes.len().saturating_sub(self.limit as usize)..];
        self.file.seek(SeekFrom::End(0))?;
        self.file.write_all(bytes)?;
        self.size += bytes.len() as u64;
        if self.mirror {
            let _ = std::io::stderr().write_all(bytes);
        }
        Ok(original)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

fn stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}
fn bounded(text: &str, max: usize) -> String {
    let mut end = text.len().min(max);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}
fn read_bounded(path: &Path, max: usize) -> Result<String> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(max as u64)
        .read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
fn tail(path: &Path, max: usize) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let length = file.metadata()?.len();
    file.seek(SeekFrom::Start(length.saturating_sub(max as u64)))?;
    let mut bytes = Vec::new();
    file.take(max as u64).read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
fn last_lines(text: &str, lines: usize) -> String {
    let lines: Vec<_> = text.lines().rev().take(lines).collect();
    lines.into_iter().rev().collect::<Vec<_>>().join("\n")
}
fn redact_value(value: &mut Value, home: Option<&Path>) {
    match value {
        Value::String(text) => {
            *text = text
                .lines()
                .map(|line| {
                    let lower = line.to_ascii_lowercase();
                    if [
                        "password",
                        "passwd",
                        "authorization",
                        "bearer ",
                        "api_key",
                        "api-key",
                        "access_token",
                        "secret=",
                        "token=",
                    ]
                    .iter()
                    .any(|marker| lower.contains(marker))
                    {
                        return "[credential-like line redacted]".into();
                    }
                    home.map(|p| line.replace(p.to_string_lossy().as_ref(), "$HOME"))
                        .unwrap_or_else(|| line.to_owned())
                })
                .collect::<Vec<_>>()
                .join("\n");
        }
        Value::Array(values) => {
            for value in values {
                redact_value(value, home);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                redact_value(value, home);
            }
        }
        _ => {}
    }
}
