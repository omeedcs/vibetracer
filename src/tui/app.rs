use std::collections::{HashMap, HashSet};

use chrono::Utc;

use crate::analysis::blast_radius::DependencyStatus;
use crate::analysis::sentinels::SentinelViolation;
use crate::analysis::watchdog::WatchdogAlert;
use crate::event::EditEvent;
use crate::snapshot::store::SnapshotStore;
use crate::theme::Theme;
use crate::tui::layout::AppLayout;
use crate::claude_log::{ConversationTurn, TokenStats};
use crate::tui::alerts::AlertEvaluator;
use crate::tui::bookmarks::BookmarkManager;
use crate::tui::filter::Filter;
use crate::tui::session_diff::SessionDiff;
use crate::tui::widgets::command_palette::CommandPalette;
use crate::tui::widgets::conversation::ConversationState;
use crate::tui::widgets::dashboard::DashboardState;

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

/// Whether the preview pane shows a full file or a diff.
#[derive(Debug, Clone, PartialEq)]
pub enum PreviewMode {
    File,
    Diff,
}

/// Style for toast notifications displayed in the status bar.
#[derive(Debug, Clone, PartialEq)]
pub enum ToastStyle {
    Info,
    Success,
    Warning,
}

/// Vim-style modal system for the TUI.
#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    /// Default mode. Scrubbing, navigation, toggling panels.
    Normal,
    /// Focused timeline manipulation — zoom, pan, solo/mute, jump-to-file.
    Timeline,
    /// Deep-dive on selected edit/operation — metadata, diff, reasoning.
    Inspect,
    /// Filter edits by file, agent, time range, content, intent.
    Search,
}

impl Mode {
    /// Short label for status bar display.
    pub fn label(&self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Timeline => "TIMELINE",
            Mode::Inspect => "INSPECT",
            Mode::Search => "SEARCH",
        }
    }
}

/// Top-level application state for the TUI.
pub struct App {
    pub edits: Vec<EditEvent>,
    pub playhead: usize,
    pub playback: PlaybackState,

    pub focused_pane: Pane,
    pub sidebar_visible: bool,
    pub sidebar_panel: SidebarPanel,

    // ── cockpit mode system ──────────────────────────────────────────────────
    /// Current vim-style mode.
    pub mode: Mode,
    /// Whether the dashboard panel (right side) is visible.
    pub dashboard_visible: bool,
    /// Search/filter input string (active in Search mode).
    pub search_input: String,
    /// Selected item index in the inspect/timeline modes.
    pub mode_cursor: usize,
    /// Command palette state.
    pub command_palette: CommandPalette,
    /// Dashboard panel state (sparklines, metrics).
    pub dashboard_state: DashboardState,
    /// Active search filter (applied when search is locked).
    pub active_filter: Option<Filter>,
    /// Cached filter match results (one bool per edit).
    pub filter_matches: Vec<bool>,

    // ── conversation / Claude integration ────────────────────────────────────
    /// Parsed conversation turns from Claude Code logs.
    pub conversation_turns: Vec<ConversationTurn>,
    /// Conversation panel UI state.
    pub conversation_state: ConversationState,
    /// Aggregated token stats from Claude logs.
    pub token_stats: TokenStats,
    /// Whether conversation panel is visible (replaces dashboard).
    pub conversation_visible: bool,

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
    /// Whether to show full file content or diff in the preview pane.
    pub preview_mode: PreviewMode,
    /// Current scroll offset in the preview pane (rendered).
    pub preview_scroll: usize,
    /// Target scroll offset for smooth scrolling.
    pub preview_scroll_target: usize,
    /// Cached file content: (after_hash, decoded content).
    pub cached_content: Option<(String, String)>,
    /// Zoom factor for the timeline (1.0 = default).
    pub timeline_zoom: f64,
    /// Horizontal scroll offset in the timeline.
    pub timeline_scroll: usize,
    /// Toast notification message (displayed in status bar, auto-dismissed).
    pub toast_message: Option<String>,
    /// Toast notification style (determines color).
    pub toast_style: ToastStyle,
    /// When the toast was triggered (for auto-dismiss timing).
    pub toast_time: Option<std::time::Instant>,
    /// Agent solo filter: only show edits from this agent_id.
    pub solo_agent: Option<String>,
    /// Last computed layout (for mouse-aware scroll routing).
    pub last_layout: Option<AppLayout>,
    /// Flash timer for active track highlight on scrub.
    pub track_flash: Option<(String, std::time::Instant)>,
    /// Flash timer for playback state change.
    pub playback_flash: Option<std::time::Instant>,

