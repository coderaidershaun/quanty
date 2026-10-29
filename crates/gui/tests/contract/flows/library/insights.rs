//! The Insights view of a document shows the insights, takeaways and cheat sheet that were saved
//! for it, opens the page an insight rests on, and says what it finds when there are none or the
//! file cannot be read.

use backend::fake::Fake;
use backend::{Handler, Reply};
use contract::{ChapterInsights, Command, Tab};
use eframe::egui::accesskit::Role;
use gui::app::layout::DEFAULT_WINDOW;
use gui::state::SourceTarget;
use gui::testkit;

use super::{BLACK_SCHOLES, BOOK, NOTES, SAMPLE_PAGES, documents_of, open_document};
use crate::flows::recording::{self, Seen};
use crate::flows::{Window, click, copied_by, has, node, says, shared};

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
fn the_insights_view_shows_the_saved_insights_takeaways_and_cheat_sheet_and_a_page_chip_opens_its_page()
 {
    let (mut harness, seen) = open_library("black-scholes");
    open_document(&mut harness, NOTES, BLACK_SCHOLES);
    click(&mut harness, Role::Tab, "Insights");

    assert!(says(&harness, "Written by sonnet"));
    for title in ["Insights", "Takeaways", "Cheat sheet"] {
        assert!(has(&harness, Role::Label, title), "the section `{title}`");
    }
    for phrase in [
        "drops out of the price",
        "so its price can never become negative",
        "Assumptions",
        "the strike",
    ] {
        assert!(says(&harness, phrase), "the view says `{phrase}`");
    }
    for chip in ["Page 2", "Page 3"] {
        assert!(has(&harness, Role::Button, chip), "the chip `{chip}`");
    }

    let insights = shared(&harness)
        .insights
        .saved
        .ready()
        .and_then(Option::as_ref)
        .map(|saved| {
            let texts: Vec<&str> = saved
                .insights
                .iter()
                .map(|insight| insight.text.as_str())
                .collect();
            texts.join("\n\n")
        })
        .expect("the chapter has insights");
    let copied = copied_by(&mut harness, |harness| {
        node(harness, Role::Button, "Copy insights").click();
    });
    assert_eq!(copied, [insights], "a section copies its own text");

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
    assert_eq!(reads, [(chapter.id, chapter.folder.clone())]);

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
    open_document(&mut harness, NOTES, BLACK_SCHOLES);
    // The read never ends, so the app is never at rest: `click` would wait for ever.
    node(&harness, Role::Tab, "Insights").click();
    harness.run_ok();
    assert!(says(&harness, "Reading the insights"));
    assert!(
        !has(&harness, Role::Button, "Export"),
        "nothing is saved yet, so there is nothing to export"
    );
}
