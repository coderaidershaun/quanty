//! The database in use at the right end of the footer, and the sheet that chooses another one.
//! A choice is written to the file in the home folder only after its stores have answered, so the
//! file never names a database the app could not reach.

mod draft;
mod form;

use std::path::PathBuf;

use choices::{DATABASES_FILE, DatabasesError};
use eframe::egui;

use crate::contract::{Database, Databases, Intent, Loadable};
use crate::state::Shared;
use crate::theme::{TextRole, Tone, space};
use crate::widgets::{Button, ControlSize, dot, modal};
use draft::{Draft, Row};

/// A longer name is cut in the footer, where the room is fixed. The sheet shows it whole.
const NAME_LIMIT: usize = 24;

pub(super) struct DatabasePicker {
    databases: Databases,
    local: Database,
    /// `None` when the app runs from fixtures, so a fixture never writes a real file.
    path: Option<PathBuf>,
    is_open: bool,
    draft: Draft,
    /// The database that Use asked for, until the file has been written or the sheet is left.
    sent: Option<Sent>,
    save_error: Option<String>,
}

struct Sent {
    database: Database,
    /// The saved entry the draft was opened from, which a new name replaces.
    replaces: Option<String>,
}

impl DatabasePicker {
    /// The sheet is open from the start when no database is in use.
    pub fn new(shared: &Shared) -> Self {
        let facts = &shared.health.facts;
        Self {
            databases: facts.databases.clone(),
            draft: Draft::of(&facts.local_database, Row::Local),
            local: facts.local_database.clone(),
            path: facts
                .fixture
                .is_none()
                .then(|| facts.home.join(DATABASES_FILE)),
            is_open: shared.database.in_use.is_none(),
            sent: None,
            save_error: None,
        }
    }

