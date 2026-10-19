//! What a person asks a dataset build to do, how far a build has got, and what it made.

use crate::contract::{DocId, Usage};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DatasetRequest {
    /// The name of the dataset, which names its folder. The panel sends it trimmed.
    pub name: String,
    /// `None`: every media.
    pub media: Option<String>,
    /// The documents kept out of training, for evaluation.
    pub holdout: Vec<DocId>,
    /// Rewritten texts for each training section.
    pub rewrites: u32,
    /// Questions for each section.
    pub questions: u32,
    /// The most fine-tuning examples the build writes.
    pub max_sft: u32,
    /// Exports, checks and plans, and asks no model.
    pub dry_run: bool,
}

// SMELL: the dataset crate keeps its own copy of these defaults, and the contract may not name
// that crate, so a change to one must be made in both.
impl Default for DatasetRequest {
    fn default() -> Self {
        DatasetRequest {
            name: String::new(),
            media: None,
            holdout: Vec::new(),
            rewrites: 3,
            questions: 2,
            max_sft: 300,
            dry_run: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DatasetStage {
    /// `done` of all `total` documents are exported.
    Exporting {
        done: u32,
        total: u32,
    },
    Checking,
    Planning,
    /// `done` of all `total` synthetic texts are written.
    WritingSynthetic {
        done: u32,
        total: u32,
    },
    /// `done` of all `total` fine-tuning examples are written.
    WritingSft {
        done: u32,
        total: u32,
    },
    WritingManifest,
}

impl DatasetStage {
    /// `done / total` for the three stages that count, `None` for the others and for a total of 0.
    pub fn share_done(self) -> Option<f32> {
        match self {
            DatasetStage::Exporting { done, total }
            | DatasetStage::WritingSynthetic { done, total }
            | DatasetStage::WritingSft { done, total } => {
                (total > 0).then(|| done as f32 / total as f32)
            }
            DatasetStage::Checking | DatasetStage::Planning | DatasetStage::WritingManifest => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct DatasetProgress {
    pub stage: DatasetStage,
    /// What the build used so far.
    pub spent: Usage,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub struct DatasetReport {
    pub name: String,
    /// The live backend makes it absolute.
    pub folder: std::path::PathBuf,
    /// Exported documents, for training and held out.
    pub documents: usize,
    pub sections_train: usize,
    pub sections_heldout: usize,
    /// Sections the checks kept out, for every reason.
    pub rejected: usize,
    /// Rewritten texts of the training sections.
    pub synthetic: usize,
    /// Fine-tuning examples whose answer writes out its working.
    pub sft_think: usize,
    /// Fine-tuning examples answered straight out.
    pub sft_direct: usize,
    /// `Some` for a dry run only: the model calls a full build would make.
    pub planned_calls: Option<u32>,
    pub usage: Usage,
}

impl DatasetReport {
    /// The counts in words. A build: "2 documents, 4 sections for training and 2 held out, 1
    /// rejected. 12 synthetic texts and 8 fine-tuning examples (5 think, 3 direct)." A dry run:
    /// "2 documents, 4 sections for training and 2 held out, 1 rejected. Dry run: 70 model calls
    /// planned, none made."
    pub fn summary(&self) -> String {
        let exported = format!(
            "{}, {} for training and {} held out, {} rejected.",
            count(self.documents, "document"),
            count(self.sections_train, "section"),
            self.sections_heldout,
            self.rejected,
        );
        match self.planned_calls {
            Some(calls) => format!(
                "{exported} Dry run: {} planned, none made.",
                count(calls as usize, "model call")
            ),
            None => format!(
                "{exported} {} and {} ({} think, {} direct).",
                count(self.synthetic, "synthetic text"),
                count(self.sft_think + self.sft_direct, "fine-tuning example"),
                self.sft_think,
                self.sft_direct,
            ),
        }
    }
}

fn count(number: usize, noun: &str) -> String {
    match number {
        1 => format!("1 {noun}"),
        _ => format!("{number} {noun}s"),
    }
}

#[cfg(test)]
mod tests {
    use super::DatasetStage;

    #[test]
    fn a_stage_with_nothing_to_count_has_no_share() {
        // A build asked for no questions tells its fine-tuning stage as 0 of 0, and 0 / 0 would
        // read "about NaN%".
        let empty = [
            DatasetStage::Exporting { done: 0, total: 0 },
            DatasetStage::WritingSynthetic { done: 0, total: 0 },
            DatasetStage::WritingSft { done: 0, total: 0 },
        ];
        for stage in empty {
            assert_eq!(stage.share_done(), None, "{stage:?}");
        }
    }
}
