//! Incremental, bounded log buffer for the TUI.
//!
//! Reads only the bytes appended since the last sync, keeps at most
//! `MAX_LINES` lines, and sanitizes control/ANSI sequences so they cannot
//! corrupt the terminal.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub const MAX_LINES: usize = 5000;
const INITIAL_TAIL_BYTES: u64 = 1024 * 1024;
const MAX_READ_BYTES: u64 = 4 * 1024 * 1024;
const MAX_LINE_CHARS: usize = 2000;

#[derive(Default)]
pub struct LogView {
    path: PathBuf,
    pos: u64,
    partial: String,
    lines: Vec<String>,
    /// Indices into `lines` matching the current filter (None = no filter).
    filtered: Option<Vec<usize>>,
    query_lower: String,
    placeholder: Option<String>,
}

impl LogView {
    /// Sync with file. Returns how many *visible* lines were appended
    /// (used to keep a scrolled view anchored). `reset` is true when the
    /// buffer was reloaded from scratch.
    pub fn sync(&mut self, path: &Path, query: &str) -> SyncResult {
        let mut result = SyncResult::default();
        let query_lower = query.to_lowercase();
        let query_changed = query_lower != self.query_lower;

        if path != self.path {
            self.path = path.to_path_buf();
            self.pos = 0;
            self.partial.clear();
            self.lines.clear();
            self.filtered = None;
            self.placeholder = None;
            result.reset = true;
            self.load_initial();
        } else if !self.path.as_os_str().is_empty() {
            match std::fs::metadata(&self.path) {
                Ok(meta) => {
                    let len = meta.len();
                    if len < self.pos {
                        // truncated / rotated
                        self.pos = 0;
                        self.partial.clear();
                        self.lines.clear();
                        result.reset = true;
                        self.load_initial();
                    } else if len > self.pos {
                        let before = self.lines.len();
                        self.read_from(self.pos, len.min(self.pos + MAX_READ_BYTES));
                        let added = self.lines.len().saturating_sub(before);
                        result.changed = added > 0;
                        result.appended = added;
                    }
                }
                Err(_) => {}
            }
        }

        let trimmed = self.trim();
        if trimmed {
            result.changed = true;
        }

        if query_changed || result.reset || result.changed {
            let old_len = self.visible_len();
            self.query_lower = query_lower;
            self.rebuild_filter();
            result.changed = true;
            if !query_changed && !result.reset {
                result.appended = self.visible_len().saturating_sub(old_len);
            }
        }
        result
    }

    fn load_initial(&mut self) {
        if !self.path.is_file() {
            self.placeholder = Some("(no logs recorded yet)".to_string());
            return;
        }
        let Ok(meta) = std::fs::metadata(&self.path) else {
            return;
        };
        let len = meta.len();
        let start = len.saturating_sub(INITIAL_TAIL_BYTES);
        self.read_from(start, len);
        if start > 0 && !self.lines.is_empty() {
            // first line is probably cut in the middle
            self.lines.remove(0);
        }
        if self.lines.is_empty() {
            self.placeholder = Some("(empty log)".to_string());
        }
    }

    fn read_from(&mut self, start: u64, end: u64) {
        let Ok(mut file) = File::open(&self.path) else {
            return;
        };
        if file.seek(SeekFrom::Start(start)).is_err() {
            return;
        }
        let mut buf = Vec::with_capacity((end - start) as usize);
        if file.take(end - start).read_to_end(&mut buf).is_err() {
            return;
        }
        self.pos = start + buf.len() as u64;
        self.placeholder = None;

        let text = String::from_utf8_lossy(&buf);
        let mut chunk = std::mem::take(&mut self.partial);
        chunk.push_str(&text);
        let mut rest = chunk.as_str();
        while let Some(i) = rest.find('\n') {
            self.lines.push(sanitize(&rest[..i]));
            rest = &rest[i + 1..];
        }
        self.partial = rest.to_string();
        if self.partial.len() > 64 * 1024 {
            let p = std::mem::take(&mut self.partial);
            self.lines.push(sanitize(&p));
        }
    }

    fn trim(&mut self) -> bool {
        if self.lines.len() > MAX_LINES + MAX_LINES / 4 {
            let drop = self.lines.len() - MAX_LINES;
            self.lines.drain(..drop);
            true
        } else {
            false
        }
    }

    fn rebuild_filter(&mut self) {
        self.filtered = if self.query_lower.is_empty() {
            None
        } else {
            Some(
                self.lines
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| l.to_lowercase().contains(&self.query_lower))
                    .map(|(i, _)| i)
                    .collect(),
            )
        };
    }

    pub fn visible_len(&self) -> usize {
        match &self.filtered {
            Some(f) => f.len(),
            None => self.lines.len() + usize::from(self.lines.is_empty() && self.placeholder.is_some()),
        }
    }

    /// Visible lines in `[start, end)` of the (filtered) view.
    pub fn slice(&self, start: usize, end: usize) -> Vec<&str> {
        if self.lines.is_empty() {
            return match (&self.placeholder, start == 0 && end > 0) {
                (Some(p), true) => vec![p.as_str()],
                _ => Vec::new(),
            };
        }
        match &self.filtered {
            Some(f) => f[start.min(f.len())..end.min(f.len())]
                .iter()
                .map(|&i| self.lines[i].as_str())
                .collect(),
            None => self.lines[start.min(self.lines.len())..end.min(self.lines.len())]
                .iter()
                .map(|s| s.as_str())
                .collect(),
        }
    }
}

#[derive(Default)]
pub struct SyncResult {
    pub changed: bool,
    pub reset: bool,
    pub appended: usize,
}

/// Strip ANSI escapes + control chars, expand tabs, cap length.
fn sanitize(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    let mut count = 0;
    while let Some(c) = chars.next() {
        if count >= MAX_LINE_CHARS {
            out.push('…');
            break;
        }
        match c {
            '\x1b' => {
                if chars.peek() == Some(&'[') {
                    chars.next();
                    for n in chars.by_ref() {
                        if ('@'..='~').contains(&n) {
                            break;
                        }
                    }
                } else {
                    chars.next();
                }
            }
            '\t' => {
                out.push_str("    ");
                count += 4;
            }
            '\r' => {}
            c if c.is_control() => {}
            c => {
                out.push(c);
                count += 1;
            }
        }
    }
    out
}
