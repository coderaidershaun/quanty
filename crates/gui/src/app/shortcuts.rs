//! Turns the keys a person presses into intents.

use eframe::egui;

use super::top_bar::is_bound;
use crate::contract::{Chord, Intent, KeyName, SHORTCUTS, When};

/// Reads this frame's keys and pushes the intents they stand for. A row that matches takes its
/// key away, so a widget never sees it too. Nothing is read while a sheet is open.
pub(super) fn read(ctx: &egui::Context, intents: &mut Vec<Intent>) {
    if ctx.memory(|memory| memory.top_modal_layer().is_some()) {
        return;
    }
    // An open list has the keyboard too, as a text box has. `egui_wants_keyboard_input` is not
    // the test: it is true for any focused widget, so a tab reached with the Tab key would
    // block every plain key.
    let has_keyboard = text_box_has_keyboard(ctx) || egui::Popup::is_any_open(ctx);
    ctx.input_mut(|input| {
        for row in SHORTCUTS.iter().filter(|row| is_bound(row)) {
            if row.when == When::NotTyping && has_keyboard {
                continue;
            }
            let (modifiers, key) = pattern_of(row.chord);
            if chord_pressed(input, modifiers, key, row.repeats) {
                intents.push(row.intent.clone());
            }
        }
    });
}

/// True while a text box has the keyboard, and for the one key press that takes it away.
/// Escape makes egui drop the focus before the app looks at the keys, and that press must only
/// leave the box: it must not also stop an answer that is being written.
fn text_box_has_keyboard(ctx: &egui::Context) -> bool {
    let remembered = egui::Id::new("quanty: the text box that had the keyboard");
    let now = ctx
        .text_edit_focused()
        .then(|| ctx.memory(|memory| memory.focused()))
        .flatten();
    let before = ctx.data(|data| data.get_temp::<egui::Id>(remembered));
    if let Some(id) = now {
        ctx.data_mut(|data| data.insert_temp(remembered, id));
    }
    now.is_some() || before.is_some_and(|id| ctx.memory(|memory| memory.had_focus_last_frame(id)))
}

fn pattern_of(chord: Chord) -> (egui::Modifiers, egui::Key) {
    let mut modifiers = egui::Modifiers::NONE;
    if chord.command {
        modifiers |= egui::Modifiers::COMMAND;
    }
    if chord.shift {
        modifiers |= egui::Modifiers::SHIFT;
    }
    (modifiers, key_of(chord.key))
}

fn key_of(key: KeyName) -> egui::Key {
    match key {
        KeyName::Num1 => egui::Key::Num1,
        KeyName::Num2 => egui::Key::Num2,
        KeyName::Num3 => egui::Key::Num3,
        KeyName::J => egui::Key::J,
        KeyName::K => egui::Key::K,
        KeyName::R => egui::Key::R,
        KeyName::S => egui::Key::S,
        KeyName::Slash => egui::Key::Slash,
        KeyName::Period => egui::Key::Period,
        KeyName::Escape => egui::Key::Escape,
        KeyName::OpenBracket => egui::Key::OpenBracket,
        KeyName::CloseBracket => egui::Key::CloseBracket,
    }
}

