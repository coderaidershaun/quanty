//! The fields and the buttons of the sheet.

use eframe::egui;

use super::DatabasePicker;
use super::draft::Row;
use crate::contract::{Database, Intent};
use crate::state::{DatabaseLink, Shared};
use crate::theme::{TextRole, color, space};
use crate::widgets::{Button, ControlSize, Dropdown, Notice, TextInput};

const SHEET_WIDTH: f32 = 460.0;
const NEW_ENTRY: &str = "New…";

const OFF_WHILE_BUSY: &str =
    "Choosing a database is available when the current operation finishes.";
const FORGET_OFF_IN_USE: &str = "The database in use cannot be forgotten. Use another one first.";
const CANCEL_OFF_UNCHOSEN: &str = "Choose a database first. Nothing loads until you do.";
const OFF_WHILE_CHECKING: &str = "Wait for the check to end.";

/// The button of the sheet that was pressed in this frame.
enum Pressed {
    Forget(String),
    Cancel,
    Use,
}

impl DatabasePicker {
    pub(super) fn contents(
        &mut self,
        ui: &mut egui::Ui,
        shared: &Shared,
        can_switch: bool,
        intents: &mut Vec<Intent>,
    ) {
        let link = &shared.database;
        ui.set_width(SHEET_WIDTH);
        ui.label(TextRole::Heading.rich("Choose a database"));
        ui.label(
            TextRole::Body
                .rich("The two stores of a library and the folder of its pages.")
                .color(color::TEXT_SECONDARY),
        );
        ui.add_space(space::MD);
        ui.add_enabled_ui(!link.is_checking(), |ui| self.fields(ui));

        let problem = self.problem();
        if let Some(problem) = &problem {
            ui.label(TextRole::Small.rich(problem));
        }
        self.messages(ui, link);
        ui.add_space(space::MD);

        let use_off = if can_switch {
            problem
        } else {
            Some(OFF_WHILE_BUSY.to_owned())
        };
        match self.buttons(ui, link, use_off.as_deref()) {
            Some(Pressed::Forget(name)) => self.forget(&name),
            Some(Pressed::Cancel) => self.leave(),
            Some(Pressed::Use) => self.use_draft(link.in_use.as_ref(), intents),
            None => {}
        }
    }

    /// How the last switch ended, or why the database in use is down when the sheet shows it,
    /// and why the file could not be saved.
    fn messages(&self, ui: &mut egui::Ui, link: &DatabaseLink) {
        let shows_in_use = match &self.draft.row {
            Row::Local => link.in_use.as_ref().is_some_and(Database::is_local),
            Row::Saved(name) => link
                .in_use
                .as_ref()
                .is_some_and(|database| database.name == *name),
            Row::New => false,
        };
        let failure = if self.sent.is_some() {
            link.switch_failure.as_ref()
        } else if shows_in_use {
            link.state.failure()
        } else {
            None
        };
        if link.is_checking() {
            ui.label(TextRole::Small.rich("Checking…"));
        } else if let Some(failure) = failure {
            // The detail holds the cause that the store gave, such as a key it refused.
            Notice::error(&failure.hint).body(&failure.detail).show(ui);
        }
        if let Some(error) = &self.save_error {
            ui.label(TextRole::Small.rich(format!("⚠ {error}")));
        }
    }

    fn buttons(
        &self,
        ui: &mut egui::Ui,
        link: &DatabaseLink,
        use_off: Option<&str>,
    ) -> Option<Pressed> {
        let is_checking = link.is_checking();
        let in_use_name = link.in_use.as_ref().map(|database| database.name.as_str());
        let mut pressed = None;
        ui.horizontal(|ui| {
            if let Row::Saved(name) = &self.draft.row {
                let is_in_use = in_use_name == Some(name.as_str());
                let why_off = if is_checking {
                    OFF_WHILE_CHECKING
                } else {
                    FORGET_OFF_IN_USE
                };
                let button = ui.add_enabled(!is_checking && !is_in_use, Button::danger("Forget"));
                if button.on_disabled_hover_text(why_off).clicked() {
                    pressed = Some(Pressed::Forget(name.clone()));
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let button = ui.add_enabled(use_off.is_none(), Button::primary("Use"));
                if button
                    .on_disabled_hover_text(use_off.unwrap_or_default())
                    .clicked()
                {
                    pressed = Some(Pressed::Use);
                }
                let why_off = if is_checking {
                    OFF_WHILE_CHECKING
                } else {
                    CANCEL_OFF_UNCHOSEN
                };
                let can_leave = in_use_name.is_some() && !is_checking;
                let button = ui.add_enabled(can_leave, Button::secondary("Cancel"));
                if button.on_disabled_hover_text(why_off).clicked() {
                    pressed = Some(Pressed::Cancel);
                }
            });
        });
        pressed
    }

    fn dropdown(&mut self, ui: &mut egui::Ui) {
        let names: Vec<&str> = std::iter::once(self.local.name.as_str())
            .chain(self.databases.saved.iter().map(|saved| saved.name.as_str()))
            .chain([NEW_ENTRY])
            .collect();
        let chosen = Dropdown::new("Database", &names, SHEET_WIDTH)
            .selected(self.selected())
            .size(ControlSize::Medium)
            .show(ui);
        if let Some(index) = chosen {
            self.select(index);
        }
    }

    fn fields(&mut self, ui: &mut egui::Ui) {
        self.dropdown(ui);
        let is_editable = self.draft.row != Row::Local;
        if !is_editable {
            ui.label(TextRole::Small.rich("Edit .env to change Local."));
        }
        ui.add_space(space::SM);
        ui.add_enabled_ui(is_editable, |ui| {
            let draft = &mut self.draft;
            field(ui, "Name", &mut draft.name, "Office server");
            field(
                ui,
                "Qdrant URL",
                &mut draft.qdrant_url,
                "http://127.0.0.1:16334",
            );
            secret_field(
                ui,
                "Qdrant API key",
                &mut draft.qdrant_api_key,
                "Leave empty for none",
            );
            field(
                ui,
                "FalkorDB URL",
                &mut draft.falkordb_url,
                "falkor://127.0.0.1:16379",
            );
            secret_field(
                ui,
                "FalkorDB password",
                &mut draft.falkordb_password,
                "Leave empty for none",
            );
            field(ui, "Graph", &mut draft.falkordb_graph, "quanty");
            field(
                ui,
                "Content folder",
                &mut draft.content_folder,
                "/Volumes/quanty-content",
            );
        });
    }
}

fn field(ui: &mut egui::Ui, caption: &str, text: &mut String, example: &str) {
    labelled(
        ui,
        caption,
        TextInput::new(caption, text).placeholder(example),
    );
}

/// A field that shows dots for what is typed, for a key or a password.
fn secret_field(ui: &mut egui::Ui, caption: &str, text: &mut String, example: &str) {
    let input = TextInput::new(caption, text).placeholder(example).secret();
    labelled(ui, caption, input);
}

fn labelled(ui: &mut egui::Ui, caption: &str, input: TextInput<'_>) {
    ui.label(TextRole::Caption.rich(caption));
    input.show(ui);
    ui.add_space(space::SM);
}
