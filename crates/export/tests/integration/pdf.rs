//! A document written as a PDF.

use export::{Format, write};

use crate::support;

#[test]
fn pdf_holds_the_whole_document_with_its_pictures() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let path = folder.path().join("answer.pdf");

    write(&support::answer_like(), Format::Pdf, &path).expect("the PDF is written");

    assert!(std::fs::read(&path).unwrap().starts_with(b"%PDF-"));
    let text = support::pdf_text(&path);
    for phrase in [
        "Question:HowistheBlack–Scholesformuladerived?",
        "Black–Scholesformulaanditsassumptions",
        "Whythehedgeremovesrisk",
        "Thepriceofanoptionfollowsfromapartialdifferentialequation",
        "thevolatility",
        "isconstant.",
        "(2.3)",
        "Figure1.1Volatilityovertime",
        "Table1.1Greeks",
        "Delta",
        "Pricechangeperunitofthestock",
        "defcall_price(spot,strike):",
        "Listing2.1ThecallpriceinPython",
        "[1]",
        "[2]",
        "Sources",
        "Hull,chapter2:Options,p.7",
        "Natenberg,chapter1:Volatility,p.5—Figure1.1",
    ] {
        assert!(
            text.contains(phrase),
            "the PDF has no {phrase:?} in {text:?}"
        );
    }
    assert!(!text.contains("\\frac"), "formulas are pictures, not LaTeX");

    let sample = image::ImageReader::open(support::sample_picture())
        .unwrap()
        .into_dimensions()
        .unwrap();
    let pictures = support::pdf_pictures(&path);
    assert!(pictures.contains(&(i64::from(sample.0), i64::from(sample.1))));
    let formulas = pictures
        .iter()
        .filter(|&&picture| picture != (i64::from(sample.0), i64::from(sample.1)))
        .count();
    assert!(
        formulas >= 2,
        "the inline formula and the one set apart are pictures"
    );
}
