//! One screen that shows the colours, the text styles and every widget in every state, so a
//! reviewer sees the whole kit at once.

mod states;
mod tokens;

use eframe::egui;

use crate::theme::{TextRole, space};

/// The width of the left column, which holds the colours, the text styles and the icons.
const TOKENS_WIDTH: f32 = 680.0;

// SMELL: the path already says `gallery`, so `gallery::GalleryState` says it twice. `State` is
// enough.
/// What the gallery keeps between frames. Whoever shows the gallery owns it.
#[derive(Debug, Default)]
pub struct GalleryState {
    /// The name of the widget that was last used.
    pub last_activated: Option<&'static str>,
    demo: Demo,
}

/// The values the demo widgets hold, so a text box keeps what is typed and a tab stays chosen.
#[derive(Debug)]
struct Demo {
    question: String,
    follow_up: String,
    mode: Option<usize>,
    zoom: f32,
    tab: usize,
    top_tab: usize,
    compact_tab: usize,
    is_confirm_open: bool,
}

impl Default for Demo {
    fn default() -> Self {
        Demo {
            question: String::new(),
            follow_up: String::new(),
            mode: None,
            zoom: 1.0,
            tab: 0,
            top_tab: 0,
            compact_tab: 0,
            is_confirm_open: false,
        }
    }
}

/// Draws the gallery: the colours, the text styles, the icons, and every widget in every state.
pub fn show(ui: &mut egui::Ui, state: &mut GalleryState) {
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            ui.label(TextRole::Title.rich("Gallery"));
            ui.label(TextRole::Body.rich(format!(
                "Last used: {}",
                state.last_activated.unwrap_or("nothing yet")
            )));
            ui.horizontal_top(|ui| {
                let height = ui.available_height();
                column(ui, TOKENS_WIDTH, height, tokens::show);
                let width = ui.available_width();
                column(ui, width, height, |ui| states::show(ui, state));
            });
        });
}

/// A column of the given size, laid out top down whatever the layout around it is.
fn column(ui: &mut egui::Ui, width: f32, height: f32, add_contents: impl FnOnce(&mut egui::Ui)) {
    let layout = egui::Layout::top_down(egui::Align::Min);
    ui.allocate_ui_with_layout(egui::vec2(width, height), layout, add_contents);
}

/// A title above a group of demos.
fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(space::LG);
    ui.label(TextRole::Heading.rich(title));
}

