//! The record of what one build made: the options it ran with, the documents and files it holds
//! with their hashes and counts, how the dataset is mixed, what was left out, and what the models
//! cost.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rag_core::DocId;
use serde::Serialize;
use serde_json::Value;

use crate::documents::{DocRole, Split};
use crate::generate;
use crate::layout::{DatasetFolder, WriteError, write_json};
use crate::options::BuildOptions;
use crate::progress::Report;
use crate::sample::PIPELINE_VERSION;
use crate::sft;

/// What the dataset does not do, in plain words, so that a person who trains on it does not take
/// a null or an estimate for a measurement.
const NOTES: [&str; 8] = [
    "`dedup.minhash_cluster` is null: near-duplicate sections are not looked for, only exact copies are removed.",
    "`n_tokens` is an estimate of four characters to a token, because no Gemma tokenizer is used; count the tokens with the tokenizer of the model before you pack or budget.",
    "Nothing is packed: every record is one text or one conversation, and the training run packs them into sequences.",
    "No answer is checked with sympy; a calculation is checked by the agreement of its tries and a judge, and a program by running it.",
    "`ifd` and `known_by_cpt_model` are null: neither is measured.",
    "Nothing is decontaminated: no text or example is compared with any benchmark.",
    "No replay data is included: the public text that continued pre-training mixes in to keep the model's general skills is left to the training run.",
    "How to train on this dataset is in docs/training.md of the quanty repository.",
];

/// The scores a judge gives, from the lowest to the highest.
const JUDGE_SCORES: std::ops::RangeInclusive<u8> = 1..=5;

#[derive(Debug, Clone, Serialize)]
pub struct Manifest {
    pub name: String,
    /// Seconds since 1970-01-01 UTC. The workspace has no date crate, so the time is a plain count.
    pub created_at: u64,
    pub pipeline_version: String,
    /// The prompts a full build asks, such as `rephrase_wiki@1`. A dry run lists them too, because
    /// the same build would ask them.
    pub prompt_versions: Vec<String>,
    pub options: BuildOptions,
    pub documents: Vec<ManifestDocument>,
    pub files: Vec<ManifestFile>,
    /// The version of the dataset: the SHA-256 of one line `<file sha256>  <path>` for each entry
    /// of `files`, in order. Two builds that wrote the same files have the same version, whenever
    /// they ran.
    pub sha256: String,
    pub mix: Mix,
    pub rejected: Rejected,
    pub judge_scores: JudgeScores,
    pub usage: Vec<ModelUsageLine>,
    /// `None` when a model has no price.
    pub cost_usd: Option<f64>,
    pub notes: Vec<String>,
}

/// What the dataset holds, counted the way the summary of the build counts it.
#[derive(Debug, Clone, Serialize)]
pub struct Mix {
    /// The kept sections of the training documents.
    pub raw: usize,
    /// The kept sections of the held-out documents.
    pub heldout: usize,
    /// The synthetic texts written, by kind, such as `notation_variant`.
    pub synthetic_by_kind: BTreeMap<String, usize>,
    /// The kept fine-tuning examples, by task. Training and held-out examples are counted together.
    pub sft_by_task: BTreeMap<String, usize>,
    pub think: usize,
    pub direct: usize,
}

/// What was left out, as a count for each reason.
#[derive(Debug, Clone, Serialize)]
pub struct Rejected {
    /// Sections kept out by the checks: `page_score`, `latex` or `duplicate_of`.
    pub sections: BTreeMap<String, usize>,
    /// Drafts of synthetic texts that were turned down or not answered, by the id of the role that
    /// wrote them, such as `notation`. The kind of a kept text is not the id of its role.
    pub synthetic: BTreeMap<String, usize>,
    /// Fine-tuning questions that failed: `no_answer`, `disagreement`, `judge` or `python_exec`.
    pub sft: BTreeMap<String, usize>,
}

