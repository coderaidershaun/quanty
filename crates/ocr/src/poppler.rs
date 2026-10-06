//! Thin wrappers over the Poppler command line tools, which do all the PDF work.

use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output, Stdio};
use std::time::Duration;

use tokio::process::Command;

use crate::figure::PageBox;
use crate::jev::JEV_API_KEY_VARIABLE;

const TOOL_TIMEOUT: Duration = Duration::from_secs(60);
/// The resolution of `page.png`.
const IMAGE_DPI: u32 = 150;
/// The resolution a figure is drawn at. The scans' own layers are 160 and 320 pixels per inch, so
/// 200 is sharper than the page picture, and 300 doubled the size of the largest sample figure
/// for little gain.
const FIGURE_DPI: u32 = 200;
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

    #[error("pdftotext printed no page size for {}", file.display())]
    NoPageSize { file: PathBuf },
}

/// The path to hand a tool as an argument. A relative path that starts with `-` would be read as
/// an option, so a relative path gets `./` in front. An absolute path cannot start with `-`.
fn tool_path(path: &Path) -> PathBuf {
    if path.is_relative() {
        Path::new(".").join(path)
    } else {
        path.to_path_buf()
    }
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
        // The tools have no use for the Jev API key, so they never get it.
        .env_remove(JEV_API_KEY_VARIABLE)
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
    let output = run("pdfinfo", pdf, &[tool_path(pdf).as_os_str()]).await?;
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
            tool_path(source).as_os_str(),
            tool_path(destination).as_os_str(),
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
        &[
            "-layout".as_ref(),
            tool_path(page_pdf).as_os_str(),
            "-".as_ref(),
        ],
    )
    .await?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// One line of the text layer and where it sits on the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextLine {
    /// In thousandths of the page: left and top rounded down, right and bottom rounded up.
    pub area: PageBox,
    /// The line's words, joined by single spaces.
    pub text: String,
}

/// The lines of a page's text layer, with the page size the tool printed them against.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PageText {
    pub width: f64,
    pub height: f64,
    pub lines: Vec<TextLine>,
}

/// The line boxes of a one-page PDF's text layer.
pub(crate) async fn text_lines(page_pdf: &Path) -> Result<PageText, PopplerError> {
    let output = run(
        "pdftotext",
        page_pdf,
        &[
            "-bbox-layout".as_ref(),
            tool_path(page_pdf).as_os_str(),
            "-".as_ref(),
        ],
    )
    .await?;
    parse_page_text(&String::from_utf8_lossy(&output.stdout), page_pdf)
}

// The tool's output is regular, one element per line, so it is read by hand instead of with an
// XML library.
fn parse_page_text(output: &str, file: &Path) -> Result<PageText, PopplerError> {
    let no_size = || PopplerError::NoPageSize {
        file: file.to_path_buf(),
    };
    let page_tag = tag_after(output, "<page ").ok_or_else(no_size)?;
    let width = number_attribute(page_tag, "width").ok_or_else(no_size)?;
    let height = number_attribute(page_tag, "height").ok_or_else(no_size)?;
    if width <= 0.0 || height <= 0.0 {
        return Err(no_size());
    }
    let thousandths = |value: f64, size: f64, round: fn(f64) -> f64| {
        (round(value / size * 1000.0) as i32).clamp(0, 1000)
    };
    let mut lines = Vec::new();
    for element in output.split("<line ").skip(1) {
        let element = element.split("</line>").next().unwrap_or(element);
        let tag = element.split('>').next().unwrap_or(element);
        let (Some(x_min), Some(y_min), Some(x_max), Some(y_max)) = (
            number_attribute(tag, "xMin"),
            number_attribute(tag, "yMin"),
            number_attribute(tag, "xMax"),
            number_attribute(tag, "yMax"),
        ) else {
            continue;
        };
        let words: Vec<String> = element
            .split("</word>")
            .filter_map(|chunk| chunk.rsplit_once('>'))
            .map(|(_, word)| unescape(word))
            .filter(|word| !word.is_empty())
            .collect();
        lines.push(TextLine {
            area: PageBox {
                left: thousandths(x_min, width, f64::floor),
                top: thousandths(y_min, height, f64::floor),
                right: thousandths(x_max, width, f64::ceil),
                bottom: thousandths(y_max, height, f64::ceil),
            },
            text: words.join(" "),
        });
    }
    Ok(PageText {
        width,
        height,
        lines,
    })
}

