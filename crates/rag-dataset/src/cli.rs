//! The arguments of the `rag-dataset` command, as clap reads them.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use rag_core::Config;
use rag_dataset::options::{
    DEFAULT_MAX_SFT, DEFAULT_MIN_PAGE_SCORE, DEFAULT_QUESTIONS, DEFAULT_REWRITES, DEFAULT_SAMPLES,
};
use rag_dataset::{BuildOptions, Selection};

#[derive(Parser)]
#[command(name = "rag-dataset")]
pub(super) struct Cli {
    #[command(subcommand)]
    pub(super) command: Command,
}

#[derive(Subcommand)]
pub(super) enum Command {
    /// Build a training dataset from the converted documents: whole sections for continued
    /// pre-training, and, unless this is a dry run, synthetic texts and fine-tuning examples
    /// that the selected model writes
    Build(GivenBuild),
}

// The structs below have no doc comment, because clap would print one as the first line of
// `rag-dataset build --help`.
#[derive(Args)]
pub(super) struct GivenBuild {
    /// The name of the dataset. It is the name of its folder in DATASET_DIR, unless --out is given
    #[arg(long, value_name = "NAME")]
    name: String,

    /// Build from this media only, whatever its capitals. Repeat it for more. Without it every
    /// media is built
    #[arg(long = "media", value_name = "TITLE")]
    media: Vec<String>,

    /// Build from this document only: the document id that an ingest prints, or its whole title.
    /// Repeat it for more. Without it every document of the chosen media is built
    #[arg(long = "document", value_name = "ID_OR_TITLE")]
    documents: Vec<String>,

    /// Keep this document out of training for evaluation: its sections go to cpt/heldout.jsonl.
    /// Named like --document. Repeat it for more
    #[arg(long = "holdout", value_name = "ID_OR_TITLE")]
    holdout: Vec<String>,

    /// Also build from the parts of a book that are not chapters, such as its index. A part
    /// titled with "appendix" is always built
    #[arg(long)]
    include_book_parts: bool,

    /// How many rewritten texts each training section gets
    #[arg(long, default_value_t = DEFAULT_REWRITES)]
    rewrites: u32,

    /// How many questions each section gets
    #[arg(long, default_value_t = DEFAULT_QUESTIONS)]
    questions: u32,

    /// The most fine-tuning examples to write
    #[arg(long, default_value_t = DEFAULT_MAX_SFT)]
    max_sft: u32,

    /// The most synthetic texts to write. Without it the plan decides
    #[arg(long, value_name = "N")]
    max_synthetic: Option<u32>,

    /// How many times the solver answers each question
    #[arg(long, default_value_t = DEFAULT_SAMPLES)]
    samples: u32,

    /// Do not run the Python that a model wrote to check an answer
    #[arg(long)]
    no_run_code: bool,

    /// Export, check and plan only: ask no model and print what a full build would ask
    #[arg(long)]
    dry_run: bool,

    /// Reject a section whose worst page matches less of its text layer than this
    #[arg(long, default_value_t = DEFAULT_MIN_PAGE_SCORE)]
    min_page_score: f64,

    /// Keep a section whose formulas do not render, instead of rejecting it
    #[arg(long)]
    keep_unrendered: bool,

    /// Do not connect to the graph. The plan then ignores how rare a concept is
    #[arg(long)]
    pub(super) no_graph: bool,

    // This help text is an attribute and not a doc comment, because rustdoc takes `<NAME>` for an
    // HTML tag that is never closed.
    #[arg(
        long,
        value_name = "DIR",
        help = "The dataset's own folder, where its files go. Without it, DATASET_DIR/<NAME>"
    )]
    out: Option<PathBuf>,
}

impl GivenBuild {
    pub(super) fn options(&self, config: &Config) -> BuildOptions {
        let mut options = BuildOptions::new(&self.name, config);
        if let Some(out) = &self.out {
            options.out = out.clone();
        }
        options.selection = Selection {
            media: self.media.clone(),
            documents: self.documents.clone(),
            holdout: self.holdout.clone(),
        };
        options.include_book_parts = self.include_book_parts;
        options.rewrites = self.rewrites;
        options.questions = self.questions;
        options.max_sft = self.max_sft;
        options.max_synthetic = self.max_synthetic;
        options.samples = self.samples;
        options.run_code = !self.no_run_code;
        options.generation = !self.dry_run;
        options.min_page_score = self.min_page_score;
        options.keep_unrendered = self.keep_unrendered;
        options
    }
}
