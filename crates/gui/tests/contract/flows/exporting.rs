//! The Share menu of the answer exports the written answer as a PDF, a Markdown file or an HTML
//! file at the place the save dialog gives, tells the person how it ended, and offers nothing
//! before the answer is written.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use eframe::egui::accesskit::Role;
use export::Format;
use gui::app::layout::DEFAULT_WINDOW;
use gui::contract::{Command, NoticeKind};
use gui::testkit;

use super::recording::{self, Seen};
use super::{Window, click, has, is_enabled, node, says, shared};

type Asked = Rc<RefCell<Vec<(Format, String)>>>;

const QUESTION: &str = "How is the Black–Scholes formula derived and what assumptions does it make";

/// Stands in for the save dialog: it writes down what it was asked and answers with the file
/// name inside `folder`.
fn stand_in_for_the_save_dialog(harness: &mut Window, folder: PathBuf) -> Asked {
    let asked = Asked::default();
    let record = asked.clone();
    harness.state_mut().set_save_dialog(move |format, name| {
        record.borrow_mut().push((format, name.to_owned()));
        Some(folder.join(name))
    });
    asked
}

#[test]
fn each_export_writes_the_file_that_the_save_dialog_names() {
    let (mut harness, _) = recording::open("black-scholes", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    let folder = tempfile::tempdir().expect("a temporary folder");
    let asked = stand_in_for_the_save_dialog(&mut harness, folder.path().to_owned());

    click(&mut harness, Role::Button, "Share");
    for item in [
        "Copy as Markdown",
        "Export as PDF…",
        "Export as Markdown…",
        "Export as HTML…",
    ] {
        assert!(has(&harness, Role::Button, item), "the menu has {item}");
    }

    let starts = [
        (Format::Pdf, "Export as PDF…", "%PDF-"),
        (Format::Markdown, "Export as Markdown…", "# Black–Scholes"),
        (Format::Html, "Export as HTML…", "<!DOCTYPE html>"),
    ];
    for (at, (format, item, beginning)) in starts.into_iter().enumerate() {
        if at > 0 {
            click(&mut harness, Role::Button, "Share");
        }
        click(&mut harness, Role::Button, item);

        let name = format!("{QUESTION}.{}", format.extension());
        assert_eq!(
            asked.borrow().last(),
            Some(&(format, name.clone())),
            "the dialog offers a name made from the question"
        );
        let written = std::fs::read(folder.path().join(&name)).expect("the file is written");
        assert!(
            written.starts_with(beginning.as_bytes()),
            "the {} file starts with {beginning}",
            format.name()
        );
        let notice = shared(&harness)
            .notices
            .first()
            .expect("the export left a notice");
        assert_eq!(notice.kind, NoticeKind::Done);
        let path = folder.path().join(&name);
        assert!(notice.detail.contains(&path.display().to_string()));
        assert!(
            says(
                &harness,
                &format!("Exported the answer as {}", format.name())
            ),
            "the answer pane shows how it ended"
        );
    }
    assert_eq!(
        asked.borrow().len(),
        3,
        "the dialog was asked once for each export"
    );
}

#[test]
fn a_write_that_fails_says_where_and_why() {
    let (mut harness, _) = recording::open("black-scholes", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    let folder = tempfile::tempdir().expect("a temporary folder");
    let missing = folder.path().join("no-such-folder");
    stand_in_for_the_save_dialog(&mut harness, missing.clone());

    click(&mut harness, Role::Button, "Share");
    click(&mut harness, Role::Button, "Export as PDF…");

    let notice = shared(&harness)
        .notices
        .first()
        .expect("the failed export left a notice");
    assert_eq!(notice.kind, NoticeKind::Failed);
    let path = missing
        .join(format!("{QUESTION}.pdf"))
        .display()
        .to_string();
    assert!(
        notice.detail.contains(&path),
        "the notice names the path: {}",
        notice.detail
    );
    assert!(
        notice.detail.contains("No such file or directory"),
        "the notice gives the reason of the system: {}",
        notice.detail
    );
    assert!(says(&harness, "Could not export the answer as PDF"));
    assert!(says(&harness, "No such file or directory"));

    click(&mut harness, Role::Button, "Dismiss");
    assert!(
        !says(&harness, "Could not export the answer as PDF"),
        "Dismiss takes the strip away"
    );
    assert!(shared(&harness).notices.is_empty());
}

#[test]
fn nothing_is_exported_before_the_answer_is_written() {
    let (mut harness, seen): (Window, Seen) = recording::open("answering", DEFAULT_WINDOW);
    let folder = tempfile::tempdir().expect("a temporary folder");
    let asked = stand_in_for_the_save_dialog(&mut harness, folder.path().to_owned());
    testkit::run_until(&mut harness, "the results", |shared| {
        shared.ask.search.ready().is_some()
    });
    harness.run_ok();

    assert!(shared(&harness).ask.answer.is_loading());
    assert!(!is_enabled(&harness, Role::Button, "Share"));
    node(&harness, Role::Button, "Share").click();
    harness.run_ok();
    assert!(!has(&harness, Role::Button, "Export as PDF…"));
    assert!(shared(&harness).ask.export_request(Format::Pdf).is_none());
    assert!(asked.borrow().is_empty());
    assert_eq!(
        seen.count(|command| matches!(command, Command::Export { .. })),
        0
    );
}
