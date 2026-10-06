//! Runs the real `claude` command on sample pages, because nothing offline can tell when its
//! flags or its replies change.

use std::path::PathBuf;
use std::time::Duration;

use ocr::categorise::{PageCategories, categorise_page};
use tokio::time::timeout;

const PAGE_TIMEOUT: Duration = Duration::from_secs(120);

async fn categorise_fixture(file_name: &str) -> PageCategories {
    let pdf_page = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/pdfs")
        .join(file_name);
    timeout(PAGE_TIMEOUT, categorise_page(&pdf_page))
        .await
        .unwrap_or_else(|_| panic!("categorising {file_name} took longer than {PAGE_TIMEOUT:?}"))
        .unwrap_or_else(|error| panic!("categorising {file_name} failed: {error} ({error:?})"))
}

#[tokio::test]
#[ignore = "calls the real claude CLI: needs a Claude Code login and network, and uses subscription quota"]
async fn categorises_real_pages() {
    let (tables, line_chart) = tokio::join!(
        categorise_fixture("tables.pdf"),
        categorise_fixture("diagram-2d-line-chart.pdf"),
    );

    assert!(tables.table, "tables.pdf: {tables:?}");
    assert!(
        !tables.diagram_2d_single_axis_chart,
        "tables.pdf: {tables:?}"
    );

    assert!(
        line_chart.diagram_2d_single_axis_chart,
        "diagram-2d-line-chart.pdf: {line_chart:?}"
    );
    assert!(
        !line_chart.diagram_2d_multi_axis_chart,
        "diagram-2d-line-chart.pdf: {line_chart:?}"
    );
    assert!(
        !line_chart.table,
        "diagram-2d-line-chart.pdf: {line_chart:?}"
    );
}
