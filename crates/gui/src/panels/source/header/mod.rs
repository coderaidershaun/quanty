//! The top of the source panel: its title, the book and chapter pickers, and the page pager.

mod pager;
mod pickers;

use eframe::egui;

use super::row;
use crate::contract::Intent;
use crate::panels::PanelCx;
use crate::widgets::section_header;
use pager::PagerText;
use pickers::Lists;

const TITLE: &str = "Source in Context";
/// The panel's inner height from which the header has three rows. Below it the pager moves up
/// beside the title and the two pickers share a row, so the page keeps its room.
const FOLD_BELOW: f32 = 440.0;
/// The part of a row that the book picker takes when the chapter picker is beside it.
const BOOK_SHARE: f32 = 0.55;

/// What the header remembers between frames, so it builds its texts only when their source
/// changes.
#[derive(Debug, Default)]
pub(super) struct Cache {
    lists: Lists,
    pager: PagerText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rows {
    Two,
    Three,
}

pub(super) fn show(ui: &mut egui::Ui, cache: &mut Cache, cx: &mut PanelCx<'_>) {
    let shared = cx.shared;
    cache.lists.refresh(shared);
    let rows = if ui.available_height() >= FOLD_BELOW {
        Rows::Three
    } else {
        Rows::Two
    };
    let [book, chapter] = cache.lists.pickers(shared);
    let (mut opened_book, mut opened_chapter, mut turn) = (None, None, None);
    match rows {
        Rows::Three => {
            section_header(ui, TITLE, |_| {});
            let room = ui.available_width();
            opened_book = book.show(ui, room);
            row::show(ui, |ui| {
                let gap = ui.spacing().item_spacing.x;
                let rest = ui.available_width() - gap - cache.pager.width();
                opened_chapter = chapter.show(ui, rest.max(0.0));
                turn = cache.pager.show(ui, &shared.source);
            });
        }
        Rows::Two => {
            turn = row::show(ui, |ui| {
                section_header(ui, TITLE, |ui| cache.pager.show(ui, &shared.source))
            });
            row::show(ui, |ui| {
                opened_book = book.show(ui, BOOK_SHARE * ui.available_width());
                opened_chapter = chapter.show(ui, ui.available_width());
            });
        }
    }
    for doc in [opened_book, opened_chapter].into_iter().flatten() {
        cx.intents.push(Intent::OpenSource {
            doc,
            page: 1,
            piece: None,
        });
    }
    if let Some(delta) = turn {
        cx.intents.push(Intent::TurnPage(delta));
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::kittest::{NodeT, Queryable};

    use crate::contract::{Event, FailureKind, Intent};
    use crate::panels::source::samples::{
        self, DEFAULT_SIZE as SIZE, Sample, chapter_text, has_picture_of, place_text, printed_text,
        shows,
    };
    use crate::state::Shared;
    use crate::testkit::{self, sample};

    #[test]
    fn the_pickers_open_a_chapter_and_the_pager_turns_while_the_old_page_stays() {
        let top = samples::page(Sample::FigureTop);
        let next = samples::page(Sample::EveryKind);
        let mut harness = samples::panel(SIZE, samples::opened(Sample::FigureTop, None));
        samples::see_pictures(&mut harness);
        assert!(shows(&harness, &place_text(&top)));
        assert!(shows(&harness, &printed_text(&top)));

        harness.get_by_label("Next page").click();
        harness.run();
        assert_eq!(harness.state().intents, vec![Intent::TurnPage(1)]);
        harness.state_mut().apply_intents();
        harness.run();
        assert!(shows(&harness, &place_text(&next)));
        assert!(shows(&harness, "…"));
        assert!(
            has_picture_of(&harness, Sample::FigureTop),
            "the old page stays while the next loads"
        );
        testkit::save_png(&mut harness, "source-turning");

        samples::land(&mut harness.state_mut().shared, Sample::EveryKind);
        harness.run();
        assert!(shows(&harness, &place_text(&next)));
        assert!(shows(&harness, &printed_text(&next)));
        assert!(has_picture_of(&harness, Sample::EveryKind));

        harness.state_mut().shared = samples::opened(Sample::FigureLow, None);
        harness.run();
        harness.get_by_label("Next page").click();
        harness.run();
        assert!(
            harness.state().intents.is_empty(),
            "the last page has no next"
        );

        let open_book = samples::book_title(Sample::FigureLow.doc());
        let other_book = samples::book_title(Sample::Text.doc());
        harness.get_by_label("Book").click();
        harness.run();
        harness.get_by_label(&open_book).click();
        harness.run();
        assert!(
            harness.state().intents.is_empty(),
            "the book that is open is not opened again"
        );
        harness.get_by_label("Book").click();
        harness.run();
        harness.get_by_label(&other_book).click();
        harness.run();
        let (first_doc, first_text) = samples::first_chapter(Sample::Text.doc());
        let first_of_other_book = Intent::OpenSource {
            doc: first_doc,
            page: 1,
            piece: None,
        };
        assert_eq!(harness.state().intents, vec![first_of_other_book.clone()]);

        harness.state_mut().intents.clear();
        harness.state_mut().shared = samples::opened(Sample::Text, None);
        harness.run();
        harness.get_by_label("Chapter").click();
        harness.run();
        harness.get_by_label(&first_text).click();
        harness.run();
        assert_eq!(harness.state().intents, vec![first_of_other_book]);

        // A library that is not read: both pickers are off and say what the page says.
        let mut shared = Shared::default();
        shared.apply_intent(
            Intent::OpenSource {
                doc: Sample::FigureTop.doc(),
                page: top.page,
                piece: None,
            },
            &mut Vec::new(),
        );
        samples::land(&mut shared, Sample::FigureTop);
        harness.state_mut().shared = shared;
        harness.state_mut().intents.clear();
        harness.run();
        assert!(harness.get_by_label("Book").accesskit_node().is_disabled());
        assert!(
            harness
                .get_by_label("Chapter")
                .accesskit_node()
                .is_disabled()
        );
        let book = harness.get_by_label("Book").value();
        assert_eq!(book, top.book);
        let chapter = harness.get_by_label("Chapter").value();
        let label = top
            .chapter
            .as_ref()
            .expect("the sample page names its chapter");
        assert_eq!(chapter, Some(chapter_text(label)));
        harness.get_by_label("Book").hover();
        harness.run_ok();
        assert!(shows(&harness, "Reading the library…"));

        let mut shared = samples::library(samples::catalogue());
        shared.apply_intent(Intent::RefreshCatalogue, &mut Vec::new());
        let request = shared.library.pending.expect("a read is waiting");
        let failure = sample::failure(FailureKind::QdrantDown);
        let reply = Event::Catalogue {
            request,
            result: Err(failure.clone()),
        };
        shared.apply_event(reply, &mut Vec::new());
        harness.state_mut().shared = shared;
        harness.run();
        harness.get_by_label("Chapter").hover();
        harness.run_ok();
        assert!(shows(&harness, &failure.hint));
    }
}
