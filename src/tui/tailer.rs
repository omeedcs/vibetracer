use crate::event::EditEvent;
use crate::snapshot::edit_log::EditLog;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::mpsc;

/// Load existing edits from an edit log and start tailing for new ones.
///
/// Returns the existing edits and a receiver for new events appended to the
/// file after the call. The tailing thread is a daemon thread that exits
/// when the receiver is dropped.
pub fn tail_edit_log(
    edit_log_path: PathBuf,
) -> anyhow::Result<(Vec<EditEvent>, mpsc::Receiver<EditEvent>)> {
    // Load existing edits.
    let existing = if edit_log_path.exists() {
        EditLog::read_all(&edit_log_path)?
    } else {
        Vec::new()
    };

    let (tx, rx) = mpsc::channel();

    // Spawn a daemon thread that watches the file for new lines.
    std::thread::Builder::new()
        .name("edit-log-tailer".into())
        .spawn(move || {
            // Open file, seek to end so we only see new appends.
            let file = match std::fs::File::open(&edit_log_path) {
                Ok(f) => f,
                Err(_) => {
                    // File might not exist yet if the daemon hasn't written
                    // anything. Wait for it to appear.
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        match std::fs::File::open(&edit_log_path) {
                            Ok(f) => break f,
                            Err(_) => continue,
                        }
                    }
                }
            };
            let mut reader = BufReader::new(file);
            let _ = reader.seek(SeekFrom::End(0));

            let mut line_buf = String::new();
            loop {
                line_buf.clear();
                match reader.read_line(&mut line_buf) {
                    Ok(0) => {
                        // No new data -- sleep briefly and retry.
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    Ok(_) => {
                        let trimmed = line_buf.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if let Ok(event) = serde_json::from_str::<EditEvent>(trimmed) {
                            if tx.send(event).is_err() {
                                return; // Receiver dropped -- TUI has exited.
                            }
                        }
                    }
                    Err(_) => {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                    }
                }
            }
        })?;

    Ok((existing, rx))
}