/// True when the chord was pressed this frame, and takes its key events away. A held key counts
/// again only when `repeats` is true. `InputState::consume_key` is not used because it counts
/// every repeat of a held key.
fn chord_pressed(
    input: &mut egui::InputState,
    pattern: egui::Modifiers,
    key: egui::Key,
    repeats: bool,
) -> bool {
    let mut found = false;
    input.events.retain(|event| {
        let hit = matches!(
            event,
            egui::Event::Key { key: pressed, pressed: true, repeat, modifiers, .. }
                if *pressed == key
                    && modifiers.matches_logically(pattern)
                    && (repeats || !*repeat)
        );
        found |= hit;
        !hit
    });
    if found && pattern == egui::Modifiers::NONE {
        // A plain key also arrives as the text it types. A box that has the focus by the end of
        // this frame would type it.
        input.events.retain(
            |event| !matches!(event, egui::Event::Text(text) if text.eq_ignore_ascii_case(key.symbol_or_name())),
        );
    }
    found
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    use super::*;

    /// A text box, a button that is not a text box, and what `read` made of the keys.
    struct Probe {
        text: String,
        intents: Vec<Intent>,
        focus_the_box: bool,
    }

    impl eframe::App for Probe {
        fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            let before = self.intents.len();
            read(ctx, &mut self.intents);
            self.focus_the_box |= self.intents[before..].contains(&Intent::FocusAskBar);
        }

        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            let id = egui::Id::new("question");
            if std::mem::take(&mut self.focus_the_box) {
                ui.memory_mut(|memory| memory.request_focus(id));
            }
            ui.add(egui::TextEdit::singleline(&mut self.text).id(id));
            let _chip = ui.button("A chip");
        }
    }

    fn probe() -> Harness<'static, Probe> {
        Harness::builder()
            .with_size(egui::vec2(300.0, 120.0))
            .build_eframe(|_creation| Probe {
                text: String::new(),
                intents: Vec::new(),
                focus_the_box: false,
            })
    }

    fn key_event(key: egui::Key, modifiers: egui::Modifiers, repeat: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat,
            modifiers,
        }
    }

    #[test]
    fn a_plain_key_waits_while_a_text_box_has_the_keyboard_and_a_held_key_counts_once() {
        let mut harness = probe();

        harness.key_press(egui::Key::Slash);
        harness.run();
        assert_eq!(harness.state().intents, vec![Intent::FocusAskBar]);

        // A focused widget that is not a text box does not hold a plain key back.
        harness.get_by_label("A chip").focus();
        harness.run();
        harness.key_press(egui::Key::Slash);
        harness.run();
        assert_eq!(harness.state().intents.len(), 2);

        // The slash that opened the box is not typed into it.
        harness.get_by_label("A chip").focus();
        harness.run();
        harness.state_mut().text.clear();
        harness
            .input_mut()
            .events
            .push(egui::Event::Text("/".to_owned()));
        harness.key_press(egui::Key::Slash);
        harness.run();
        assert_eq!(harness.state().intents.len(), 3);
        assert_eq!(
            harness.state().text,
            "",
            "the key that opens the box is not typed in it"
        );

        // While the box has the keyboard, plain keys are text and a ⌘ key still works.
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .focus();
        harness.run();
        for key in [egui::Key::Slash, egui::Key::J, egui::Key::Escape] {
            harness.key_press(key);
            harness.run();
        }
        assert_eq!(
            harness.state().intents.len(),
            3,
            "a plain key waits for the text box"
        );
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::K);
        harness.run();
        assert_eq!(harness.state().intents.len(), 4);

        // A held ⌘K is one press. A held J, which wants repeats, is one press for each repeat.
        harness.state_mut().intents.clear();
        harness.get_by_label("A chip").focus();
        harness.run();
        let command = egui::Modifiers::COMMAND;
        harness
            .input_mut()
            .events
            .push(key_event(egui::Key::K, command, false));
        harness.run();
        for _ in 0..2 {
            harness
                .input_mut()
                .events
                .push(key_event(egui::Key::K, command, true));
            harness.run();
        }
        assert_eq!(harness.state().intents, vec![Intent::FocusAskBar]);
        harness.state_mut().intents.clear();
        harness.get_by_label("A chip").focus();
        harness.run();
        harness
            .input_mut()
            .events
            .push(key_event(egui::Key::J, egui::Modifiers::NONE, false));
        harness.run();
        for _ in 0..2 {
            harness
                .input_mut()
                .events
                .push(key_event(egui::Key::J, egui::Modifiers::NONE, true));
            harness.run();
        }
        assert_eq!(harness.state().intents, vec![Intent::StepResult(1); 3]);
    }
}
