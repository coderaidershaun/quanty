//! The Figures, Formulas and Tables tabs: one card for each piece of that kind on the page.

use eframe::egui;

use super::piece;
use super::states::whole_area;
use super::tabs::{SourceTab, TabCx};
use crate::theme::{Icon, Kind};
use crate::widgets::{Card, Placeholder};

pub(super) fn show(ui: &mut egui::Ui, tab: SourceTab, cx: &mut TabCx<'_>) {
    let (page, index) = (cx.page, cx.index);
    let places = index.of(tab);
    if places.is_empty() {
        let (icon, title) = nothing_of(tab);
        whole_area(ui, Placeholder::empty(icon, title));
        return;
    }
    if piece::is_scrolled_by_person(ui) {
        *cx.reveal = None;
    }
    egui::ScrollArea::vertical()
        .id_salt(cx.scroll_id(tab))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for piece in places.iter().filter_map(|place| page.pieces.get(*place)) {
                let is_marked = cx
                    .target_piece
                    .is_some_and(|target| target.number == piece.number);
                let tag = piece
                    .label
                    .as_deref()
                    .unwrap_or_else(|| piece::kind_name(piece.kind));
                let shown = ui
                    .push_id(piece.number, |ui| {
                        Card::new()
                            .tag(Kind::from(piece.kind), tag)
                            .selected(is_marked)
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                piece::body(ui, piece, cx);
                            })
                    })
                    .inner;
                if *cx.reveal == Some(piece.number) {
                    piece::bring_into_view(&shown.response);
                }
            }
        });
}

