//! The small kit of widgets every panel is built from: buttons, inputs, tabs, a bar of views,
//! rows of a list, cards, chips, notices, a spinner and the sheets that ask a question. Each one
//! carries an accessible label.

mod controls;
mod frames;
pub mod gallery;
mod look;
mod marks;
mod messages;
mod progress;

pub use controls::{
    Button, CopyButton, Dropdown, ListRow, NavBar, NavItem, Slider, Step, Stepper, Tab, TabStrip,
    TextInput, TextInputResponse, paint_caret, style_list_rows,
};
pub use frames::{
    Card, CardResponse, Copyable, CopyableResponse, PanelFrame, Surface, panel_frame,
    section_header, separator,
};
pub use look::ControlSize;
pub(crate) use look::focus_ring;
pub use marks::{Badge, Chip, CitationChip, StepMarker, StepState, dot, logo};
pub use messages::{
    Choice, Confirm, Notice, NoticeResponse, Placeholder, PlaceholderResponse, modal,
};
pub(crate) use messages::{close_the_top_sheet, forget_the_close_request, is_sheet_or_popup_open};
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
