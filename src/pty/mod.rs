use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

pub struct EmbeddedTerminal {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    reader: Arc<Mutex<Box<dyn Read + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    output_buffer: Arc<Mutex<Vec<String>>>,
    #[allow(dead_code)]
    cols: u16,
    #[allow(dead_code)]
    rows: u16,
}

impl EmbeddedTerminal {
    /// Spawn a command in a PTY. Default: user's shell.
    pub fn new(cols: u16, rows: u16, command: Option<&str>) -> anyhow::Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let cmd = if let Some(cmd) = command {
            CommandBuilder::new(cmd)
        } else {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
            CommandBuilder::new(shell)
        };

        let child = pair.slave.spawn_command(cmd)?;
        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        Ok(Self {
            master: pair.master,
            child,
            reader: Arc::new(Mutex::new(reader)),
            writer: Arc::new(Mutex::new(writer)),
            output_buffer: Arc::new(Mutex::new(Vec::new())),
            cols,
            rows,
        })
    }

    /// Spawn a thread that reads PTY output into the buffer.
    pub fn start_reader(&self) -> std::thread::JoinHandle<()> {
        let reader = Arc::clone(&self.reader);
        let buffer = Arc::clone(&self.output_buffer);

        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                let mut reader = reader.lock().unwrap();
                match reader.read(&mut buf) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let text = String::from_utf8_lossy(&buf[..n]).to_string();
                        let mut buffer = buffer.lock().unwrap();
                        // Split into lines, append to buffer
                        for line in text.split('\n') {
                            if let Some(last) = buffer.last_mut() {
                                if !last.ends_with('\n') {
                                    last.push_str(line);
                                    continue;
                                }
                            }
                            buffer.push(line.to_string());
                        }
                        // Keep buffer bounded (last 1000 lines)
                        if buffer.len() > 1000 {
                            let drain = buffer.len() - 1000;
                            buffer.drain(..drain);
                        }
                    }
                    Err(_) => break,
                }
            }
        })
    }

    /// Send keyboard input to the PTY.
    pub fn send_input(&self, data: &[u8]) -> anyhow::Result<()> {
        let mut writer = self.writer.lock().unwrap();
        writer.write_all(data)?;
        writer.flush()?;
        Ok(())
    }

    /// Send a key event (converting crossterm KeyEvent to terminal bytes).
    pub fn send_key(&self, key: crossterm::event::KeyEvent) -> anyhow::Result<()> {
        use crossterm::event::{KeyCode, KeyModifiers};

        let bytes: Vec<u8> = match key.code {
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    // Ctrl+C = 0x03, Ctrl+D = 0x04, etc.
                    let ctrl = (c as u8).wrapping_sub(b'a').wrapping_add(1);
                    vec![ctrl]
                } else {
                    let mut buf = [0u8; 4];
                    let s = c.encode_utf8(&mut buf);
                    s.as_bytes().to_vec()
                }
            }
            KeyCode::Enter => vec![b'\r'],
            KeyCode::Backspace => vec![0x7f],
            KeyCode::Tab => vec![b'\t'],
            KeyCode::Esc => vec![0x1b],
            KeyCode::Up => vec![0x1b, b'[', b'A'],
            KeyCode::Down => vec![0x1b, b'[', b'B'],
            KeyCode::Right => vec![0x1b, b'[', b'C'],
            KeyCode::Left => vec![0x1b, b'[', b'D'],
            KeyCode::Home => vec![0x1b, b'[', b'H'],
            KeyCode::End => vec![0x1b, b'[', b'F'],
            KeyCode::Delete => vec![0x1b, b'[', b'3', b'~'],
            _ => return Ok(()),
        };
        self.send_input(&bytes)
    }

    /// Get the current terminal output (last N lines).
    pub fn get_output(&self, max_lines: usize) -> Vec<String> {
        let buffer = self.output_buffer.lock().unwrap();
        let start = buffer.len().saturating_sub(max_lines);
        buffer[start..].to_vec()
    }

    /// Resize the PTY.
    pub fn resize(&self, cols: u16, rows: u16) -> anyhow::Result<()> {
        self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }

    /// Check if the child process is still running.
    pub fn is_alive(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
    }
}