    /// The database in use and how its last check ended, in the room the layout gives it. A click
    /// opens the sheet. It draws static text with no spinner, so a window at rest asks for no
    /// repaint.
    pub fn footer_entry(&mut self, ui: &mut egui::Ui, rect: egui::Rect, shared: &Shared) {
        let link = &shared.database;
        let (label, tone) = match (&link.in_use, &link.state) {
            (None, _) => ("Database: not chosen".to_owned(), Tone::Warning),
            (Some(database), state) => {
                let (words, tone) = match state {
                    Loadable::Idle => ("not checked", Tone::Warning),
                    Loadable::Loading => ("checking…", Tone::Warning),
                    Loadable::Ready(()) => ("connected", Tone::Success),
                    Loadable::Failed(_) => ("down", Tone::Danger),
                };
                (
                    format!("Database: {} · {words}", shortened(&database.name)),
                    tone,
                )
            }
        };
        let hint = link
            .state
            .failure()
            .map_or("", |failure| failure.hint.trim_end_matches('.'));
        let builder = egui::UiBuilder::new()
            .id_salt("database")
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center));
        ui.scope_builder(builder, |ui| {
            ui.spacing_mut().item_spacing.x = space::SM;
            dot(ui, tone);
            if ui
                .add(Button::ghost(&label).size(ControlSize::Small))
                .clicked()
            {
                self.open(link.in_use.as_ref());
            }
            if let Some(error) = &self.save_error {
                ui.label("⚠").on_hover_text(error);
            }
            ui.scope(|ui| {
                ui.set_max_width(ui.available_width());
                ui.add(egui::Label::new(TextRole::Small.rich(hint)).truncate());
            });
        });
    }

    /// Call on every frame. `can_switch` is false while anything runs that a new database would
    /// pull the stores from under.
    pub fn sheet(
        &mut self,
        ctx: &egui::Context,
        shared: &Shared,
        can_switch: bool,
        intents: &mut Vec<Intent>,
    ) {
        // This runs with the sheet shut too: a Use on the database already in use shuts the sheet
        // at once, and its answer comes later.
        self.save_if_used(shared);
        if !self.is_open {
            return;
        }
        let sheet = modal(ctx, egui::Id::new("database-sheet"), |ui| {
            self.contents(ui, shared, can_switch, intents);
        });
        // Leaving the sheet by Escape or a click outside is refused while nothing is in use, so the
        // first start cannot skip the choice, and while a check is on its way, so that a switch
        // cannot land after the sheet is gone and be left out of the file.
        let must_stay = shared.database.in_use.is_none() || shared.database.is_checking();
        if sheet.should_close() && !must_stay {
            self.leave();
        }
    }

    fn open(&mut self, in_use: Option<&Database>) {
        self.draft = match in_use {
            Some(database) if !database.is_local() => {
                Draft::of(database, Row::Saved(database.name.clone()))
            }
            _ => Draft::of(&self.local, Row::Local),
        };
        self.sent = None;
        self.is_open = true;
    }

    fn leave(&mut self) {
        self.sent = None;
        self.is_open = false;
    }

    /// Writes the choice once the database that Use asked for is the one in use and no check is
    /// left. A name that was changed replaces the entry it came from.
    fn save_if_used(&mut self, shared: &Shared) {
        let is_used = self.sent.as_ref().is_some_and(|sent| {
            shared.database.in_use.as_ref() == Some(&sent.database)
                && !shared.database.is_checking()
        });
        let Some(sent) = self.sent.take_if(|_| is_used) else {
            return;
        };
        let before = self.databases.clone();
        if let Some(old) = sent
            .replaces
            .as_deref()
            .filter(|old| *old != sent.database.name)
        {
            self.databases.forget(old);
        }
        self.databases.chosen = Some(sent.database.name.clone());
        if !sent.database.is_local() {
            self.databases.put(sent.database);
        }
        self.persist(before);
        self.is_open = false;
    }

    /// Writes the file. When that fails the list goes back to what it was, as the model picker
    /// does, and the reason stays for the footer and the sheet.
    fn persist(&mut self, before: Databases) {
        let Some(path) = &self.path else {
            return;
        };
        match self.databases.save_to(path) {
            Ok(()) => self.save_error = None,
            Err(error) => {
                self.databases = before;
                self.save_error = Some(error_message(&error));
            }
        }
    }

    fn forget(&mut self, name: &str) {
        let before = self.databases.clone();
        self.databases.forget(name);
        self.persist(before);
        if self.save_error.is_none() {
            self.draft = Draft::of(&self.local, Row::Local);
        }
    }

    fn select(&mut self, index: usize) {
        self.draft = match index {
            0 => Draft::of(&self.local, Row::Local),
            _ => match self.databases.saved.get(index - 1) {
                Some(saved) => Draft::of(saved, Row::Saved(saved.name.clone())),
                None => Draft::blank(&self.local.falkordb_graph),
            },
        };
    }

    fn selected(&self) -> Option<usize> {
        match &self.draft.row {
            Row::Local => Some(0),
            Row::Saved(name) => self
                .databases
                .saved
                .iter()
                .position(|saved| saved.name == *name)
                .map(|place| place + 1),
            Row::New => Some(self.databases.saved.len() + 1),
        }
    }

    /// The database that Use would ask for.
    fn candidate(&self) -> Database {
        match self.draft.row {
            Row::Local => self.local.clone(),
            Row::Saved(_) | Row::New => self.draft.database(),
        }
    }

    /// Why the draft cannot be used, so that nothing the file would refuse becomes the database
    /// in use. Local is never checked.
    fn problem(&self) -> Option<String> {
        if self.draft.row == Row::Local {
            return None;
        }
        let database = self.draft.database();
        if let Err(DatabasesError::Invalid { what, .. }) = database.validate() {
            return Some(format!("Not ready: {what}."));
        }
        let is_taken = self
            .databases
            .saved
            .iter()
            .any(|saved| saved.name == database.name)
            && self.draft.row != Row::Saved(database.name.clone());
        is_taken.then(|| {
            format!(
                "A database named {} is saved already. Choose another name.",
                database.name
            )
        })
    }

    /// Asks for the draft. Using the database already in use is a new check of it, so the sheet
    /// shuts and the footer shows the result.
    fn use_draft(&mut self, in_use: Option<&Database>, intents: &mut Vec<Intent>) {
        let database = self.candidate();
        let replaces = match &self.draft.row {
            Row::Saved(name) => Some(name.clone()),
            Row::Local | Row::New => None,
        };
        let is_recheck = in_use == Some(&database);
        self.sent = Some(Sent {
            database: database.clone(),
            replaces,
        });
        intents.push(Intent::UseDatabase(database));
        if is_recheck {
            self.is_open = false;
        }
    }
}

fn shortened(name: &str) -> String {
    if name.chars().count() <= NAME_LIMIT {
        return name.to_owned();
    }
    let kept: String = name.chars().take(NAME_LIMIT - 1).collect();
    format!("{kept}…")
}

// SMELL: the model picker has the same function. Both need one shared home.
fn error_message(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut cause = error.source();
    while let Some(error) = cause {
        message.push_str(": ");
        message.push_str(&error.to_string());
        cause = error.source();
    }
    message
}
