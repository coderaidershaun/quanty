//! Checks that the fake backend loads every page of the three sample chapters, and that it knows
//! the pages that the other tests rely on.

use gui::backend::fake::Fake;
use gui::backend::{Handler, Reply};
use gui::contract::{
    Command, DocId, Event, Failure, FailureKind, PageBox, PageView, PieceKind, RequestId,
};
use uuid::Uuid;

fn doc(number: u128) -> DocId {
    DocId(Uuid::from_u128(number))
}

fn serve_all(fake: &Fake, command: Command) -> Vec<Event> {
    let (reply, received, _stop) = Reply::collecting();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime starts")
        .block_on(fake.serve(command, reply));
    received.try_iter().collect()
}

fn load_page(fake: &Fake, doc: DocId, page: u32) -> (Result<PageView, Failure>, Event) {
    let command = Command::LoadPage {
        request: RequestId(1),
        doc,
        page,
        folder: None,
    };
    let mut events = serve_all(fake, command).into_iter();
    let (page, concepts) = (events.next(), events.next());
    assert!(events.next().is_none(), "a page load sends two events");
    match (page, concepts) {
        (Some(Event::Page { result, .. }), Some(concepts)) => (result, concepts),
        other => panic!("a page load did not send a page and its concepts: {other:?}"),
    }
}

fn assert_page_loads(fake: &Fake, doc_number: u128, page: u32, pages: u32) -> Event {
    let (view, concepts) = load_page(fake, doc(doc_number), page);
    let view = view.unwrap_or_else(|failure| panic!("page {page} of {doc_number}: {failure:?}"));
    assert!(matches!(
        concepts,
        Event::PageConcepts { result: Ok(_), .. }
    ));
    assert_eq!(
        (view.doc, view.page, view.page_count),
        (doc(doc_number), page, pages)
    );
    assert!(view.book.is_some() && view.chapter.is_some() && !view.pieces.is_empty());
    for piece in &view.pieces {
        assert!(
            !piece.text.ends_with('\n'),
            "a piece keeps no closing newline"
        );
        let picture = piece.image.as_ref().map(|image| image.path.is_file());
        assert_eq!(
            picture.is_some(),
            piece.kind == PieceKind::Figure,
            "only a figure has a picture"
        );
        assert!(picture.unwrap_or(true), "a picture that is named is there");
    }
    let has_pictures = doc_number == 3;
    assert_eq!(
        view.image.is_some(),
        has_pictures,
        "only the book has page pictures"
    );
    assert_eq!(view.previous_image.is_some(), has_pictures && page > 1);
    assert_eq!(view.next_image.is_some(), has_pictures && page < pages);
    concepts
}

fn assert_known_pages(fake: &Fake) {
    let figure_page = load_page(fake, doc(3), 5)
        .0
        .expect("page 5 of the book loads");
    assert_eq!(figure_page.printed_page.as_deref(), Some("233"));
    assert_eq!(figure_page.pieces.len(), 4);
    let figure = &figure_page.pieces[0];
    assert_eq!(figure.label.as_deref(), Some("Figure 13-4"));
    let cut = PageBox {
        left: 15,
        top: 23,
        right: 905,
        bottom: 485,
    };
    assert_eq!(figure.cut, Some(cut));
    let picture = figure.image.as_ref().expect("the figure has its picture");
    assert!(picture.path.ends_with("page-num-5/01-figure.png"));

    let notes_page = load_page(fake, doc(2), 3)
        .0
        .expect("page 3 of chapter 2 loads");
    assert_eq!(notes_page.printed_page.as_deref(), Some("7"));
    assert_eq!(notes_page.pieces.len(), 10);
    assert_eq!(notes_page.pieces[2].kind, PieceKind::Heading { rank: 2 });
    let formula = &notes_page.pieces[4];
    assert_eq!(formula.kind, PieceKind::Formula);
    assert_eq!(formula.label.as_deref(), Some("(2.4)"));
    assert_eq!(formula.name.as_deref(), Some("Black–Scholes call price"));

    let missing = load_page(fake, doc(3), 8)
        .0
        .expect_err("page 8 of 7 does not load");
    assert_eq!(missing.kind, FailureKind::SourceMissing);
}

pub(super) fn assert_every_sample_page_loads(fake: &Fake) {
    let mut empty_pages = 0;
    for (doc_number, pages) in [(1, 3), (2, 3), (3, 7)] {
        for page in 1..=pages {
            let concepts = assert_page_loads(fake, doc_number, page, pages);
            empty_pages += usize::from(
                matches!(concepts, Event::PageConcepts { result: Ok(list), .. } if list.is_empty()),
            );
        }
    }
    assert!(
        empty_pages > 0 && empty_pages < 13,
        "some sample pages have concepts and some have none"
    );
    assert_known_pages(fake);
}
