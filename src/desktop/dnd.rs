//! External drag-and-drop payload policy: parsing incoming drops and building
//! outgoing drag payloads, without touching a windowing system.
//!
//! eframe 0.27 delivers OS file drops as already-decoded `DroppedFile` paths
//! and exposes no API for starting a native drag. These helpers are the
//! testable shim surface a future platform adapter (NSDraggingSession,
//! OLE `IDataObject`, XDND) would call for both directions.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use url::Url;

/// Byte length of the Windows `DROPFILES` header that precedes the file list.
const DROPFILES_HEADER: usize = 20;

/// Parse one `file://` URI, decoding percent escapes. Non-file and remote
/// URIs yield `None`, matching the launcher's local-file-only policy.
pub fn parse_single_uri(uri: &str) -> Option<PathBuf> {
    let url = Url::parse(uri.trim()).ok()?;
    if url.scheme() != "file" {
        return None;
    }
    url.to_file_path().ok()
}

/// Parse a `text/uri-list` payload (RFC 2483): one URI per CRLF/LF line.
/// `#` comment lines, blank lines and non-file or remote URIs are ignored.
pub fn parse_uri_list(payload: &str) -> Vec<PathBuf> {
    payload.lines().filter_map(parse_single_uri).collect()
}

/// Parse a Windows `CF_HDROP` blob: a `DROPFILES` header followed by
/// null-separated, double-null-terminated paths (UTF-16 when `fWide` is set,
/// ANSI bytes otherwise). Truncated or malformed payloads decode to no paths.
pub fn parse_cf_hdrop(payload: &[u8]) -> Vec<PathBuf> {
    let Some(header) = payload.get(..DROPFILES_HEADER) else {
        return Vec::new();
    };
    let offset = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
    let wide = i32::from_le_bytes([header[16], header[17], header[18], header[19]]) != 0;
    let Some(list) = payload.get(offset..) else {
        return Vec::new();
    };
    let items = if wide {
        read_wide_strings(list)
    } else {
        read_ansi_strings(list)
    };
    items
        .into_iter()
        .filter(|item| !item.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn read_wide_strings(bytes: &[u8]) -> Vec<String> {
    let mut units = Vec::new();
    let mut items = Vec::new();
    for chunk in bytes.as_chunks::<2>().0 {
        let unit = u16::from_le_bytes(*chunk);
        if unit != 0 {
            units.push(unit);
        } else if units.is_empty() {
            break; // double-null terminator
        } else {
            items.push(String::from_utf16_lossy(&units));
            units.clear();
        }
    }
    items
}

fn read_ansi_strings(bytes: &[u8]) -> Vec<String> {
    let mut current = Vec::new();
    let mut items = Vec::new();
    for &byte in bytes {
        if byte != 0 {
            current.push(byte);
        } else if current.is_empty() {
            break; // double-null terminator
        } else {
            items.push(String::from_utf8_lossy(&current).into_owned());
            current.clear();
        }
    }
    items
}

/// Negotiated drag effect. Drops copy by default; the platform move modifier
/// (Alt on macOS, Shift on Windows/Linux) promotes the drop to a move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragEffect {
    Copy,
    Move,
}

/// Pick the drop effect from modifier keys. Copy is the default.
pub fn drag_effect(alt: bool, shift: bool) -> DragEffect {
    if alt || shift {
        DragEffect::Move
    } else {
        DragEffect::Copy
    }
}

/// Validated incoming drop: the paths to transfer and the negotiated effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropIntent {
    pub paths: Vec<PathBuf>,
    pub effect: DragEffect,
}

/// Reduce parsed drop paths to a transfer intent for `destination`.
/// Paths equal to or inside the destination are already there and would
/// collide or recurse into themselves; duplicates collapse. Returns `None`
/// when nothing remains to transfer.
pub fn incoming_intent(
    paths: Vec<PathBuf>,
    destination: &Path,
    effect: DragEffect,
) -> Option<DropIntent> {
    let mut unique = Vec::new();
    let mut seen = HashSet::new();
    for path in paths {
        if path.as_path() == destination || path.starts_with(destination) {
            continue;
        }
        if seen.insert(path.clone()) {
            unique.push(path);
        }
    }
    (!unique.is_empty()).then_some(DropIntent {
        paths: unique,
        effect,
    })
}

