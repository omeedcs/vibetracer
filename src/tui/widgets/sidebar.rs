use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::Widget,
};

use crate::tui::SidebarPanel;

/// Sidebar container widget — dispatches to the active panel.
///
/// Because each panel has its own data requirements, the Sidebar widget
/// itself renders a placeholder frame; callers are expected to render the
/// appropriate panel widget directly into the sidebar area.  This struct
/// serves as the dispatching coordinator when integrated into the main
/// render loop.
pub struct Sidebar<'a> {
    pub panel: &'a SidebarPanel,
}

impl<'a> Sidebar<'a> {
    pub fn new(panel: &'a SidebarPanel) -> Self {
        Self { panel }
    }
}

impl<'a> Widget for Sidebar<'a> {
    fn render(self, _area: Rect, _buf: &mut Buffer) {
        // The Sidebar widget acts as a dispatch coordinator.
        // Panel-specific rendering is handled by the individual panel widgets
        // (BlastRadiusPanel, SentinelPanel, WatchdogPanel, RefactorPanel,
        // EquationPanel) which are rendered directly by the main render loop.
        //
        // This impl is intentionally a no-op; it exists so the type can be
        // constructed and matched in the render loop to drive dispatch.
    }
}

/// Determine which sidebar panel to activate based on the current `SidebarPanel` value.
/// Returns a human-readable label for the active panel (useful for debug / status).
pub fn panel_label(panel: &SidebarPanel) -> &'static str {
    match panel {
        SidebarPanel::BlastRadius => "BLAST RADIUS",
        SidebarPanel::Sentinels => "SENTINEL",
        SidebarPanel::Watchdog => "CONSTANT MODIFIED",
        SidebarPanel::Refactor => "REFACTOR",
        SidebarPanel::Equations => "EQUATIONS",
    }
}
