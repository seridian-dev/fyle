//! Headless developer diagnostics; no graphical dependencies required.
mod support;
use anyhow::{bail, Context, Result};
use fyle::diagnostics::{doctor, write_bundle, DiagnosticsStore};
use std::{ffi::OsString, path::PathBuf};

const HELP: &str = "Fyle developer tools
  fyle-cli gui --help                      agent GUI control, state and screenshots
  fyle-cli doctor [--json]                 environment, build and desktop checks
  fyle-cli crashes [--latest] [--json]      local panic/startup/native-exit reports
  fyle-cli logs [--tail N] [--follow] [--json]
  fyle-cli debug bundle [--output PATH|-]   bounded, redacted JSON support bundle
  fyle-cli debug run [-- GUI_ARGS...]       launch GUI with full backtraces/debug logs
  fyle-cli debug paths                     locate local logs and updater data
  fyle-cli update [--check | --status | --rollback | --enable-auto | --disable-auto]
Nothing is uploaded automatically. Update commands require an installed launcher.";

fn main() -> Result<()> {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let store = DiagnosticsStore::default();
    match args.first().and_then(|arg| arg.to_str()) {
        None | Some("--help" | "-h" | "help") => println!("{HELP}"),
        Some("--version" | "-v") => println!("Fyle {} ({})", env!("CARGO_PKG_VERSION"), fyle::diagnostics::revision()),
        Some("gui") => support::gui::run(&args[1..])?,
        Some("doctor") => {
            check_flags(&args[1..], &["--json"])?;
            let info = doctor();
            if args[1..].contains(&"--json".into()) { print_json(&info)?; }
            else {
                println!("Fyle {} ({})", info["version"].as_str().unwrap_or(""), info["revision"].as_str().unwrap_or(""));
                for key in ["os", "arch", "build", "desktop", "desktop_environment", "session_type", "display_backend", "init_system", "init_process", "terminal", "xdg_open_available", "dbus_session_present", "diagnostics_directory"] {
                    println!("{key}: {}", info[key]);
                }
                println!("Use fyle-cli debug bundle to collect shareable diagnostics.");
            }
        }
        Some("crashes") => {
            check_flags(&args[1..], &["--json", "--latest"])?;
            let reports = store.crashes()?;
            if args[1..].contains(&"--latest".into()) {
                if let Some(report) = reports.first() { print_json(report)?; }
                else if args[1..].contains(&"--json".into()) { print_json(&serde_json::Value::Null)?; }
                else { println!("No local crash reports."); }
            } else if args[1..].contains(&"--json".into()) { print_json(&reports)?; }
            else if reports.is_empty() { println!("No local crash reports."); }
            else {
                for report in &reports { println!("{}  {}  {}  {}", report["timestamp"], report["kind"], report["version"], report["message"]); }
                println!("Use fyle-cli crashes --latest for the newest report and backtrace.");
            }
        }
        Some("logs") => logs(&store, &args[1..])?,
        Some("debug") => match args.get(1).and_then(|arg| arg.to_str()) {
            Some("paths") if args.len() == 2 => print_json(&serde_json::json!({"diagnostics": store.root, "updater": fyle::diagnostics::managed_root()}))?,
            Some("bundle") => {
                let output = match &args[2..] {
                    [] => OsString::from(format!("fyle-debug-{}.json", chrono::Utc::now().format("%Y%m%dT%H%M%S%.9f"))),
                    [flag, path] if flag == "--output" => path.clone(),
                    _ => bail!("Usage: fyle-cli debug bundle [--output PATH|-]"),
                };
                let bundle = store.bundle(dirs::home_dir().as_deref())?;
                if output == "-" { print_json(&bundle)?; }
                else { let path = PathBuf::from(output); write_bundle(&path, &bundle)?; println!("Debug bundle: {}\nHome paths and credential-like lines are redacted; review before sharing.", path.display()); }
            }
            Some("run") => debug_run(&store, &args[2..])?,
            _ => bail!("Usage: fyle-cli debug <bundle|run|paths>"),
        },
        Some("update") => bail!("Updates require a managed installation. Run ./install.sh, then use the installed fyle-cli update."),
        Some(command) => bail!("Unknown command: {command}\n{HELP}"),
    }
    Ok(())
}
fn print_json(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
fn check_flags(args: &[OsString], allowed: &[&str]) -> Result<()> {
    for arg in args {
        if !allowed.iter().any(|flag| arg == flag) {
            bail!("Unknown option: {}", arg.to_string_lossy());
        }
    }
    Ok(())
}
fn logs(store: &DiagnosticsStore, args: &[OsString]) -> Result<()> {
    let mut lines = 100;
    let mut follow = false;
    let mut json = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--tail") => {
                lines = args
                    .next()
                    .and_then(|arg| arg.to_str())
                    .context("--tail needs a line count")?
                    .parse::<usize>()?;
                anyhow::ensure!(
                    (1..=1000).contains(&lines),
                    "--tail must be between 1 and 1000"
                );
            }
            Some("--follow") => follow = true,
            Some("--json") => json = true,
            _ => bail!("Unknown logs option: {}", arg.to_string_lossy()),
        }
    }
    anyhow::ensure!(!(json && follow), "--json and --follow cannot be combined");
    let mut previous = std::collections::HashMap::<String, String>::new();
    loop {
        let logs = store.logs(lines)?;
        if json {
            return print_json(&logs);
        }
        if logs.is_empty() && !follow {
            println!("No local logs yet. Use fyle-cli debug run to reproduce a problem.");
        }
        previous.retain(|name, _| logs.iter().any(|log| &log.name == name));
        for log in logs {
            if previous.get(&log.name) != Some(&log.text) {
                println!("--- {} ---", log.name);
                if let Some(old) = previous
                    .get(&log.name)
                    .filter(|old| log.text.starts_with(old.as_str()))
                {
                    print!("{}", &log.text[old.len()..]);
                    println!();
                } else {
                    println!("{}", log.text);
                }
                previous.insert(log.name, log.text);
            }
        }
        if !follow {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    Ok(())
}
fn debug_run(store: &DiagnosticsStore, args: &[OsString]) -> Result<()> {
    let executable =
        std::env::current_exe()?.with_file_name(if cfg!(windows) { "fyle.exe" } else { "fyle" });
    anyhow::ensure!(
        executable.is_file(),
        "GUI binary not found beside fyle-cli; build/install the GUI first"
    );
    let args = args.strip_prefix(&[OsString::from("--")]).unwrap_or(args);
    eprintln!("Debugging Fyle; logs: {}", store.root.display());
    let mut child = std::process::Command::new(executable)
        .args(args)
        .env("RUST_BACKTRACE", "full")
        .env(
            "RUST_LOG",
            std::env::var("RUST_LOG").unwrap_or_else(|_| "warn,fyle=debug".into()),
        )
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let mirror = std::env::var_os("FYLE_HARNESS_ID").is_none();
    let stdout = capture_stream(
        child.stdout.take().context("Missing child stdout")?,
        store.clone(),
        "debug-stdout",
        mirror,
    );
    let stderr = capture_stream(
        child.stderr.take().context("Missing child stderr")?,
        store.clone(),
        "debug-stderr",
        mirror,
    );
    let status = child.wait()?;
    let _ = stdout.join();
    let _ = stderr.join();
    if !status.success() {
        let path = store.record_crash(
            "process_exit",
            &format!("Debug GUI exited with {status}"),
            "Native faults need a debugger/core dump for a native stack; see session logs",
        )?;
        bail!("GUI failed; report: {}", path.display());
    }
    Ok(())
}

fn capture_stream(
    mut stream: impl std::io::Read + Send + 'static,
    store: DiagnosticsStore,
    prefix: &'static str,
    mirror: bool,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        use std::io::Write;
        let mut writer = store.session_writer(prefix, mirror).ok();
        let mut buffer = [0; 8192];
        loop {
            let count = match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => count,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            };
            if let Some(output) = &mut writer {
                if output.write_all(&buffer[..count]).is_err() {
                    writer = None;
                }
            } else if mirror {
                let _ = std::io::stderr().write_all(&buffer[..count]);
            }
        }
        if let Some(mut writer) = writer {
            let _ = writer.flush();
        }
    })
}
