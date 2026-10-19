//! What one build is asked to do: the dataset's name and folders, which documents it takes, which
//! it holds out, and how much it writes.

use std::path::PathBuf;

use rag_core::Config;
use serde::Serialize;

/// How many rewritten texts are written from each passage of a training section.
pub const DEFAULT_REWRITES: u32 = 3;
/// How many questions each section gets.
pub const DEFAULT_QUESTIONS: u32 = 2;
/// The most fine-tuning examples a build writes.
pub const DEFAULT_MAX_SFT: u32 = 300;
/// How many times the solver answers one question.
pub const DEFAULT_SAMPLES: u32 = 3;
/// A section whose worst page matches less of the text layer than this is rejected. Inline math on
/// a transcribed page counts as words that the text layer prints as symbols, so a good page of
/// math matches less than a page of prose does.
pub const DEFAULT_MIN_PAGE_SCORE: f64 = 0.90;

#[derive(Debug, Clone, Serialize)]
pub struct BuildOptions {
    /// The dataset's name. It is the name of its folder when `out` is not given.
    pub name: String,
    /// The dataset's own folder. Its files go straight in it.
    pub out: PathBuf,
    pub content_folder: PathBuf,
    /// Where the answers of the models are kept, so that a second build asks nothing.
    pub cache_folder: PathBuf,
    pub selection: Selection,
    /// Also export the parts of a book that are not chapters, such as its index.
    pub include_book_parts: bool,
    pub rewrites: u32,
    pub questions: u32,
    pub max_sft: u32,
    pub max_synthetic: Option<u32>,
    /// How many times the solver answers one question.
    pub samples: u32,
    /// Run the Python that a model wrote to check an answer.
    pub run_code: bool,
    /// False for a dry run, which exports, checks and plans and asks no model.
    pub generation: bool,
    pub min_page_score: f64,
    /// Keep a section whose formulas do not render, instead of rejecting it.
    pub keep_unrendered: bool,
}

impl BuildOptions {
    /// A build of the whole library with every default, writing to the dataset folder of the
    /// config under the name.
    pub fn new(name: &str, config: &Config) -> BuildOptions {
        BuildOptions {
            name: name.to_owned(),
            out: config.dataset_folder.join(name),
            content_folder: config.content_folder.clone(),
            cache_folder: config.dataset_cache_folder.clone(),
            selection: Selection::default(),
            include_book_parts: false,
            rewrites: DEFAULT_REWRITES,
            questions: DEFAULT_QUESTIONS,
            max_sft: DEFAULT_MAX_SFT,
            max_synthetic: None,
            samples: DEFAULT_SAMPLES,
            run_code: true,
            generation: true,
            min_page_score: DEFAULT_MIN_PAGE_SCORE,
            keep_unrendered: false,
        }
    }
}

/// Which documents a build takes and which it holds out. Titles match whatever their capitals.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Selection {
    /// Titles of media. Empty means every media.
    pub media: Vec<String>,
    /// Document ids (the UUID that an ingest prints) or whole stored titles, such as "Option
    /// Volatility and Pricing, chapter 1: Sample Pages". Empty means every document of the chosen
    /// media.
    pub documents: Vec<String>,
    /// Documents kept out of training for evaluation, named like `documents`.
    pub holdout: Vec<String>,
}