    // ── blame & annotations ─────────────────────────────────────────────────
    /// Whether blame view is active (per-line attribution).
    pub blame_visible: bool,
    /// Whether inline annotations are active.
    pub annotations_visible: bool,

    // ── bookmarks ────────────────────────────────────────────────────────────
    /// Bookmark manager for timeline positions.
    pub bookmark_manager: BookmarkManager,
    /// Whether the bookmark list popup is visible.
    pub bookmark_popup_visible: bool,
    /// Currently selected index in the bookmark list popup.
    pub bookmark_popup_selected: usize,

    // ── configurable alerts ─────────────────────────────────────────────────
    /// Evaluator for configurable notification triggers.
    pub alert_evaluator: AlertEvaluator,
    /// Instant when a Flash alert action fired (renders a brief tinted overlay).
    pub screen_flash: Option<std::time::Instant>,

    // ── session diff ────────────────────────────────────────────────────────
    /// Currently visible session diff (None when overlay is hidden).
    pub session_diff: Option<SessionDiff>,
    /// Selected file index within the session diff overlay.
    pub session_diff_selected: usize,
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

            mode: Mode::Normal,
            dashboard_visible: true,
            search_input: String::new(),
            mode_cursor: 0,
            command_palette: CommandPalette::new(),
            dashboard_state: DashboardState::new(),
            active_filter: None,
            filter_matches: Vec::new(),

            conversation_turns: Vec::new(),
            conversation_state: ConversationState::new(),
            token_stats: TokenStats::default(),
            conversation_visible: false,

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
            preview_mode: PreviewMode::File,
            preview_scroll: 0,
            preview_scroll_target: 0,
            cached_content: None,
            timeline_zoom: 1.0,
            timeline_scroll: 0,
            toast_message: None,
            toast_style: ToastStyle::Info,
            toast_time: None,
            solo_agent: None,
            last_layout: None,
            track_flash: None,
            playback_flash: None,

            blame_visible: false,
            annotations_visible: false,

            bookmark_manager: BookmarkManager::new(),
            bookmark_popup_visible: false,
            bookmark_popup_selected: 0,

            alert_evaluator: AlertEvaluator::empty(),
            screen_flash: None,

            session_diff: None,
            session_diff_selected: 0,
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

    /// Retrieve the file content for the current edit at the playhead.
    ///
    /// Returns `(content, filename)`. Uses `cached_content` when the hash
    /// hasn't changed to avoid redundant disk reads.
    pub fn current_file_content(
        &mut self,
        session_dir: &std::path::Path,
    ) -> Option<(String, String)> {
        let edit = self.edits.get(self.playhead)?;
        let hash = edit.after_hash.clone();
        let filename = edit.file.clone();

        // Return cached value if the hash matches.
        if let Some((ref cached_hash, ref cached_content)) = self.cached_content {
            if *cached_hash == hash {
                return Some((cached_content.clone(), filename));
            }
        }

        // Retrieve from the snapshot store.
        let store = SnapshotStore::new(session_dir.join("snapshots"));
        let bytes = store.retrieve(&hash).ok()?;
        let content = String::from_utf8_lossy(&bytes).into_owned();

        self.cached_content = Some((hash, content.clone()));
        Some((content, filename))
    }

    /// Parse the current edit's patch to find which lines were added.
    ///
    /// Returns a set of 1-based line numbers in the new file that correspond
    /// to `+` lines in the unified diff.
    pub fn changed_lines_from_patch(&self) -> HashSet<usize> {
        let mut result = HashSet::new();
        let edit = match self.current_edit() {
            Some(e) => e,
            None => return result,
        };

        let mut new_line: usize = 0;

        for line in edit.patch.lines() {
            if line.starts_with("@@") {
                // Parse the `+start,count` portion of `@@ -old,count +new,count @@`
                if let Some(plus_pos) = line.find('+') {
                    let after_plus = &line[plus_pos + 1..];
                    let num_str: String = after_plus
                        .chars()
                        .take_while(|c| c.is_ascii_digit())
                        .collect();
                    if let Ok(start) = num_str.parse::<usize>() {
                        new_line = start;
                    }
                }
            } else if line.starts_with('+') {
                result.insert(new_line);
                new_line += 1;
            } else if line.starts_with('-') {
                // Removed line: don't advance new-file counter.
            } else {
                // Context line: advance new-file counter.
                new_line += 1;
            }
        }

        result
    }

