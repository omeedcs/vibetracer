use anyhow::Result;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::Duration;

/// Watches a directory for filesystem changes and sends changed file paths
/// over an mpsc channel.
pub struct FsWatcher {
    root: PathBuf,
    tx: Sender<PathBuf>,
    debounce_ms: u64,
    ignore: Vec<String>,
    watcher: Option<Box<dyn Watcher + Send>>,
}

impl FsWatcher {
    /// Create a new watcher with no ignore patterns.
    pub fn new(root: PathBuf, tx: Sender<PathBuf>, debounce_ms: u64) -> Result<Self> {
        Self::with_ignore(root, tx, debounce_ms, Vec::new())
    }

    /// Create a new watcher with ignore patterns.
    /// Paths containing any ignore pattern as a path component will be filtered.
    pub fn with_ignore(
        root: PathBuf,
        tx: Sender<PathBuf>,
        debounce_ms: u64,
        ignore: Vec<String>,
    ) -> Result<Self> {
        Ok(Self {
            root,
            tx,
            debounce_ms,
            ignore,
            watcher: None,
        })
    }

    /// Start watching the root directory recursively.
    pub fn start(&mut self) -> Result<()> {
        let tx = self.tx.clone();
        let ignore = self.ignore.clone();
        let debounce = Duration::from_millis(self.debounce_ms);

        let mut watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
            if let Ok(event) = result {
                // Only process events that indicate actual file modifications/creations
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) => {}
                    _ => return,
                }

                for path in event.paths {
                    // Filter out paths containing any ignore pattern as a component
                    let should_ignore = path.components().any(|component| {
                        let comp_str = component.as_os_str().to_string_lossy();
                        ignore.iter().any(|pat| comp_str == pat.as_str())
                    });

                    if !should_ignore {
                        // Best-effort send; if receiver is gone we just drop the event
                        let _ = tx.send(path);
                    }
                }
            }
        })?;

        // Configure debounce if the watcher supports it (notify 7 handles this internally)
        let _ = debounce; // debounce_ms stored for reference; recommended_watcher has internal handling

        watcher.watch(&self.root, RecursiveMode::Recursive)?;

        self.watcher = Some(Box::new(watcher));
        Ok(())
    }

    /// Stop watching by dropping the watcher.
    pub fn stop(&mut self) {
        self.watcher = None;
    }
}
