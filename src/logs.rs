use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::thread;
use std::time::Duration;

const LOG_POLL_INTERVAL: Duration = Duration::from_millis(150);
const MSG_NO_LOG: &str = "(no log yet)\n";
const MSG_UNABLE_TO_READ: &str = "(unable to read log file)\n";

pub fn read_tail(log_file: &Path, lines: usize) -> String {
    if !log_file.is_file() {
        return MSG_NO_LOG.to_string();
    }

    let content = match fs::read_to_string(log_file) {
        Ok(c) => c,
        Err(_) => return MSG_UNABLE_TO_READ.to_string(),
    };

    let all: Vec<&str> = content.lines().collect();
    if all.is_empty() {
        return String::new();
    }

    let start = if all.len() > lines {
        all.len() - lines
    } else {
        0
    };

    let mut result = all[start..].join("\n");
    result.push('\n');
    result
}

pub fn follow_log<F>(log_file: &Path, mut on_data: F)
where
    F: FnMut(&str),
{
    let mut pos = if log_file.is_file() {
        fs::metadata(log_file).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };

    loop {
        if log_file.is_file() {
            if let Ok(metadata) = fs::metadata(log_file) {
                let current_size = metadata.len();
                if current_size < pos {
                    pos = 0;
                }

                if current_size > pos {
                    if let Ok(mut file) = File::open(log_file) {
                        if file.seek(SeekFrom::Start(pos)).is_ok() {
                            let bytes_to_read = (current_size - pos) as usize;
                            let mut buffer = vec![0u8; bytes_to_read];
                            if file.read_exact(&mut buffer).is_ok() {
                                pos = current_size;
                                if let Ok(chunk_str) = std::str::from_utf8(&buffer) {
                                    on_data(chunk_str);
                                }
                            }
                        }
                    }
                }
            }
        }
        thread::sleep(LOG_POLL_INTERVAL);
    }
}
