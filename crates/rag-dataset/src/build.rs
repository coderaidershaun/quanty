//! Runs one build from the converted documents to the manifest, stage by stage, and tells the
//! caller where it is as it goes.

use std::collections::{BTreeMap, HashMap};
use std::io;
use std::path::PathBuf;

use graph::{FalkorGraph, GraphError};
use ocr::{Catalogue, ContentError, ReadChapterError};
use rag_core::{ConceptId, Llm, LlmError, UsageTally};
use rag_ingestion::chapter_items;

use crate::checks::{Checked, Reason, Seen, Verdict, check};
use crate::documents::{ExportDocument, SelectionError, Split, exported_documents};
use crate::generate::asker::{AskError, Asker};
use crate::generate::{self, GenerateError};
use crate::layout::{DatasetFolder, WriteError, write_json, write_jsonl};
use crate::manifest::{Manifest, ManifestDocument, ManifestError};
use crate::options::BuildOptions;
use crate::plan::{GenerationPlan, PlanError, plan_for};
use crate::progress::{Report, SplitCounts, Step, Written};
use crate::sample::{CptSample, Dedup, Quality, RejectedSample};
use crate::section::{Section, sections_of};
use crate::sft::{self, SftError};

#[derive(thiserror::Error, Debug)]
pub enum BuildError {
    #[error(
        "{name:?} cannot name a dataset folder; use letters, digits, '-', '_' and '.', and not '.' or '..'"
    )]
    Name { name: String },

    #[error("could not find the full path of the content folder {}; check CONTENT_DIR", path.display())]
    ContentFolder {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("could not read the converted documents; check CONTENT_DIR")]
    Catalogue(#[source] ContentError),

    #[error(transparent)]
    Selection(#[from] SelectionError),

    #[error(
        "there is no document to export; check that CONTENT_DIR holds converted documents, that the media and documents named exist and are finished, and that a part of a book is asked for with --include-book-parts"
    )]
    NothingToExport,

    #[error(
        "the model is not ready to write the dataset; sign in to it or fix its settings, or run again as a dry run, which asks no model"
    )]
    ModelNotReady {
        #[source]
        source: LlmError,
    },

    #[error("could not read the chapter in {}; convert it again", folder.display())]
    Chapter {
        folder: PathBuf,
        #[source]
        source: ReadChapterError,
    },

    #[error("could not remove {}, which an earlier build of this dataset wrote; remove it and run again", path.display())]
    Remove {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error(transparent)]
    Write(#[from] WriteError),

    #[error(transparent)]
    Plan(#[from] PlanError),

    #[error(transparent)]
    Ask(#[from] AskError),

    #[error(transparent)]
    Generate(#[from] GenerateError),

    #[error(transparent)]
    Sft(#[from] SftError),

    #[error(transparent)]
    Manifest(#[from] ManifestError),
}

impl BuildError {
    /// The failure of the model behind this error, so that a caller can word it by the service.
    pub fn llm_error(&self) -> Option<&LlmError> {
        match self {
            BuildError::ModelNotReady { source }
            | BuildError::Ask(AskError::Stopped(source))
            | BuildError::Generate(GenerateError::Ask(AskError::Stopped(source)))
            | BuildError::Sft(SftError::Ask(AskError::Stopped(source))) => Some(source),
            _ => None,
        }
    }

    /// The failure of the graph behind this error, so that a caller can word it by the service.
    pub fn graph_error(&self) -> Option<&GraphError> {
        match self {
            BuildError::Plan(PlanError::Graph(source)) => Some(source),
            _ => None,
        }
    }
}

/// A section with what the checks found out about it.
struct CheckedSection {
    section: Section,
    quality: Quality,
    dedup: Dedup,
}

/// Exports the sections of the chosen documents, checks and plans them, and, unless the options
/// ask for a dry run, asks the model to write the synthetic texts and the examples. The dataset's
/// files are written to `options.out`; the files of an earlier build of the same dataset are
/// removed first, so that the folder holds only this build.
///
/// `on_step` is told each step with what the models have cost so far, which is nothing in a dry
/// run.
///
/// # Errors
/// - [`BuildError::Name`] when the name cannot name a folder, and [`BuildError::Selection`] when
///   the selection names a media or a document that is not there. Nothing is written for either.
/// - [`BuildError::NothingToExport`] when the selection leaves no document
/// - [`BuildError::ModelNotReady`] when the options ask for generation and the model is not ready.
///   Nothing is removed or written for it. A dry run asks no model, so it never fails this way.
/// - [`BuildError::Catalogue`], [`BuildError::ContentFolder`] and [`BuildError::Chapter`] when
///   the converted documents cannot be read
/// - [`BuildError::Remove`], [`BuildError::Write`] and [`BuildError::Manifest`] when the folder
///   of the dataset cannot be used
/// - [`BuildError::Plan`] when the graph cannot be read, [`BuildError::Ask`] when the folder of
///   kept answers cannot be made, and [`BuildError::Generate`] and [`BuildError::Sft`] when the
///   model can answer no question or a file cannot be written
pub async fn build(
    options: &BuildOptions,
    llm: &impl Llm,
    graph: Option<&FalkorGraph>,
    on_step: &mut (dyn FnMut(Step, &UsageTally) + Send),
) -> Result<Report, BuildError> {
    check_name(&options.name)?;
    let documents = chosen_documents(options)?;
    // Checked before the earlier files are removed, so that a model that is not ready leaves the
    // last build in place.
    if options.generation {
        llm.check_ready()
            .await
            .map_err(|source| BuildError::ModelNotReady { source })?;
    }
    let folder = DatasetFolder::new(&options.out);
    remove_earlier_files(&folder)?;

    let no_usage = UsageTally::default();
    let sections = export(&documents, on_step, &no_usage)?;

    on_step(Step::Checking, &no_usage);
    let (kept, rejected) = check_sections(sections, options);
    let kept_sections: Vec<Section> = kept.iter().map(|kept| kept.section.clone()).collect();

    on_step(Step::Planning, &no_usage);
    let plan = plan_for(&kept_sections, graph, options).await?;
    write_json(&folder.plan(), &plan)?;
    write_cpt_files(&folder, &kept, &rejected, &plan)?;

    // SMELL: the synthetic texts and the fine-tuning examples take the same six inputs, so each
    // new input is added to both functions and to both calls below. The inputs want one struct.
    let (synthetic, sft, usage) = if options.generation {
        let asker = Asker::new(llm, &options.cache_folder)?;
        let synthetic =
            generate::write_synthetic(&plan, &kept_sections, &asker, &folder, options, on_step)
                .await?;
        let sft = sft::write_sft(&plan, &kept_sections, &asker, &folder, options, on_step).await?;
        (synthetic, sft, asker.usage())
    } else {
        (
            Written::default(),
            Written::default(),
            UsageTally::default(),
        )
    };

    on_step(Step::WritingManifest, &usage);
    Manifest::of(
        options,
        &folder,
        manifest_documents(&documents, &kept),
        &usage,
    )?
    .write(&folder)?;

    Ok(Report {
        name: options.name.clone(),
        folder: options.out.clone(),
        documents: split_counts(documents.iter().map(|document| document.split)),
        sections: split_counts(kept.iter().map(|kept| kept.section.document.split)),
        rejected: reason_counts(&rejected),
        planned: plan.totals,
        generation: options.generation,
        synthetic,
        sft,
        usage,
    })
}

fn check_name(name: &str) -> Result<(), BuildError> {
    let is_usable = !name.is_empty()
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_.".contains(character));
    if is_usable {
        Ok(())
    } else {
        Err(BuildError::Name {
            name: name.to_owned(),
        })
    }
}

/// The documents of the content folder that the options choose, with their folders made absolute.
fn chosen_documents(options: &BuildOptions) -> Result<Vec<ExportDocument>, BuildError> {
    let content_folder = std::path::absolute(&options.content_folder).map_err(|source| {
        BuildError::ContentFolder {
            path: options.content_folder.clone(),
            source,
        }
    })?;
    let catalogue = Catalogue::read(&content_folder).map_err(BuildError::Catalogue)?;
    let documents = exported_documents(&catalogue, options)?;
    if documents.is_empty() {
        return Err(BuildError::NothingToExport);
    }
    Ok(documents)
}

/// Only the files a build writes are removed, so a folder that holds other things keeps them.
fn remove_earlier_files(folder: &DatasetFolder) -> Result<(), BuildError> {
    let files = folder
        .jsonl_files()
        .into_iter()
        .chain([folder.plan(), folder.manifest()]);
    for path in files {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(BuildError::Remove { path, source }),
        }
    }
    Ok(())
}

/// The sections of every document, in the order of the documents.
fn export(
    documents: &[ExportDocument],
    on_step: &mut (dyn FnMut(Step, &UsageTally) + Send),
    no_usage: &UsageTally,
) -> Result<Vec<Section>, BuildError> {
    let total = documents.len();
    on_step(Step::Exporting { done: 0, total }, no_usage);
    let mut sections = Vec::new();
    for (position, document) in documents.iter().enumerate() {
        let chapter =
            ocr::read_chapter(&document.folder).map_err(|source| BuildError::Chapter {
                folder: document.folder.clone(),
                source,
            })?;
        let items = chapter_items(&chapter);
        sections.extend(sections_of(document, &chapter, &items));
        on_step(
            Step::Exporting {
                done: position + 1,
                total,
            },
            no_usage,
        );
    }
    Ok(sections)
}

/// The sections the checks kept, and the ones they kept out with the reason, each in the order of
/// the sections.
fn check_sections(
    sections: Vec<Section>,
    options: &BuildOptions,
) -> (Vec<CheckedSection>, Vec<(CheckedSection, Reason)>) {
    let mut seen = Seen::default();
    let mut kept = Vec::new();
    let mut rejected = Vec::new();
    for section in sections {
        let Checked {
            quality,
            dedup,
            verdict,
        } = check(&section, &mut seen, options);
        let checked = CheckedSection {
            section,
            quality,
            dedup,
        };
        match verdict {
            Verdict::Keep => kept.push(checked),
            Verdict::Reject(reason) => rejected.push((checked, reason)),
        }
    }
    (kept, rejected)
}

/// The kept sections of the training documents, those of the held-out documents, and the ones the
/// checks kept out. Each file is written even when it is empty, after the plan, so that the
/// concepts the plan found are in the records.
fn write_cpt_files(
    folder: &DatasetFolder,
    kept: &[CheckedSection],
    rejected: &[(CheckedSection, Reason)],
    plan: &GenerationPlan,
) -> Result<(), WriteError> {
    let concepts: HashMap<&str, &[ConceptId]> = plan
        .sections
        .iter()
        .map(|section| (section.section_id.as_str(), section.concept_ids.as_slice()))
        .collect();
    let record = |checked: &CheckedSection| {
        let concept_ids = concepts
            .get(checked.section.id.as_str())
            .copied()
            .unwrap_or_default();
        CptSample::raw(
            &checked.section,
            checked.quality.clone(),
            checked.dedup.clone(),
            concept_ids,
        )
    };
    let [train, heldout] = [Split::Train, Split::Heldout].map(|split| {
        kept.iter()
            .filter(|kept| kept.section.document.split == split)
            .map(&record)
            .collect::<Vec<_>>()
    });
    let rejected: Vec<RejectedSample> = rejected
        .iter()
        .map(|(checked, reason)| RejectedSample {
            sample: record(checked),
            rejected: reason.to_string(),
        })
        .collect();
    write_jsonl(&folder.cpt_raw(), &train)?;
    write_jsonl(&folder.cpt_heldout(), &heldout)?;
    write_jsonl(&folder.cpt_rejected(), &rejected)
}

fn manifest_documents(
    documents: &[ExportDocument],
    kept: &[CheckedSection],
) -> Vec<ManifestDocument> {
    documents
        .iter()
        .map(|document| {
            let sections: Vec<&Section> = kept
                .iter()
                .map(|kept| &kept.section)
                .filter(|section| section.document.id == document.id)
                .collect();
            ManifestDocument {
                document_id: document.id,
                title: document.title.clone(),
                doc_role: document.role,
                split: document.split,
                sections: sections.len(),
                n_tokens: sections.iter().map(|section| section.n_tokens).sum(),
            }
        })
        .collect()
}

/// The rejected sections, counted by the name of their reason.
fn reason_counts(rejected: &[(CheckedSection, Reason)]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for (_, reason) in rejected {
        *counts.entry(reason.name().to_owned()).or_insert(0) += 1;
    }
    counts
}

fn split_counts(splits: impl Iterator<Item = Split>) -> SplitCounts {
    splits.fold(SplitCounts::default(), |mut counts, split| {
        match split {
            Split::Train => counts.train += 1,
            Split::Heldout => counts.heldout += 1,
        }
        counts
    })
}
