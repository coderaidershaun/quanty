//! Turns the keys a person presses into intents.

use eframe::egui;

use super::top_bar::is_bound;
use crate::contract::{Chord, Intent, KeyName, SHORTCUTS, When};

/// A row that matches takes its key away, so a widget never sees it too.
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

/// `InputState::consume_key` is not used because it counts every repeat of a held key.
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
        let typed = key.symbol_or_name();
        input.events.retain(
            |event| !matches!(event, egui::Event::Text(text) if text.eq_ignore_ascii_case(typed)),
        );
    }
    found
}