    /// Update dashboard state from current app data.
    /// Called each frame to keep the dashboard in sync.
    pub fn update_dashboard(&mut self) {
        // File heatmap
        let mut file_counts: HashMap<String, u32> = HashMap::new();
        for edit in &self.edits {
            *file_counts.entry(edit.file.clone()).or_insert(0) += 1;
        }
        let mut file_heat: Vec<(String, u32)> = file_counts.into_iter().collect();
        file_heat.sort_by(|a, b| b.1.cmp(&a.1));
        self.dashboard_state.file_heat = file_heat;

        // Agent status
        let mut agent_counts: HashMap<String, (u32, i64)> = HashMap::new(); // (count, last_ts)
        for edit in &self.edits {
            if let Some(ref label) = edit.agent_label {
                let entry = agent_counts.entry(label.clone()).or_insert((0, 0));
                entry.0 += 1;
                entry.1 = entry.1.max(edit.ts);
            }
        }
        let now_ms = chrono::Utc::now().timestamp_millis();
        let mut agents: Vec<(String, u32, bool)> = agent_counts
            .into_iter()
            .map(|(label, (count, last_ts))| {
                let active = (now_ms - last_ts) < 10_000; // active if seen in last 10s
                (label, count, active)
            })
            .collect();
        agents.sort_by(|a, b| b.1.cmp(&a.1));
        self.dashboard_state.agent_status = agents;

        // Operations
        let mut op_counts: HashMap<String, (u32, bool)> = HashMap::new();
        for edit in &self.edits {
            if let Some(ref intent) = edit.operation_intent {
                let entry = op_counts.entry(intent.clone()).or_insert((0, false));
                entry.0 += 1;
            }
        }
        let ops: Vec<(String, u32, bool)> = op_counts
            .into_iter()
            .map(|(name, (count, active))| (name, count, active))
            .collect();
        self.dashboard_state.operations = ops;

        // Edit velocity (edits in last 60s)
        let cutoff = now_ms - 60_000;
        let recent_count = self.edits.iter().filter(|e| e.ts > cutoff).count();
        self.dashboard_state.edit_velocity = recent_count as f64;

        // Token/cost from Claude conversation logs
        self.dashboard_state.tokens_in = self.token_stats.total_in;
        self.dashboard_state.tokens_out = self.token_stats.total_out;
        self.dashboard_state.total_cost = self.token_stats.total_cost;
        if self.token_stats.total_in + self.token_stats.total_out > 0 {
            self.dashboard_state.cache_hit_pct = (self.token_stats.total_cache_read as f64
                / (self.token_stats.total_in + self.token_stats.total_out) as f64)
                * 100.0;
        }

        // Analysis summaries from app state
        self.dashboard_state.sentinel_fail = self.sentinel_violations.len() as u32;
        self.dashboard_state.sentinel_failures = self
            .sentinel_violations
            .iter()
            .map(|v| format!("{}: {}", v.rule_name, v.description))
            .collect();

        self.dashboard_state.watchdog_ok = self.watchdog_alerts.is_empty();
        self.dashboard_state.watchdog_alerts = self
            .watchdog_alerts
            .iter()
            .map(|a| format!("{} in {}", a.constant_pattern, a.file))
            .collect();

        if let Some((_, ref status)) = self.blast_radius_status {
            self.dashboard_state.stale_count = status.stale.len() as u32;
            self.dashboard_state.updated_count = status.updated.len() as u32;
            self.dashboard_state.untouched_count = status.untouched.len() as u32;
            self.dashboard_state.stale_files = status.stale.clone();
        }
    }

    /// Display a toast notification in the status bar for 2 seconds.
    pub fn show_toast(&mut self, message: String, style: ToastStyle) {
        self.toast_message = Some(message);
        self.toast_style = style;
        self.toast_time = Some(std::time::Instant::now());
    }

    /// Check if the toast is still active (within 2 seconds).
    pub fn toast_active(&self) -> bool {
        self.toast_time
            .map(|t| t.elapsed().as_secs() < 2)
            .unwrap_or(false)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
