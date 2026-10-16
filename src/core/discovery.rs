//! Local query policy and deterministic fuzzy ranking, independent of GUI and I/O.
use crate::types::FileEntry;
use anyhow::{bail, ensure, Result};

#[derive(Debug)]
enum Filter {
    Name(String),
    Extension(Vec<String>),
    Directory(bool),
    Size(char, u64),
}
#[derive(Debug, Default)]
pub struct Query(Vec<(bool, Filter)>);
impl Query {
    /// Terms are ANDed. Quotes group a name; a leading minus negates any term.
    pub fn parse(input: &str) -> Result<Self> {
        let mut terms = Vec::new();
        let mut token = String::new();
        let mut quoted = false;
        for ch in input.chars() {
            match ch {
                '"' => quoted = !quoted,
                ch if ch.is_whitespace() && !quoted => {
                    if !token.is_empty() {
                        terms.push(std::mem::take(&mut token));
                    }
                }
                ch => token.push(ch),
            }
        }
        ensure!(!quoted, "Close the quote to finish this name filter");
        if !token.is_empty() {
            terms.push(token);
        }
        let mut filters = Vec::new();
        for term in terms {
            let (negative, term) = term
                .strip_prefix('-')
                .map_or((false, term.as_str()), |s| (true, s));
            let term = term.to_lowercase();
            let filter = if let Some(extension) = term.strip_prefix("ext:") {
                ensure!(!extension.is_empty(), "Use ext:rs or ext:png");
                let extensions: Vec<_> = extension
                    .split(',')
                    .map(|ext| ext.trim_start_matches('.').to_owned())
                    .collect();
                ensure!(
                    extensions.iter().all(|ext| !ext.is_empty()),
                    "Use ext:rs or ext:rs,py"
                );
                Filter::Extension(extensions)
            } else if let Some(kind) = term.strip_prefix("kind:") {
                Filter::Directory(match kind {
                    "folder" | "dir" => true,
                    "file" => false,
                    _ => bail!("Use kind:file or kind:folder"),
                })
            } else if let Some(size) = term.strip_prefix("size:") {
                let (comparison, size) = match size.chars().next() {
                    Some(ch @ ('>' | '<' | '=')) => (ch, &size[1..]),
                    _ => ('=', size),
                };
                let digits = size
                    .find(|c: char| !c.is_ascii_digit())
                    .unwrap_or(size.len());
                let value: u64 = size[..digits]
                    .parse()
                    .map_err(|_| anyhow::anyhow!("Use size:>10mb, size:<1kb or size:0"))?;
                let factor = match &size[digits..] {
                    "" | "b" => 1,
                    "k" | "kb" | "kib" => 1024,
                    "m" | "mb" | "mib" => 1024 * 1024,
                    "g" | "gb" | "gib" => 1024 * 1024 * 1024,
                    _ => bail!("Size units: b, kb, mb, gb (powers of 1024)"),
                };
                Filter::Size(
                    comparison,
                    value
                        .checked_mul(factor)
                        .ok_or_else(|| anyhow::anyhow!("Size is too large"))?,
                )
            } else {
                Filter::Name(term)
            };
            filters.push((negative, filter));
        }
        Ok(Self(filters))
    }
    pub fn matches(&self, entry: &FileEntry) -> bool {
        let mut name = None;
        self.0.iter().all(|(negative, filter)| {
            let matches = match filter {
                Filter::Name(term) => name
                    .get_or_insert_with(|| entry.name.to_lowercase())
                    .contains(term),
                Filter::Extension(extensions) => {
                    !entry.is_dir
                        && entry.path.extension().is_some_and(|ext| {
                            extensions.contains(&ext.to_string_lossy().to_lowercase())
                        })
                }
                Filter::Directory(is_dir) => entry.is_dir == *is_dir,
                Filter::Size(comparison, value) => {
                    !entry.is_dir
                        && match comparison {
                            '>' => entry.size > *value,
                            '<' => entry.size < *value,
                            _ => entry.size == *value,
                        }
                }
            };
            matches != *negative
        })
    }
}

/// Higher is better. Exact and prefix matches precede subsequences; stable ties
/// are resolved by the caller, so file order never jitters between frames.
pub fn fuzzy_score(query: &str, candidate: &str) -> Option<i64> {
    let query = query.to_lowercase();
    let candidate = candidate.to_lowercase();
    if query.is_empty() {
        return Some(0);
    }
    if candidate == query {
        return Some(1_000_000);
    }
    if candidate.starts_with(&query) {
        return Some(500_000 - candidate.chars().count() as i64);
    }
    let mut remaining = query.chars();
    let mut wanted = remaining.next()?;
    let mut score = 0;
    let mut previous = None;
    for (index, ch) in candidate.chars().enumerate() {
        if ch == wanted {
            score += if previous == Some(index.saturating_sub(1)) {
                20
            } else {
                1
            };
            score -= index as i64;
            previous = Some(index);
            if let Some(next) = remaining.next() {
                wanted = next;
            } else {
                return Some(score);
            }
        }
    }
    None
}
