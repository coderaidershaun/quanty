//! Names a pdf of a book that is given a title, with no store: the part is stored under its book,
//! and its folder comes from its own title.

use std::path::Path;

use ocr::{ChapterJob, DocumentName, MediaDocument};
use rag_core::Category;
use rag_ingestion::{NamedPdf, UnnamedPdf};

#[test]
fn a_book_pdf_given_a_title_is_a_part_named_after_its_book() {
    let unnamed = UnnamedPdf {
        media_title: "Option Volatility & Pricing",
        category: Some(Category::Book),
        document_title: Some(" Index "),
        pdf: Path::new("notes-17.pdf"),
    };

    let named = unnamed.named_by(Category::Book).unwrap();

    assert_eq!(
        named,
        NamedPdf {
            category: Category::Book,
            document: MediaDocument {
                media_title: "Option Volatility & Pricing".to_owned(),
                name: DocumentName::Title("Option Volatility & Pricing, Index".to_owned()),
            },
        }
    );
    let content_folder = Path::new("content");
    let job = ChapterJob::new(named.document, unnamed.pdf, content_folder).unwrap();
    assert_eq!(
        job.chapter_folder(),
        content_folder
            .join("option-volatility-pricing")
            .join("index")
    );
}