/// Lays `items` in rows of cells that are `cell` wide, as many to a row as fit. A wrapped row
/// of nested layouts does not wrap by itself, so the rows are cut here.
fn rows<T>(ui: &mut egui::Ui, cell: f32, items: &[T], mut add_cell: impl FnMut(&mut egui::Ui, &T)) {
    let fit = (ui.available_width() / (cell + ui.spacing().item_spacing.x)).floor();
    let per_row = (fit as usize).max(1);
    for row in items.chunks(per_row) {
        ui.horizontal(|ui| {
            for item in row {
                add_cell(ui, item);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use eframe::egui::accesskit::Role;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    use super::*;
    use crate::state::Shared;
    use crate::testkit::{self, Host};
    use crate::theme::size;

    /// A window tall enough to show the whole gallery with no scrolling.
    const WINDOW: [f32; 2] = [1400.0, 2450.0];

    fn gallery() -> (Harness<'static, Host>, Rc<RefCell<GalleryState>>) {
        let state = Rc::new(RefCell::new(GalleryState::default()));
        let drawn = Rc::clone(&state);
        let mut harness = testkit::panel(WINDOW, Shared::default(), move |ui, _cx| {
            show(ui, &mut drawn.borrow_mut());
        });
        // The spinners ask for a repaint after a delay, so `run` settles with them on screen.
        harness.run();
        (harness, state)
    }

    #[test]
    fn every_interactive_widget_answers_to_its_role_and_label() {
        let (mut harness, state) = gallery();
        // What a click on each widget must write to `last_activated`.
        let rows = [
            (Role::Button, "Ask", "Button::primary"),
            (Role::Button, "Secondary", "Button::secondary"),
            (Role::Button, "Ghost", "Button::ghost"),
            (Role::Button, "Delete", "Button::danger"),
            (Role::Button, "Copy", "Button::icon_only"),
            (Role::Button, "Clear question", "TextInput::trailing"),
            (Role::Button, "Send", "TextInput::send"),
            (Role::Button, "Next page", "Stepper"),
            (Role::Tab, "Results", "TabStrip"),
            (Role::Tab, "Library", "TabStrip::magenta"),
            (Role::Tab, "Pictures", "TabStrip::compact"),
            (Role::Button, "Citation 1", "CitationChip"),
            (Role::Button, "Black-Scholes model", "Chip::kind"),
            (
                Role::Button,
                "Show the step-by-step derivation",
                "Chip::plain",
            ),
            (Role::Button, "Copy formula", "Card::action"),
            (Role::Button, "Open result", "Card::clickable"),
            (Role::Button, "Retry", "Notice::action"),
            (Role::Button, "Dismiss", "Notice::dismiss"),
            (Role::Button, "Ask a question", "Placeholder::action"),
        ];
        for (role, label, expected) in rows {
            state.borrow_mut().last_activated = None;
            harness.get_by_role_and_label(role, label).click();
            harness.run();
            assert_eq!(state.borrow().last_activated, Some(expected), "{label}");
        }

        state.borrow_mut().last_activated = None;
        harness
            .get_by_role_and_label(Role::ComboBox, "Mode")
            .click();
        harness.run();
        harness
            .get_by_role_and_label(Role::Button, "Search only")
            .click();
        harness.run();
        assert_eq!(state.borrow().last_activated, Some("Dropdown"));
        assert_eq!(state.borrow().demo.mode, Some(1));

        // The names that the tests of the panels look for.
        harness.get_by_label(
            "No sources found. I can't answer this question from the ingested documents. \
             Try broadening the question or adding more relevant chapters.",
        );
        harness.get_by_label(
            "Qdrant is not running. Start it with docker compose up, then try again.",
        );
        harness.get_by_label("Loading the page. This takes a moment.");
        harness.get_by_role_and_label(Role::Button, states::CUT_CHIP);
        harness.get_by_role_and_label(Role::ProgressIndicator, "Reading the page");
        harness.get_by_role_and_label(Role::Button, "Previous page");
        harness.get_by_role_and_label(Role::Slider, "Zoom");
        harness.get_by_label("Step 3");
        harness.get_by_label("Ingested");
        harness.get_by_role_and_label(Role::Label, "Figure (from source)");
        // The title and the body of a notice and of a placeholder are also found on their own.
        for text in [
            "Qdrant is not running",
            "Start it with docker compose up, then try again.",
            "Nothing here yet",
            "Ask a question to see the results.",
        ] {
            harness.get_by_role_and_label(Role::Label, text);
        }
        let step = harness.get_by_role_and_label(Role::Label, states::LONG_STEP);
        assert!(
            step.rect().height() <= size::CONTROL_SM,
            "the text of a stepper stays on one line: {} high",
            step.rect().height()
        );
        // The actions of a panel header run from right to left, and a stepper among them must
        // still read previous, text, next.
        let [previous, text, next] = [
            (Role::Button, "Previous figure"),
            (Role::Label, states::HEADER_STEP),
            (Role::Button, "Next figure"),
        ]
        .map(|(role, label)| harness.get_by_role_and_label(role, label).rect());
        assert!(
            previous.center().x < text.center().x && text.center().x < next.center().x,
            "a stepper in a right-to-left row reads previous, text, next: \
             {previous:?}, {text:?}, {next:?}"
        );
        let results = harness.get_by_role_and_label(Role::Tab, "Results");
        assert_eq!(
            results.value().as_deref(),
            Some("28"),
            "a tab's count is its value"
        );
        let book = harness.get_by_role_and_label(Role::ComboBox, "Book").rect();
        assert!(
            (book.width() - states::BOOK_WIDTH).abs() < 0.5,
            "a dropdown with a width stays that wide: {}",
            book.width()
        );

        testkit::save_png(&mut harness, "gallery");
    }

    #[test]
    fn a_disabled_or_loading_button_ignores_activation() {
        let (mut harness, state) = gallery();
        for label in ["Ask loading", "Ask disabled"] {
            harness.get_by_role_and_label(Role::Button, label).click();
            harness.run();
            assert_eq!(state.borrow().last_activated, None, "{label} was activated");
        }
        harness.get_by_role_and_label(Role::Button, "Ask").click();
        harness.run();
        assert_eq!(state.borrow().last_activated, Some("Button::primary"));
    }
}