/// Encode a path the way `CF_HDROP` file lists carry it: UTF-16 code units.
fn os_str_utf16(value: &std::ffi::OsStr) -> Vec<u16> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        value.encode_wide().collect()
    }
    #[cfg(not(windows))]
    {
        value.to_string_lossy().encode_utf16().collect()
    }
}

/// Build a `text/uri-list` payload (CRLF-terminated lines). Non-absolute
/// paths are skipped; an empty result means nothing can be dragged.
pub fn uri_list(paths: &[PathBuf]) -> String {
    let mut list = String::new();
    for path in paths {
        if let Ok(url) = Url::from_file_path(path) {
            list.push_str(url.as_str());
            list.push_str("\r\n");
        }
    }
    list
}

/// Build a `CF_HDROP` blob (`DROPFILES` header with wide-character paths).
pub fn cf_hdrop(paths: &[PathBuf]) -> Vec<u8> {
    let mut blob = vec![0u8; DROPFILES_HEADER];
    blob[0..4].copy_from_slice(&(DROPFILES_HEADER as u32).to_le_bytes());
    blob[16..20].copy_from_slice(&1i32.to_le_bytes()); // fWide: UTF-16 paths
    for path in paths {
        for unit in os_str_utf16(path.as_os_str()) {
            blob.extend_from_slice(&unit.to_le_bytes());
        }
        blob.extend_from_slice(&0u16.to_le_bytes());
    }
    blob.extend_from_slice(&0u16.to_le_bytes());
    blob
}

/// Everything a native drag shim hands to other applications:
/// `text/uri-list` for X11/Wayland and browsers, `CF_HDROP` bytes for
/// Windows, and the item count for the drag image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DragPayload {
    pub count: usize,
    pub uri_list: String,
    pub cf_hdrop: Vec<u8>,
}

/// Build the outgoing payload for dragging `paths` out of Fyle. Only absolute
/// paths can be dragged; an empty or relative selection yields `None`.
pub fn build_drag_payload(paths: &[PathBuf]) -> Option<DragPayload> {
    let draggable: Vec<PathBuf> = paths
        .iter()
        .filter(|path| Url::from_file_path(path).is_ok())
        .cloned()
        .collect();
    if draggable.is_empty() {
        return None;
    }
    Some(DragPayload {
        count: draggable.len(),
        uri_list: uri_list(&draggable),
        cf_hdrop: cf_hdrop(&draggable),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_parsing_keeps_local_hosts_and_skips_comments_and_schemes() {
        assert_eq!(
            parse_uri_list("file://localhost/tmp/a%20b.txt\r\n# note\r\n\n"),
            vec![PathBuf::from("/tmp/a b.txt")]
        );
        assert_eq!(
            parse_uri_list("https://example.com/a"),
            Vec::<PathBuf>::new()
        );
        assert_eq!(parse_uri_list("not a uri"), Vec::<PathBuf>::new());
        assert!(parse_single_uri("file://remote-host/x").is_none());
    }

    #[test]
    fn cf_hdrop_parses_ansi_lists_and_offsets() {
        let mut payload = vec![0u8; DROPFILES_HEADER];
        payload[0..4].copy_from_slice(&(DROPFILES_HEADER as u32).to_le_bytes());
        payload.extend_from_slice(b"C:\\a.txt\0C:\\b.txt\0\0");
        assert_eq!(
            parse_cf_hdrop(&payload),
            vec![PathBuf::from(r"C:\a.txt"), PathBuf::from(r"C:\b.txt")]
        );
    }

    #[test]
    fn drag_effect_copies_by_default_and_moves_with_modifier() {
        assert_eq!(drag_effect(false, false), DragEffect::Copy);
        assert_eq!(drag_effect(true, false), DragEffect::Move);
        assert_eq!(drag_effect(false, true), DragEffect::Move);
    }

    #[test]
    fn incoming_intent_filters_self_targets_and_duplicates() {
        let dir = Path::new("/data");
        let inside = PathBuf::from("/data/child");
        let outside = PathBuf::from("/other/a.txt");
        let intent = incoming_intent(
            vec![
                inside.clone(),
                dir.to_path_buf(),
                outside.clone(),
                outside.clone(),
            ],
            dir,
            DragEffect::Copy,
        )
        .unwrap();
        assert_eq!(intent.paths, vec![outside]);
        assert!(incoming_intent(Vec::new(), dir, DragEffect::Copy).is_none());
    }
}
