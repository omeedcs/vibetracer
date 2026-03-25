use std::collections::{HashMap, HashSet};

use chrono::Utc;

use crate::analysis::blast_radius::DependencyStatus;
use crate::analysis::sentinels::SentinelViolation;
use crate::analysis::watchdog::WatchdogAlert;
use crate::event::EditEvent;
use crate::theme::Theme;

/// Which primary pane currently has keyboard focus.
#[derive(Debug, Clone, PartialEq)]
pub enum Pane {
    Preview,
    Timeline,
    Sidebar,
}

/// Which panel is active inside the sidebar.
#[derive(Debug, Clone, PartialEq)]
pub enum SidebarPanel {
    BlastRadius,
    Sentinels,
    Watchdog,
}

/// Current playback mode.
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackState {
    /// Following live edits as they arrive.
    Live,
    /// Paused at a fixed position in the timeline.
    Paused,
    /// Playing back at the given speed multiplier.
    Playing { speed: u8 },
}

/// Per-file track metadata shown in the timeline.
#[derive(Debug, Clone)]
pub struct TrackInfo {
    pub filename: String,
    pub edit_indices: Vec<usize>,
    pub stale: bool,
}

/// Top-level application state for the TUI.
pub struct App {
    pub edits: Vec<EditEvent>,
    pub playhead: usize,
    pub playback: PlaybackState,

    pub focused_pane: Pane,
    pub sidebar_visible: bool,
    pub sidebar_panel: SidebarPanel,

    pub solo_track: Option<String>,
    pub muted_tracks: Vec<String>,

    pub checkpoint_ids: Vec<u32>,
    pub session_start: i64,

    pub connected: bool,
    pub should_quit: bool,
    pub tracks: Vec<TrackInfo>,

    // Analysis state (populated by the event loop)
    pub watchdog_alerts: Vec<WatchdogAlert>,
    pub sentinel_violations: Vec<SentinelViolation>,
    pub blast_radius_status: Option<(String, DependencyStatus)>,

    // Color theme
    pub theme: Theme,

    // ── v2 fields ─────────────────────────────────────────────────────────────
    /// Per-file playhead positions (filename -> edit index within that file).
    pub file_playheads: HashMap<String, usize>,
    /// Files with independent (detached) playheads.
    pub detached_files: HashSet<String>,
    /// Toggle between edit view and command/operation view.
    pub command_view: bool,
    /// Toggle showing restore-generated edits in the timeline.
    pub show_restore_edits: bool,
    /// Current theme preset name.
    pub theme_name: String,
    /// When the theme was last changed (for 2s flash notification).
    pub theme_flash: Option<std::time::Instant>,
}

impl App {
    /// Create a new `App` with sensible defaults.
    pub fn new() -> Self {
        App {
            edits: Vec::new(),
            playhead: 0,
            playback: PlaybackState::Live,

            focused_pane: Pane::Timeline,
            sidebar_visible: false,
            sidebar_panel: SidebarPanel::BlastRadius,

            solo_track: None,
            muted_tracks: Vec::new(),

            checkpoint_ids: Vec::new(),
            session_start: Utc::now().timestamp(),

            connected: false,
            should_quit: false,
            tracks: Vec::new(),

            watchdog_alerts: Vec::new(),
            sentinel_violations: Vec::new(),
            blast_radius_status: None,

            theme: Theme::dark(),

            // v2 fields
            file_playheads: HashMap::new(),
            detached_files: HashSet::new(),
            command_view: false,
            show_restore_edits: false,
            theme_name: "dark".to_string(),
            theme_flash: None,
        }
    }

    /// Push a new edit into the log, update or create its track entry,
    /// and advance the playhead if in Live mode.
    pub fn push_edit(&mut self, edit: EditEvent) {
        let idx = self.edits.len();
        let file = edit.file.clone();
        self.edits.push(edit);

        // Update the track for this file.
        if let Some(track) = self.tracks.iter_mut().find(|t| t.filename == file) {
            track.edit_indices.push(idx);
            track.stale = false;
        } else {
            self.tracks.push(TrackInfo {
                filename: file,
                edit_indices: vec![idx],
                stale: false,
            });
        }

        // In Live mode, keep the playhead at the latest edit.
        if self.playback == PlaybackState::Live {
            self.playhead = self.edits.len().saturating_sub(1);
        }
    }

    /// Return a reference to the edit currently at the playhead position, if any.
    pub fn current_edit(&self) -> Option<&EditEvent> {
        if self.edits.is_empty() {
            None
        } else {
            self.edits.get(self.playhead)
        }
    }

    /// Move the playhead one step to the left (backward). Sets state to Paused.
    pub fn scrub_left(&mut self) {
        self.playback = PlaybackState::Paused;
        if self.playhead > 0 {
            self.playhead -= 1;
        }
    }

    /// Move the playhead one step to the right (forward). If we reach the end,
    /// return to Live mode.
    pub fn scrub_right(&mut self) {
        if self.edits.is_empty() {
            return;
        }
        let last = self.edits.len() - 1;
        if self.playhead < last {
            self.playhead += 1;
        }
        if self.playhead >= last {
            self.playback = PlaybackState::Live;
        }
    }

    /// Cycle playback state: Live -> Paused, Paused -> Playing{1}, Playing -> Paused.
    pub fn toggle_play(&mut self) {
        self.playback = match &self.playback {
            PlaybackState::Live => PlaybackState::Paused,
            PlaybackState::Paused => PlaybackState::Playing { speed: 1 },
            PlaybackState::Playing { .. } => PlaybackState::Paused,
        };
    }

    /// Update playback speed; only has effect if currently Playing.
    pub fn set_speed(&mut self, speed: u8) {
        if let PlaybackState::Playing { .. } = self.playback {
            self.playback = PlaybackState::Playing { speed };
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
