//! Converts one chapter PDF into pieces saved under `content/`.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use converter::{ChapterJob, convert_chapter};

const USAGE: &str =
    "usage: converter --book \"<Book Title>\" [--out <folder>] chapter-<number>-<name>.pdf

Breaks every page of a chapter PDF into its pieces and saves them under
<folder>/<book-title>/chapter-<number>/. The folder defaults to `content`.
A chapter that is already converted is not converted again.";

struct Arguments {
    book_title: String,
    output_root: PathBuf,
    chapter_pdf: PathBuf,
}

/// `None` means `--help` was asked for.
fn parse_arguments(mut arguments: impl Iterator<Item = String>) -> Result<Option<Arguments>> {
    let mut book_title = None;
    let mut output_root = None;
    let mut chapter_pdf = None;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--help" | "-h" => return Ok(None),
            "--book" => {
                book_title = Some(arguments.next().context("--book needs a book title")?);
            }
            "--out" => {
                output_root = Some(PathBuf::from(
                    arguments.next().context("--out needs a folder")?,
                ));
            }
            option if option.starts_with("--") => bail!("unknown option {option}"),
            path if chapter_pdf.is_none() => chapter_pdf = Some(PathBuf::from(path)),
            extra => bail!("expected one chapter PDF but also got {extra}"),
        }
    }
    Ok(Some(Arguments {
        book_title: book_title.context("missing --book \"<Book Title>\"")?,
        output_root: output_root.unwrap_or_else(|| PathBuf::from("content")),
        chapter_pdf: chapter_pdf
            .context("missing the chapter PDF, named chapter-<number>-<name>.pdf")?,
    }))
}

async fn run(arguments: Arguments) -> Result<()> {
    // A missing .env is fine. It is loaded before anything else runs, so an ANTHROPIC_API_KEY
    // placed in it is refused just like one set in the shell.
    match dotenvy::dotenv() {
        Ok(_) => {}
        Err(error) if error.not_found() => {}
        Err(error) => return Err(error).context("could not read the .env file"),
    }
    let job = ChapterJob::new(
        &arguments.book_title,
        &arguments.chapter_pdf,
        &arguments.output_root,
    )
    .context("could not start the conversion")?;
    println!("converting into {}", job.chapter_folder().display());
    let summary = convert_chapter(&job)
        .await
        .context("the chapter was not converted")?;
    println!("{summary}");
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    let arguments = match parse_arguments(std::env::args().skip(1)) {
        Ok(Some(arguments)) => arguments,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("error: {error:#}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    match run(arguments).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}
