//! The small kit of widgets every panel is built from: buttons, inputs, tabs, rows of a list,
//! cards, chips, notices, a spinner and the sheets that ask a question. Each one carries an
//! accessible label.

mod controls;
mod frames;
pub mod gallery;
mod look;
mod marks;
mod messages;
mod progress;

pub use controls::{
    Button, Dropdown, ListRow, Slider, Step, Stepper, Tab, TabStrip, TextInput, TextInputResponse,
    paint_caret,
};
pub use frames::{Card, CardResponse, PanelFrame, Surface, panel_frame, section_header, separator};
pub use look::ControlSize;
pub use marks::{Badge, Chip, CitationChip, StepMarker, StepState, dot, logo};
pub use messages::{
    Choice, Confirm, Notice, NoticeResponse, Placeholder, PlaceholderResponse, modal,
};
pub use progress::{indeterminate_bar, progress_bar, spinner};

pub(crate) fn interactive_cursor(ui: &eframe::egui::Ui, response: &eframe::egui::Response) {
    if response.enabled()
        && response.sense.senses_click()
        && response.hovered()
        && let Some(cursor) = ui.visuals().interact_cursor
    {
        ui.ctx().set_cursor_icon(cursor);
    }
}
