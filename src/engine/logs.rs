use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;
use std::thread;
use std::time::Duration;

pub fn read_tail(path: &Path, lines_count: usize) -> String {
    let Ok(file) = File::open(path) else {
        return "(empty log)".to_string();
    };
    let reader = BufReader::new(file);
    let all_lines: Vec<String> = reader.lines().map_while(Result::ok).collect();
    if all_lines.is_empty() {
        return "(empty log)".to_string();
    }
    let start = all_lines.len().saturating_sub(lines_count);
    all_lines[start..].join("\n")
}

pub fn stream_logs(path: &Path, tail_lines: usize, follow: bool) -> Result<()> {
    if !path.is_file() {
        println!("Log file {:?} does not exist yet.", path);
        return Ok(());
    }

    let initial = read_tail(path, tail_lines);
    if !initial.is_empty() && initial != "(empty log)" {
        println!("{}", initial);
    }

    if !follow {
        return Ok(());
    }

    let mut file =
        File::open(path).with_context(|| format!("Failed to open log file {:?}", path))?;
    file.seek(SeekFrom::End(0))?;
    let mut reader = BufReader::new(file);

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => {
                thread::sleep(Duration::from_millis(200));
            }
            Ok(_) => {
                print!("{}", line);
            }
            Err(e) => {
                eprintln!("Error reading log: {}", e);
                break;
            }
        }
    }
    Ok(())
}
