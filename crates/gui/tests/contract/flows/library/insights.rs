//! The Insights view of a document shows the insights, takeaways and cheat sheet that were saved
//! for it, opens the page an insight rests on, says what it finds when there are none or the file
//! cannot be read, and exports all three.

use backend::fake::Fake;
use backend::{Handler, Reply};
use contract::{ChapterInsights, Command, Tab};
use eframe::egui::accesskit::Role;
use export::Format;
use gui::app::layout::DEFAULT_WINDOW;
use gui::state::SourceTarget;
use gui::testkit;

use super::{BLACK_SCHOLES, BOOK, NOTES, SAMPLE_PAGES, documents_of, open_document};
use crate::flows::exporting::downloaded_file;
use crate::flows::recording::{self, Seen};
use crate::flows::{Window, click, copied_by, has, node, says, shared};

const NOTES_CHAPTER_TITLE: &str = "Quanty Sample Notes, chapter 2: Black Scholes In Depth";

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

#[test]
fn each_export_of_the_insights_writes_a_browser_download() {
    let (mut harness, seen) = open_library("black-scholes");
    open_document(&mut harness, NOTES, BLACK_SCHOLES);
    click(&mut harness, Role::Tab, "Insights");

    click(&mut harness, Role::Button, "Export");
    for item in ["Export as PDF…", "Export as Markdown…", "Export as HTML…"] {
        assert!(has(&harness, Role::Button, item), "the menu has {item}");
    }

    let starts = [
        (Format::Pdf, "Export as PDF…", "%PDF-"),
        (
            Format::Markdown,
            "Export as Markdown…",
            "# Quanty Sample Notes, chapter 2: Black Scholes In Depth",
        ),
        (Format::Html, "Export as HTML…", "<!DOCTYPE html>"),
    ];
    for (at, (format, item, beginning)) in starts.into_iter().enumerate() {
        if at > 0 {
            click(&mut harness, Role::Button, "Export");
        }
        click(&mut harness, Role::Button, item);

        let name = format.file_name(&format!("{NOTES_CHAPTER_TITLE} insights"));
        assert_eq!(
            shared(&harness)
                .export
                .as_ref()
                .unwrap()
                .download
                .as_ref()
                .unwrap()
                .file_name,
            name
        );
        let written = std::fs::read(downloaded_file(&harness, &seen)).expect("the file is written");
        assert!(
            written.starts_with(beginning.as_bytes()),
            "the {} file starts with {beginning}",
            format.name()
        );
        if format == Format::Markdown {
            let text = String::from_utf8(written).expect("Markdown is text");
            assert!(text.contains("## Takeaways"));
            assert!(text.contains("so its price can never become negative"));
        }
        assert!(
            says(
                &harness,
                &format!("Exported the insights as {}", format.name())
            ),
            "the view shows how it ended"
        );
    }
    assert_eq!(
        seen.count(|command| matches!(command, Command::Export { .. })),
        3
    );
}
