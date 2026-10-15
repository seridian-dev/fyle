//! Stable JSON CLI for agent-owned GUI sessions.
use anyhow::{bail, Context, Result};
use fyle::harness::{self, Session};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const HELP: &str = "Fyle GUI harness (opt-in, this app only)
  gui launch [--path DIRECTORY] [--timeout SECONDS]
  gui sessions
  gui state [--session ID] [--offset N] [--limit N]
  gui wait [--session ID] [--timeout SECONDS]
  gui screenshot [--session ID] [--output image.png]
  gui action <navigate|back|forward|up|refresh|search|view|select|open_folder|toggle_hidden|new_folder|new_file|dismiss_error|cancel>
      [--path PATH | --query TEXT | --mode list|grid | --name NAME] [--session ID]
  gui click --x X --y Y --snapshot ID [--button left|right] [--session ID]
  gui text --text TEXT [--snapshot ID] [--session ID]
  gui key --key Ctrl+H [--snapshot ID] [--session ID]
  gui scroll --x X --y Y --dy DELTA --snapshot ID [--session ID]
  gui close [--session ID]
State and screenshot JSON include snapshot_id. Pixel input uses screenshot pixels.
Use fresh snapshots; stale input is rejected. Every response is JSON.";

type Options = BTreeMap<String, OsString>;
fn parse(args: &[OsString]) -> Result<Options> {
    let mut options = Options::new();
    let mut args = args.iter();
    while let Some(key) = args.next() {
        let key = key.to_str().context("Option names must be UTF-8")?;
        anyhow::ensure!(key.starts_with("--"), "Expected an option, got {key}");
        let value = args.next().context("Option needs a value")?;
        anyhow::ensure!(
            options.insert(key.into(), value.clone()).is_none(),
            "Duplicate option: {key}"
        );
    }
    Ok(options)
}
fn text<'a>(options: &'a Options, key: &str) -> Result<&'a str> {
    options
        .get(key)
        .and_then(|value| value.to_str())
        .with_context(|| format!("Missing {key}"))
}
fn number(options: &Options, key: &str) -> Result<f64> {
    let number = text(options, key)?.parse::<f64>()?;
    anyhow::ensure!(number.is_finite(), "Invalid numeric value");
    Ok(number)
}
fn allowed(options: &Options, keys: &[&str]) -> Result<()> {
    for key in options.keys() {
        anyhow::ensure!(
            key == "--session" || keys.contains(&key.as_str()),
            "Unknown option {key}"
        );
    }
    Ok(())
}
fn timeout(options: &Options) -> Result<Duration> {
    let seconds = options
        .get("--timeout")
        .map(|value| {
            value
                .to_str()
                .context("Invalid timeout")?
                .parse::<u64>()
                .context("Invalid timeout")
        })
        .transpose()?
        .unwrap_or(15);
    anyhow::ensure!(
        (1..=60).contains(&seconds),
        "Timeout must be between 1 and 60 seconds"
    );
    Ok(Duration::from_secs(seconds))
}
fn select(options: &Options) -> Result<Session> {
    if options.contains_key("--session") {
        return harness::load_session(&harness::directory(), text(options, "--session")?);
    }
    let sessions: Vec<_> = harness::sessions(&harness::directory())?
        .into_iter()
        .filter(|session| harness::request(session, json!({"action":"ping"})).is_ok())
        .collect();
    anyhow::ensure!(
        sessions.len() == 1,
        "Expected exactly one active session; use gui sessions and --session ID"
    );
    Ok(sessions[0].clone())
}
fn wait(session: &Session, timeout: Duration) -> Result<Value> {
    let deadline = Instant::now() + timeout;
    loop {
        let state = harness::request(session, json!({"action":"state"}))?.data;
        if state["ready"] == true {
            return Ok(state);
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "GUI did not become ready before timeout"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}
pub fn run(args: &[OsString]) -> Result<()> {
    let action = args
        .first()
        .and_then(|arg| arg.to_str())
        .unwrap_or("--help");
    if matches!(action, "--help" | "-h" | "help") {
        println!("{HELP}");
        return Ok(());
    }
    let (semantic, rest) = if action == "action" {
        (
            Some(
                args.get(1)
                    .and_then(|arg| arg.to_str())
                    .context("GUI action name required")?,
            ),
            &args[2..],
        )
    } else {
        (None, &args[1..])
    };
    let options = parse(rest)?;
    if action == "launch" {
        allowed(&options, &["--path", "--timeout"])?;
        let root = harness::directory();
        let id = harness::random_id()?;
        let diagnostics = fyle::diagnostics::DiagnosticsStore::default().root;
        fs::create_dir_all(&diagnostics)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&diagnostics, fs::Permissions::from_mode(0o700))?;
        }
        let log_path = diagnostics.join(format!("harness-launch-{id}.log"));
        let mut log_options = fs::OpenOptions::new();
        log_options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            log_options.mode(0o600);
        }
        let log = log_options.open(&log_path)?;
        let mut command = Command::new(std::env::current_exe()?);
        command.args(["debug", "run", "--"]);
        if let Some(path) = options.get("--path") {
            command.arg(path);
        } else {
            command.arg("--home");
        }
        command
            .env("FYLE_AUTOMATION", "1")
            .env("FYLE_HARNESS_ID", &id)
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command.spawn()?;
        let deadline = Instant::now() + timeout(&options)?;
        let session = loop {
            if let Ok(session) = harness::load_session(&root, &id) {
                break session;
            }
            if let Some(status) = child.try_wait()? {
                let store = fyle::diagnostics::DiagnosticsStore::default();
                let reports = store.crashes().unwrap_or_default();
                let recent: Vec<_> = reports
                    .into_iter()
                    .take(3)
                    .map(|report| json!({"kind":report["kind"],"message":report["message"]}))
                    .collect();
                let logs = store.logs(30).unwrap_or_default();
                bail!(
                    "GUI launch failed ({status}); see {}\nRecent reports: {}\nRecent logs: {}",
                    log_path.display(),
                    serde_json::to_string(&recent)?,
                    serde_json::to_string(&logs)?
                );
            }
            anyhow::ensure!(
                Instant::now() < deadline,
                "GUI launch timed out; see {}",
                log_path.display()
            );
            std::thread::sleep(Duration::from_millis(50));
        };
        let state = wait(&session, timeout(&options)?)?;
        println!(
            "{}",
            serde_json::to_string_pretty(
                &json!({"session":session.id,"pid":session.pid,"state":state,"launch_log":log_path})
            )?
        );
        return Ok(());
    }
    if action == "sessions" {
        allowed(&options, &[])?;
        let sessions: Vec<_> = harness::sessions(&harness::directory())?.into_iter().map(|session| {
            let alive = harness::request(&session,json!({"action":"ping"})).is_ok();
            json!({"session":session.id,"pid":session.pid,"version":session.version,"revision":session.revision,"alive":alive})
        }).collect();
        println!("{}", serde_json::to_string_pretty(&sessions)?);
        return Ok(());
    }
    let session = select(&options)?;
    if action == "wait" {
        allowed(&options, &["--timeout"])?;
        println!(
            "{}",
            serde_json::to_string_pretty(&wait(&session, timeout(&options)?)?)?
        );
        return Ok(());
    }
    let mut request = json!({"action":semantic.unwrap_or(action)});
    match action {
        "state" => {
            allowed(&options, &["--offset", "--limit"])?;
            for (flag, key) in [("--offset", "offset"), ("--limit", "limit")] {
                if options.contains_key(flag) {
                    request[key] = json!(text(&options, flag)?.parse::<u64>()?);
                }
            }
        }
        "screenshot" => allowed(&options, &["--output"])?,
        "action" => {
            allowed(
                &options,
                &["--path", "--query", "--mode", "--name", "--snapshot"],
            )?;
            for (flag, key) in [
                ("--path", "path"),
                ("--query", "query"),
                ("--mode", "mode"),
                ("--name", "name"),
            ] {
                if options.contains_key(flag) {
                    request[key] = json!(text(&options, flag)?);
                }
            }
        }
        "click" | "scroll" => {
            allowed(&options, &["--x", "--y", "--snapshot", "--button", "--dy"])?;
            request["x"] = json!(number(&options, "--x")?);
            request["y"] = json!(number(&options, "--y")?);
            if action == "scroll" {
                request["dy"] = json!(number(&options, "--dy")?);
            }
            if options.contains_key("--button") {
                request["button"] = json!(text(&options, "--button")?);
            }
            anyhow::ensure!(
                options.contains_key("--snapshot"),
                "Pixel input requires --snapshot ID"
            );
        }
        "text" => {
            allowed(&options, &["--text", "--snapshot"])?;
            request["text"] = json!(text(&options, "--text")?);
        }
        "key" => {
            allowed(&options, &["--key", "--snapshot"])?;
            request["key"] = json!(text(&options, "--key")?);
        }
        "close" => allowed(&options, &[])?,
        _ => bail!("Unknown GUI command: {action}\n{HELP}"),
    }
    if options.contains_key("--snapshot") {
        request["snapshot_id"] = json!(text(&options, "--snapshot")?.parse::<u64>()?);
    }
    let mut reply = harness::request(&session, request)?;
    if action == "screenshot" {
        anyhow::ensure!(!reply.png.is_empty(), "GUI returned no screenshot pixels");
        let path = options
            .get("--output")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(format!(
                    "fyle-gui-{}.png",
                    chrono::Utc::now().format("%Y%m%dT%H%M%S%.9f")
                ))
            });
        let mut options = fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&path)
            .with_context(|| format!("Cannot create {}", path.display()))?;
        file.write_all(&reply.png)?;
        file.sync_all()?;
        reply.data["path"] = json!(path.canonicalize()?);
    }
    println!("{}", serde_json::to_string_pretty(&reply.data)?);
    Ok(())
}
