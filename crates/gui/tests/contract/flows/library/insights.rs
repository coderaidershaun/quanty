//! The Insights, Takeaways and Cheat sheet views of a document show what was saved for them, the
//! Insights view opens the page an insight rests on, and the views say what they find when there
//! are none or the file cannot be read.

use backend::fake::Fake;
use backend::{Handler, Reply};
use contract::{ChapterInsights, Command, Tab};
use eframe::egui::accesskit::Role;
use gui::app::layout::DEFAULT_WINDOW;
use gui::state::SourceTarget;
use gui::testkit;

use super::{
    BLACK_SCHOLES, BOOK, NOTES, SAMPLE_PAGES, documents_of, open_document, open_row_while_busy,
};
use crate::flows::recording::{self, Seen};
use crate::flows::{Window, click, copied_by, has, is_open_tab, node, says, shared};

/// The fake, except that a read of insights is never answered.
struct NeverAnswersInsights(Fake);

impl Handler for NeverAnswersInsights {
    async fn serve(&self, command: Command, reply: Reply) {
        if !matches!(command, Command::LoadChapterInsights { .. }) {
            self.0.serve(command, reply).await;
        }
    }
}

fn open_library(scene: &str) -> (Window, Seen) {
    let (mut harness, seen) = recording::open(scene, DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    click(&mut harness, Role::Tab, "Library");
    (harness, seen)
}

fn reads_of_insights(seen: &Seen) -> usize {
    seen.count(|command| matches!(command, Command::LoadChapterInsights { .. }))
}

#[test]
fn the_insights_takeaways_and_cheat_sheet_views_show_what_was_saved_and_a_page_chip_opens_its_page()
{
    let (mut harness, seen) = open_library("black-scholes");
    open_document(&mut harness, NOTES, BLACK_SCHOLES);
    assert!(
        is_open_tab(&harness, "Insights"),
        "a document opens on the Insights"
    );

    let views = [
        ("Insights", "drops out of the price"),
        ("Takeaways", "so its price can never become negative"),
        ("Cheat sheet", "Assumptions"),
    ];
    for (tab, phrase) in views {
        click(&mut harness, Role::Tab, tab);
        assert!(
            says(&harness, "Written by sonnet"),
            "`{tab}` names the model"
        );
        assert!(says(&harness, phrase), "`{tab}` says `{phrase}`");
        for (other, other_phrase) in views {
            if other != tab {
                assert!(
                    !says(&harness, other_phrase),
                    "`{tab}` does not say `{other_phrase}` of `{other}`"
                );
            }
        }
        let has_chips = has(&harness, Role::Button, "Page 2");
        assert_eq!(
            has_chips,
            tab == "Insights",
            "only the Insights has page chips"
        );
    }
    click(&mut harness, Role::Tab, "Insights");
    for chip in ["Page 2", "Page 3"] {
        assert!(has(&harness, Role::Button, chip), "the chip `{chip}`");
    }

    let chapter = documents_of(&harness)
        .into_iter()
        .find(|document| document.name().label() == BLACK_SCHOLES)
        .expect("the notes chapter is listed");
    let reads: Vec<_> = seen
        .all()
        .into_iter()
        .filter_map(|command| match command {
            Command::LoadChapterInsights { doc, folder, .. } => Some((doc, folder)),
            _ => None,
        })
        .collect();
    assert_eq!(
        reads,
        [(chapter.id, chapter.folder.clone())],
        "the three views share one read"
    );

    click(&mut harness, Role::Button, "Page 2");
    assert_eq!(shared(&harness).tab, Tab::Ask);
    assert_eq!(
        shared(&harness).source.target,
        Some(SourceTarget {
            doc: chapter.id,
            page: 2,
            piece: None
        })
    );
}

#[test]
fn each_state_of_the_insights_view_says_what_it_is() {
    let (mut harness, seen) = open_library("black-scholes");

    open_document(&mut harness, NOTES, "Index");
    click(&mut harness, Role::Tab, "Insights");
    assert!(says(&harness, "No insights for this part"));
    assert!(!has(&harness, Role::Button, "Copy command"));
    assert_eq!(
        reads_of_insights(&seen),
        0,
        "a part of a book is never read"
    );

    open_document(&mut harness, BOOK, SAMPLE_PAGES);
    let sample_pages = documents_of(&harness)
        .into_iter()
        .find(|document| document.name().label() == SAMPLE_PAGES)
        .expect("the book's chapter is listed");
    let command = ChapterInsights::command_for(sample_pages.id);
    assert!(says(&harness, "No insights yet"));
    assert!(says(&harness, &command), "the hint names the command");
    let copied = copied_by(&mut harness, |harness| {
        node(harness, Role::Button, "Copy command").click();
    });
    assert_eq!(copied, [command]);

    let (mut harness, seen) = open_library("source-missing");
    open_document(&mut harness, NOTES, BLACK_SCHOLES);
    click(&mut harness, Role::Tab, "Insights");
    assert!(says(&harness, "The insights could not be read"));
    assert!(says(
        &harness,
        "does not know where this document's pages are"
    ));
    assert_eq!(reads_of_insights(&seen), 1);
    click(&mut harness, Role::Button, "Try again");
    assert_eq!(
        reads_of_insights(&seen),
        2,
        "Try again reads the file again"
    );

    let (mut harness, _seen) =
        recording::open_with("black-scholes", DEFAULT_WINDOW, NeverAnswersInsights);
    testkit::settle(&mut harness);
    click(&mut harness, Role::Tab, "Library");
    // The Insights view is the first one shown. Its read never ends, so the app is never at rest:
    // `click` would wait for ever.
    open_row_while_busy(&mut harness, NOTES);
    open_row_while_busy(&mut harness, BLACK_SCHOLES);
    assert!(says(&harness, "Reading the insights"));
    assert!(
        !has(&harness, Role::Button, "Export"),
        "nothing is saved yet, so there is nothing to export"
    );
}
