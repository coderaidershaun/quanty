//! What a build reports while it works and when it is done: the steps it passes, and the summary of
//! what it wrote.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use rag_core::{UsageTally, cost_text};

use crate::plan::PlanTotals;

/// Where a build is. The counts of a step are the ones to show a person.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Exporting {
        done: usize,
        total: usize,
    },
    Checking,
    Planning,
    Writing {
        kind: GenerationKind,
        done: usize,
        total: usize,
    },
    WritingManifest,
}

/// The two stages that ask a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationKind {
    /// The rewritten texts for continued pre-training.
    Synthetic,
    /// The questions and answers for fine-tuning.
    Sft,
}

/// What a stage wrote, counted by the spelling the files use.
#[derive(Debug, Clone, Default)]
pub struct Written {
    /// A kind of text, or a task of an example.
    pub by_kind: BTreeMap<String, usize>,
    /// Fine-tuning only: `think` and `direct`.
    pub by_reasoning: BTreeMap<String, usize>,
    /// What the checks kept out, by reason.
    pub rejected: BTreeMap<String, usize>,
}

impl Written {
    pub fn total(&self) -> usize {
        self.by_kind.values().sum()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SplitCounts {
    pub train: usize,
    pub heldout: usize,
}

/// What a build did.
#[derive(Debug, Clone)]
pub struct Report {
    pub name: String,
    pub folder: PathBuf,
    pub documents: SplitCounts,
    /// The sections that were kept.
    pub sections: SplitCounts,
    /// The sections that were kept out, by the name of the reason.
    pub rejected: BTreeMap<String, usize>,
    pub planned: PlanTotals,
    /// False for a dry run, which asked no model.
    pub generation: bool,
    pub synthetic: Written,
    pub sft: Written,
    pub usage: UsageTally,
}

impl fmt::Display for Report {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(formatter, "dataset: {}", self.name)?;
        writeln!(formatter, "folder: {}", self.folder.display())?;
        writeln!(
            formatter,
            "documents: {} for training, {} held out",
            self.documents.train, self.documents.heldout
        )?;
        writeln!(
            formatter,
            "sections: {} for training, {} held out, {} rejected",
            self.sections.train,
            self.sections.heldout,
            self.rejected.values().sum::<usize>()
        )?;
        if !self.rejected.is_empty() {
            writeln!(formatter, "rejected: {}", counts_text(&self.rejected))?;
        }
        let planned_texts = self
            .planned
            .texts
            .iter()
            .map(|(kind, count)| (kind.as_str(), count));
        writeln!(formatter, "planned texts: {}", counts_text(planned_texts))?;
        writeln!(formatter, "planned questions: {}", self.planned.questions)?;
        if self.generation {
            self.write_generated(formatter)?;
        } else {
            writeln!(
                formatter,
                "planned model calls: {}",
                self.planned.model_calls
            )?;
            writeln!(formatter, "dry run: no model was asked")?;
        }
        if !self.usage.is_empty() {
            writeln!(formatter, "{}", self.usage)?;
        }
        write!(
            formatter,
            "cost of this run: {}",
            cost_text(self.usage.cost_usd())
        )
    }
}

impl Report {
    fn write_generated(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            formatter,
            "synthetic texts: {}",
            counts_text(&self.synthetic.by_kind)
        )?;
        if self.sft.by_kind.is_empty() {
            return writeln!(formatter, "SFT examples: none");
        }
        let reasoning = |name: &str| self.sft.by_reasoning.get(name).copied().unwrap_or(0);
        writeln!(
            formatter,
            "SFT examples: {} (think {}, direct {})",
            counts_text(&self.sft.by_kind),
            reasoning("think"),
            reasoning("direct")
        )
    }
}

/// `kind 3, other 1`, or `none` when there is nothing to count.
fn counts_text<N: fmt::Display, C: fmt::Display>(
    counts: impl IntoIterator<Item = (N, C)>,
) -> String {
    let counts: Vec<String> = counts
        .into_iter()
        .map(|(name, count)| format!("{name} {count}"))
        .collect();
    if counts.is_empty() {
        "none".to_owned()
    } else {
        counts.join(", ")
    }
}
