//! UI components

mod auth_editor;
mod code_gen_modal;
mod history_panel;
mod modals;
mod request_editor;
mod response_viewer;
mod sidebar;
mod style;
mod tabs;
mod top_bar;

pub use code_gen_modal::*;
pub use history_panel::*;
pub use modals::*;
pub use request_editor::*;
pub use response_viewer::*;
pub use sidebar::*;
pub use style::*;
pub use tabs::*;
pub use top_bar::*;

#[cfg(test)]
#[path = "../../tests/unit/ui.rs"]
mod tests;
