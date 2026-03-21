use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

pub struct EmbeddedTerminal {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    reader: Arc<Mutex<Box<dyn Read + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    screen: Arc<Mutex<vt100::Parser>>,
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

        let parser = vt100::Parser::new(rows, cols, 0);

        Ok(Self {
            master: pair.master,
            child,
            reader: Arc::new(Mutex::new(reader)),
            writer: Arc::new(Mutex::new(writer)),
            screen: Arc::new(Mutex::new(parser)),
            cols,
            rows,
        })
    }

    /// Spawn a thread that reads PTY output and feeds it to the VT100 parser.
    pub fn start_reader(&self) -> std::thread::JoinHandle<()> {
        let reader = Arc::clone(&self.reader);
        let screen = Arc::clone(&self.screen);

        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                let n = {
                    let mut reader = reader.lock().unwrap();
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(_) => break,
                    }
                };
                let mut screen = screen.lock().unwrap();
                screen.process(&buf[..n]);
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
                    // For alphabetic chars, Ctrl+a=0x01 .. Ctrl+z=0x1a
                    // For special chars: Ctrl+\=0x1c, Ctrl+]=0x1d, Ctrl+^=0x1e, Ctrl+_=0x1f
                    if c.is_ascii_alphabetic() {
                        let ctrl = (c.to_ascii_lowercase() as u8) - b'a' + 1;
                        vec![ctrl]
                    } else {
                        match c {
                            '\\' => vec![0x1c],
                            ']' => vec![0x1d],
                            '^' => vec![0x1e],
                            '_' => vec![0x1f],
                            '@' => vec![0x00],
                            '[' => vec![0x1b],
                            _ => {
                                // Fallback: send the raw char
                                let mut buf = [0u8; 4];
                                let s = c.encode_utf8(&mut buf);
                                s.as_bytes().to_vec()
                            }
                        }
                    }
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

    /// Get the current screen contents as lines of text from the VT100 parser.
    pub fn get_screen_lines(&self, max_lines: usize) -> Vec<String> {
        let screen = self.screen.lock().unwrap();
        let s = screen.screen();
        let contents = s.contents();
        let mut lines: Vec<String> = contents.lines().map(|l| l.trim_end().to_string()).collect();

        // Trim trailing empty lines
        while lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }

        let start = lines.len().saturating_sub(max_lines);
        lines[start..].to_vec()
    }

    /// Get raw screen contents — each row as-is from the VT100 screen.
    pub fn get_output(&self, max_lines: usize) -> Vec<String> {
        self.get_screen_lines(max_lines)
    }

    /// Resize the PTY and the VT100 parser.
    pub fn resize(&self, cols: u16, rows: u16) -> anyhow::Result<()> {
        self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let mut screen = self.screen.lock().unwrap();
        screen.set_size(rows, cols);
        Ok(())
    }

    /// Check if the child process is still running.
    pub fn is_alive(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
    }
}
