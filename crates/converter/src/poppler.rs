//! Thin wrappers over the Poppler command line tools, which do all the PDF work.

use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output, Stdio};
use std::time::Duration;

use tokio::process::Command;

const TOOL_TIMEOUT: Duration = Duration::from_secs(60);
const IMAGE_DPI: &str = "150";
// The longest side, in pixels, of the picture the models read. Small book pages are drawn far
// larger than the saved picture, because bold letters in math only show at that sharpness.
const MODEL_IMAGE_LONG_SIDE: &str = "1568";

/// Why a Poppler tool failed.
#[derive(thiserror::Error, Debug)]
pub enum PopplerError {
    #[error("could not start {tool}; install Poppler (for example `brew install poppler`)")]
    Start {
        tool: &'static str,
        #[source]
        source: std::io::Error,
    },

    #[error("{tool} did not finish within {} seconds", TOOL_TIMEOUT.as_secs())]
    TimedOut { tool: &'static str },

    #[error("{tool} failed ({status}) on {}: {stderr}", file.display())]
    Failed {
        tool: &'static str,
        file: PathBuf,
        status: ExitStatus,
        stderr: String,
    },

    #[error("pdfinfo printed no page count for {}", file.display())]
    NoPageCount { file: PathBuf },
}

async fn run(
    tool: &'static str,
    file: &Path,
    arguments: &[&std::ffi::OsStr],
) -> Result<Output, PopplerError> {
    let mut command = Command::new(tool);
    command
        .args(arguments)
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(TOOL_TIMEOUT, command.output())
        .await
        .map_err(|_| PopplerError::TimedOut { tool })?
        .map_err(|source| PopplerError::Start { tool, source })?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(PopplerError::Failed {
            tool,
            file: file.to_path_buf(),
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        })
    }
}

pub(crate) async fn page_count(pdf: &Path) -> Result<u32, PopplerError> {
    let output = run("pdfinfo", pdf, &[pdf.as_os_str()]).await?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("Pages:"))
        .and_then(|count| count.trim().parse().ok())
        .ok_or_else(|| PopplerError::NoPageCount {
            file: pdf.to_path_buf(),
        })
}

/// Writes page `position` (1 is the first) of `source` to `destination` as a one-page PDF.
pub(crate) async fn cut_page(
    source: &Path,
    position: u32,
    destination: &Path,
) -> Result<(), PopplerError> {
    let position = position.to_string();
    let position = std::ffi::OsStr::new(&position);
    run(
        "pdfseparate",
        source,
        &[
            "-f".as_ref(),
            position,
            "-l".as_ref(),
            position,
            source.as_os_str(),
            destination.as_os_str(),
        ],
    )
    .await?;
    Ok(())
}

/// The text layer of a one-page PDF, laid out as printed, exactly as the tool prints it.
pub(crate) async fn text_layer(page_pdf: &Path) -> Result<String, PopplerError> {
    let output = run(
        "pdftotext",
        page_pdf,
        &["-layout".as_ref(), page_pdf.as_os_str(), "-".as_ref()],
    )
    .await?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Draws a one-page PDF to the PNG file `destination` at [`IMAGE_DPI`] dots per inch. This is the
/// picture that is kept.
pub(crate) async fn render_image(page_pdf: &Path, destination: &Path) -> Result<(), PopplerError> {
    draw_png(page_pdf, destination, ["-r", IMAGE_DPI]).await
}

/// Draws a one-page PDF to the PNG file `destination`, scaled so its longest side is
/// [`MODEL_IMAGE_LONG_SIDE`] pixels. This is the picture the models read.
pub(crate) async fn render_model_image(
    page_pdf: &Path,
    destination: &Path,
) -> Result<(), PopplerError> {
    draw_png(page_pdf, destination, ["-scale-to", MODEL_IMAGE_LONG_SIDE]).await
}

async fn draw_png(
    page_pdf: &Path,
    destination: &Path,
    [size_option, size_value]: [&str; 2],
) -> Result<(), PopplerError> {
    // The tool adds `.png` itself, so it is given the name without it.
    let prefix = destination.with_extension("");
    run(
        "pdftoppm",
        page_pdf,
        &[
            size_option.as_ref(),
            size_value.as_ref(),
            "-png".as_ref(),
            "-singlefile".as_ref(),
            page_pdf.as_os_str(),
            prefix.as_os_str(),
        ],
    )
    .await?;
    Ok(())
}