/// The text of the tag that starts with `opening`, up to its closing `>`.
fn tag_after<'a>(text: &'a str, opening: &str) -> Option<&'a str> {
    let rest = &text[text.find(opening)? + opening.len()..];
    rest.split('>').next()
}

fn number_attribute(tag: &str, name: &str) -> Option<f64> {
    let rest = &tag[tag.find(&format!("{name}=\""))? + name.len() + 2..];
    rest.split('"').next()?.parse().ok()
}

/// Turns the five escapes the tool writes back into their characters. `&amp;` goes last, so that
/// `&amp;lt;` becomes `&lt;` and not `<`.
fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Draws a one-page PDF to the PNG file `destination` at [`IMAGE_DPI`] dots per inch. This is the
/// picture that is kept.
pub(crate) async fn render_image(page_pdf: &Path, destination: &Path) -> Result<(), PopplerError> {
    let dpi = IMAGE_DPI.to_string();
    draw_png(page_pdf, destination, &["-r", &dpi]).await
}

/// Draws a one-page PDF to the PNG file `destination`, scaled so its longest side is
/// [`MODEL_IMAGE_LONG_SIDE`] pixels. This is the picture the models read.
pub(crate) async fn render_model_image(
    page_pdf: &Path,
    destination: &Path,
) -> Result<(), PopplerError> {
    draw_png(page_pdf, destination, &["-scale-to", MODEL_IMAGE_LONG_SIDE]).await
}

/// `options` say how big the picture is drawn and, for a figure, which part of the page.
async fn draw_png(
    page_pdf: &Path,
    destination: &Path,
    options: &[&str],
) -> Result<(), PopplerError> {
    // The tool adds `.png` itself, so it is given the name without it.
    let prefix = tool_path(&destination.with_extension(""));
    let source = tool_path(page_pdf);
    let mut arguments: Vec<&std::ffi::OsStr> = options.iter().map(AsRef::as_ref).collect();
    arguments.extend([
        "-png".as_ref(),
        "-singlefile".as_ref(),
        source.as_os_str(),
        prefix.as_os_str(),
    ]);
    run("pdftoppm", page_pdf, &arguments).await?;
    Ok(())
}

/// Draws only `region` of a one-page PDF to the PNG file `destination` at [`FIGURE_DPI`].
///
/// `picture_size` is the pixel size of the page's picture at [`IMAGE_DPI`], which fixes the page's
/// size in pixels at the sharper resolution. `region` must be inside the page and usable: the
/// tool never reports a bad crop, so none may be passed. The tool draws the whole page when the
/// start is beyond the page, and everything up to the page edge when the width or height is 0.
pub(crate) async fn render_region(
    page_pdf: &Path,
    picture_size: (u32, u32),
    region: PageBox,
    destination: &Path,
) -> Result<(), PopplerError> {
    let page_width = picture_size.0 * FIGURE_DPI / IMAGE_DPI;
    let page_height = picture_size.1 * FIGURE_DPI / IMAGE_DPI;
    let side = |thousandths: i32| thousandths.clamp(0, 1000) as u32;
    let left = side(region.left) * page_width / 1000;
    let top = side(region.top) * page_height / 1000;
    let right = (side(region.right) * page_width).div_ceil(1000);
    let bottom = (side(region.bottom) * page_height).div_ceil(1000);
    // A width or height of 0 means "to the page edge", so it is never passed.
    let width = right.saturating_sub(left).max(1);
    let height = bottom.saturating_sub(top).max(1);
    let [dpi, x, y, width, height] =
        [FIGURE_DPI, left, top, width, height].map(|number| number.to_string());
    let options = ["-r", &dpi, "-x", &x, "-y", &y, "-W", &width, "-H", &height];
    draw_png(page_pdf, destination, &options).await
}