/// How many records carry each score from 1 to 5, read back from the files. A record with no judge
/// score, such as a raw text or an example checked by running its program, is not counted. A
/// score outside 1 to 5 is counted under its own number.
#[derive(Debug, Clone, Serialize)]
pub struct JudgeScores {
    /// The kept synthetic texts only, so a score of 1 or 2 is always zero: a draft that was turned
    /// down has no record. It shows in `rejected.synthetic`.
    pub synthetic: BTreeMap<u8, usize>,
    /// The fine-tuning examples that a judge scored, kept or not.
    pub sft: BTreeMap<u8, usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ManifestDocument {
    pub document_id: DocId,
    pub title: String,
    pub doc_role: DocRole,
    pub split: Split,
    /// The sections that were kept.
    pub sections: usize,
    pub n_tokens: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ManifestFile {
    /// Relative to the dataset folder, with `/` between the names.
    pub path: String,
    /// Hex of the SHA-256 of the file's bytes.
    pub sha256: String,
    /// The lines of the file.
    pub samples: usize,
    /// The sum of the `n_tokens` of its lines. Zero for a file whose lines have none.
    pub n_tokens: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelUsageLine {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub estimated: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum ManifestError {
    #[error(
        "could not read {} to list it in the manifest; check that the dataset folder can be read",
        path.display()
    )]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error(transparent)]
    Write(#[from] WriteError),
}

impl Manifest {
    /// The manifest of a dataset whose files are written. It lists every file of records that
    /// exists in the folder, and reads each of them once. The counts of what was made, the usage
    /// and the cost come from the report of the build; the judge scores exist only in the records,
    /// so they are read back from the files.
    ///
    /// # Errors
    /// [`ManifestError::Read`] when a file of the dataset cannot be read.
    pub fn of(
        options: &BuildOptions,
        folder: &DatasetFolder,
        documents: Vec<ManifestDocument>,
        report: &Report,
    ) -> Result<Manifest, ManifestError> {
        let mut files = Vec::new();
        let mut judge_scores = JudgeScores::new();
        for path in folder.jsonl_files() {
            if path.is_file() {
                let scores = judge_scores.counting(folder, &path);
                files.push(ManifestFile::read(folder.root(), &path, scores)?);
            }
        }
        // SMELL: the report counts the examples under the plain strings "think" and "direct", with
        // no constant for either, so a typo here still compiles and counts nothing. The desktop app
        // reads them the same way.
        let by_reasoning = |name: &str| report.sft.by_reasoning.get(name).copied().unwrap_or(0);
        let sha256 = dataset_version(&files);
        Ok(Manifest {
            name: options.name.clone(),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.as_secs()),
            pipeline_version: PIPELINE_VERSION.to_owned(),
            prompt_versions: generate::ROLES
                .iter()
                .chain(&sft::ROLES)
                .map(|role| role.prompt_id())
                .collect(),
            options: options.clone(),
            documents,
            files,
            sha256,
            mix: Mix {
                raw: report.sections.train,
                heldout: report.sections.heldout,
                synthetic_by_kind: report.synthetic.by_kind.clone(),
                sft_by_task: report.sft.by_kind.clone(),
                think: by_reasoning("think"),
                direct: by_reasoning("direct"),
            },
            rejected: Rejected {
                sections: report.rejected.clone(),
                synthetic: report.synthetic.rejected.clone(),
                sft: report.sft.rejected.clone(),
            },
            judge_scores,
            usage: report
                .usage
                .models()
                .map(|(model, used)| ModelUsageLine {
                    model: model.to_owned(),
                    input_tokens: used.tokens.input_tokens,
                    output_tokens: used.tokens.output_tokens,
                    cache_read_tokens: used.tokens.cache_read_tokens,
                    cache_write_tokens: used.tokens.cache_write_tokens,
                    estimated: used.estimated,
                })
                .collect(),
            cost_usd: report.usage.cost_usd(),
            notes: NOTES.map(str::to_owned).to_vec(),
        })
    }

    /// # Errors
    /// [`WriteError`] when the manifest cannot be written.
    pub fn write(&self, folder: &DatasetFolder) -> Result<(), WriteError> {
        write_json(&folder.manifest(), self)
    }
}

/// The hash of the hashes of the files, as `sha256sum` prints them, one line for each file.
fn dataset_version(files: &[ManifestFile]) -> String {
    let lines: String = files
        .iter()
        .map(|file| format!("{}  {}\n", file.sha256, file.path))
        .collect();
    ocr::sha256_hex(lines.as_bytes())
}

impl JudgeScores {
    /// Every score from 1 to 5 at zero.
    fn new() -> JudgeScores {
        let zeros = || JUDGE_SCORES.map(|score| (score, 0)).collect();
        JudgeScores {
            synthetic: zeros(),
            sft: zeros(),
        }
    }

    /// The count that the judge scores in this file add to, and where a record keeps its score.
    /// The other files hold no judge score.
    fn counting(
        &mut self,
        folder: &DatasetFolder,
        path: &Path,
    ) -> Option<(&mut BTreeMap<u8, usize>, &'static str)> {
        if path == folder.cpt_synthetic() {
            Some((&mut self.synthetic, "/quality/judge_score"))
        } else if [
            folder.sft_train(),
            folder.sft_heldout(),
            folder.sft_rejected(),
        ]
        .iter()
        .any(|sft_file| sft_file == path)
        {
            Some((&mut self.sft, "/verification/judge_score"))
        } else {
            None
        }
    }
}

impl ManifestFile {
    /// Reads the file once: its hash, its lines, the `n_tokens` of its records and, when `scores`
    /// says where a record keeps its judge score, the scores.
    fn read(
        root: &Path,
        path: &Path,
        mut scores: Option<(&mut BTreeMap<u8, usize>, &str)>,
    ) -> Result<ManifestFile, ManifestError> {
        let bytes = fs::read(path).map_err(|source| ManifestError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let relative = path.strip_prefix(root).unwrap_or(path);
        let names: Vec<_> = relative
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect();
        let text = String::from_utf8_lossy(&bytes);
        let mut samples = 0;
        let mut n_tokens = 0;
        for line in text.lines() {
            samples += 1;
            let Ok(record) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            n_tokens += record
                .get("n_tokens")
                .and_then(Value::as_u64)
                .map_or(0, |tokens| tokens as usize);
            if let Some((counts, pointer)) = &mut scores
                && let Some(score) = record.pointer(pointer).and_then(Value::as_u64)
                && let Ok(score) = u8::try_from(score)
            {
                *counts.entry(score).or_insert(0) += 1;
            }
        }
        Ok(ManifestFile {
            path: names.join("/"),
            sha256: ocr::sha256_hex(&bytes),
            samples,
            n_tokens,
        })
    }
}
