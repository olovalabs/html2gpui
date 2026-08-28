//! View layer, split by screen region. Widgets here are "dumb": they take
//! colors + data and emit callbacks/actions; all state lives in `workspace`.

pub mod activity_bar;
pub mod app_icon;
pub mod common;
pub mod diff;
pub mod settings;
pub mod sidebar;
pub mod status_bar;
pub mod tab_bar;
pub mod titlebar;
pub mod welcome;