fn nothing_of(tab: SourceTab) -> (Icon, &'static str) {
    match tab {
        SourceTab::Figures => (Icon::FIGURE, "No figures on this page"),
        SourceTab::Formulas => (Icon::FORMULA, "No formulas on this page"),
        SourceTab::Tables => (Icon::TABLE, "No tables on this page"),
        // SMELL: the Page and Concepts tabs never come here, so this text is never shown. A type
        // for the three tabs that list pieces would let the compiler say so.
        SourceTab::Page | SourceTab::Concepts => (Icon::DOCUMENT, "Nothing on this page"),
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::Rect;
    use eframe::egui::accesskit::Role;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    use crate::contract::{Intent, PieceKind};
    use crate::panels::source::samples::{
        self, DEFAULT_SIZE as DEFAULT, SMALLEST_SIZE as SMALLEST, Sample, has_picture_of,
    };
    use crate::testkit::{self, Host};
    use crate::theme::{space, stroke};

    /// Layout rounds to the pixel, so a rectangle may end this far past where it should.
    const ROUNDING: f32 = 0.5;

    /// True when the node is in view: below the tabs and above the foot of the panel.
    fn is_in_view(harness: &Harness<'static, Host>, size: [f32; 2], node: Rect) -> bool {
        let tabs = harness.get_by_role_and_label(Role::Tab, "Page").rect();
        let panel = samples::panel_rect(size);
        tabs.bottom() <= node.top() && node.bottom() <= panel.bottom()
    }

    /// Where the list under the tabs starts.
    fn list_top(harness: &Harness<'static, Host>) -> f32 {
        harness
            .get_by_role_and_label(Role::Tab, "Page")
            .rect()
            .bottom()
            + space::SM
    }

    /// True when the card that holds the node ends in view: the margin and the border under the
    /// node are inside the room of the panel.
    fn card_ends_in_view(size: [f32; 2], node: Rect) -> bool {
        let end = node.bottom() + space::MD + stroke::BORDER;
        end <= samples::panel_room(size).bottom() + ROUNDING
    }

    #[test]
    fn a_target_piece_is_marked_and_in_view_in_its_tab_or_in_the_text_of_the_page() {
        let formula = Sample::Formulas.piece(PieceKind::Formula);
        let open = samples::opened(Sample::Formulas, Some(formula.number));
        let mut harness = samples::panel(SMALLEST, open);
        samples::see_pictures(&mut harness);
        let node = harness.get_by_label(&formula.text).rect();
        assert!(is_in_view(&harness, SMALLEST, node), "{node:?}");
        assert!(
            !has_picture_of(&harness, Sample::Formulas),
            "a formula opens its own tab"
        );

        // A second click on the result brings its piece back after the person looked away.
        harness.get_by_role_and_label(Role::Tab, "Page").click();
        harness.run();
        assert!(has_picture_of(&harness, Sample::Formulas));
        let again = Intent::OpenSource {
            doc: Sample::Formulas.doc(),
            page: Sample::Formulas.page_number(),
            piece: Some(formula.number),
        };
        harness
            .state_mut()
            .shared
            .apply_intent(again, &mut Vec::new());
        samples::see_pictures(&mut harness);
        let node = harness.get_by_label(&formula.text).rect();
        assert!(is_in_view(&harness, SMALLEST, node), "{node:?}");
        assert!(!has_picture_of(&harness, Sample::Formulas));

        let formula = Sample::Text.piece(PieceKind::Formula);
        let open = samples::opened(Sample::Text, Some(formula.number));
        let mut harness = samples::panel(SMALLEST, open);
        samples::see_pictures(&mut harness);
        let node = harness.get_by_label(&formula.text).rect();
        assert!(is_in_view(&harness, SMALLEST, node), "{node:?}");
        let top = list_top(&harness);
        assert!(top <= node.top(), "the piece starts at the top: {node:?}");
        for piece in samples::page(Sample::Text).pieces {
            for text in harness.query_all_by_label(&piece.text) {
                let text = text.rect();
                assert!(
                    text.bottom() <= top || top <= text.top(),
                    "a line is cut at the top: {text:?} against {top}"
                );
            }
        }
        assert!(
            harness
                .query_all_by_label_contains("This chapter has no page pictures")
                .next()
                .is_some()
        );

        let formula = Sample::Formulas.piece(PieceKind::Formula);
        let open = samples::opened(Sample::Formulas, Some(formula.number));
        let mut harness = samples::panel(DEFAULT, open);
        samples::see_pictures(&mut harness);
        testkit::save_png(&mut harness, "source-formulas");
        let formula = Sample::Text.piece(PieceKind::Formula);
        let mut harness =
            samples::panel(DEFAULT, samples::opened(Sample::Text, Some(formula.number)));
        samples::see_pictures(&mut harness);
        testkit::save_png(&mut harness, "source-text");
        let table = Sample::Tables.piece(PieceKind::Table);
        let mut harness =
            samples::panel(DEFAULT, samples::opened(Sample::Tables, Some(table.number)));
        samples::see_pictures(&mut harness);
        assert!(
            !has_picture_of(&harness, Sample::Tables),
            "a table opens its own tab"
        );
        testkit::save_png(&mut harness, "source-tables");

        let figure = Sample::EveryKind.piece(PieceKind::Figure);
        let name = figure
            .label
            .as_deref()
            .expect("the sample figure has a label");
        let caption = figure
            .caption
            .as_deref()
            .expect("the sample figure has a caption");
        let mut harness = samples::panel(DEFAULT, samples::opened(Sample::EveryKind, None));
        samples::see_pictures(&mut harness);
        harness.get_by_role_and_label(Role::Tab, "Figures").click();
        samples::see_pictures(&mut harness);
        let picture = harness.get_by_role_and_label(Role::Image, name).rect();
        let caption = harness.get_by_label(caption).rect();
        assert!(is_in_view(&harness, DEFAULT, picture));
        assert!(picture.bottom() <= caption.top(), "{picture:?} {caption:?}");
        assert!(
            card_ends_in_view(DEFAULT, caption),
            "the whole card fits: {caption:?} in {:?}",
            samples::panel_room(DEFAULT)
        );
        testkit::save_png(&mut harness, "source-figures");
    }
}
