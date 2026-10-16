//! Shared, independently testable launch path handling.
use anyhow::{bail, Context, Result};
use std::{ffi::OsStr, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchTarget {
    pub directory: PathBuf,
    pub file: Option<PathBuf>,
}

pub fn resolve_target(input: &OsStr) -> Result<LaunchTarget> {
    let path = if let Some(value) = input.to_str().filter(|s| s.starts_with("file:")) {
        let url = url::Url::parse(value)?;
        if url.host_str().is_some_and(|host| host != "localhost") {
            bail!("Only local file URLs are supported");
        }
        url.to_file_path()
            .map_err(|_| anyhow::anyhow!("Only local file URLs are supported"))?
    } else {
        if input.to_str().is_some_and(|s| s.contains("://")) {
            bail!("Remote URLs are not supported");
        }
        PathBuf::from(input)
    };
    let path = path
        .canonicalize()
        .with_context(|| format!("Cannot open {}", path.display()))?;
    if path.is_dir() {
        Ok(LaunchTarget {
            directory: path,
            file: None,
        })
    } else {
        Ok(LaunchTarget {
            directory: path
                .parent()
                .map(PathBuf::from)
                .context("File has no parent directory")?,
            file: Some(path),
        })
    }
}

pub fn resolve_location(input: &OsStr) -> Result<PathBuf> {
    Ok(resolve_target(input)?.directory)
}

/// Respect the option terminator so a folder named --automation is not an opt-in.
pub fn automation_requested(args: impl IntoIterator<Item = std::ffi::OsString>) -> bool {
    args.into_iter()
        .take_while(|arg| arg != "--")
        .any(|arg| arg == "--automation")
}

pub fn parse_args() -> LaunchTarget {
    let mut args = std::env::args_os().skip(1);
    let mut location = None;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--help" | "-h") => {
                println!("Fyle {}\nUsage: fyle [--home | --downloads | PATH | file:///PATH]\nUse fyle-cli doctor for desktop diagnostics.", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            Some("--version" | "-v") => {
                println!("Fyle {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            Some("--automation") => {}
            Some("--home") => {
                location = dirs::home_dir().map(|path| LaunchTarget {
                    directory: path,
                    file: None,
                })
            }
            Some("--downloads") => {
                location = dirs::download_dir().map(|path| LaunchTarget {
                    directory: path,
                    file: None,
                })
            }
            Some("--") => {
                if let Some(path) = args.next() {
                    location = Some(resolve_or_exit(&path));
                }
                break;
            }
            Some(s) if s.starts_with('-') => {
                eprintln!("Unknown option: {s}");
                std::process::exit(2);
            }
            _ => location = Some(resolve_or_exit(&arg)),
        }
    }
    location.unwrap_or_else(|| LaunchTarget {
        directory: dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")),
        file: None,
    })
}
fn resolve_or_exit(input: &OsStr) -> LaunchTarget {
    resolve_target(input).unwrap_or_else(|e| {
        eprintln!("{e:#}");
        std::process::exit(2)
    })
}
