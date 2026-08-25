//! View layer, split by screen region. Widgets here are "dumb": they take
//! colors + data and emit callbacks/actions; all state lives in `workspace`.

pub mod activity_bar;
pub mod common;
pub mod sidebar;
pub mod status_bar;
pub mod terminal;
pub mod titlebar;
pub mod welcome;
