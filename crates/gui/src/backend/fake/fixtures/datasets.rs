//! The build that the dataset scenes start, its steps and its report.

use std::path::Path;

use uuid::Uuid;

use crate::contract::{
    DatasetProgress, DatasetReport, DatasetRequest, DatasetStage, DocId, ModelTokens, Usage,
};

const DOCUMENTS: u32 = 2;
const SYNTHETIC_TEXTS: u32 = 12;
const SFT_EXAMPLES: u32 = 8;
// SMELL: the dollars of this play are made up, about twice what the price table gives for its
// tokens, because the fake may not name the crate that holds the table.
const COST_OF_AN_ANSWER: f64 = 0.02;
/// What Sonnet reads and what it writes to give one answer.
const TOKENS_OF_AN_ANSWER: (u64, u64) = (4_000, 1_200);
/// The model calls that a dry run says a full build would make.
const PLANNED_CALLS: u32 = 70;
/// The step at which the play of a scene whose build never ends stops for ever.
const STALL: DatasetStage = DatasetStage::WritingSynthetic {
    done: 6,
    total: SYNTHETIC_TEXTS,
};

/// The build that the two scenes with a build in them start as they open. It is a request the
/// form could have sent, so the page that fills its form from it changes nothing.
pub(in crate::backend::fake) fn request() -> DatasetRequest {
    DatasetRequest {
        name: "notes-v1".to_owned(),
        media: Some("Quanty Sample Notes".to_owned()),
        holdout: vec![DocId(Uuid::from_u128(2))],
        ..DatasetRequest::default()
    }
}

/// The play of one build: the stages that a real build tells, in the same order, with the
/// documents counted one by one and the writing counted in a few steps.
pub(in crate::backend::fake) fn steps(wanted: &DatasetRequest) -> Vec<DatasetProgress> {
    let at = |stage, answers| DatasetProgress {
        stage,
        spent: spent_on(answers),
    };
    let mut steps: Vec<DatasetProgress> = (0..=DOCUMENTS)
        .map(|done| {
            let exporting = DatasetStage::Exporting {
                done,
                total: DOCUMENTS,
            };
            at(exporting, 0)
        })
        .collect();
    steps.push(at(DatasetStage::Checking, 0));
    steps.push(at(DatasetStage::Planning, 0));
    if wanted.dry_run {
        steps.push(at(DatasetStage::WritingManifest, 0));
        return steps;
    }
    let quarters = |total: u32| (0..=4).map(move |quarter| total / 4 * quarter);
    steps.extend(quarters(SYNTHETIC_TEXTS).map(|done| {
        let writing = DatasetStage::WritingSynthetic {
            done,
            total: SYNTHETIC_TEXTS,
        };
        at(writing, done)
    }));
    steps.extend(quarters(SFT_EXAMPLES).map(|done| {
        let writing = DatasetStage::WritingSft {
            done,
            total: SFT_EXAMPLES,
        };
        at(writing, SYNTHETIC_TEXTS + done)
    }));
    steps.push(at(
        DatasetStage::WritingManifest,
        SYNTHETIC_TEXTS + SFT_EXAMPLES,
    ));
    steps
}

/// The play up to and including the stall. A dry run writes no text, so it has no stall and is
/// played whole.
pub(in crate::backend::fake) fn steps_to_the_stall(
    wanted: &DatasetRequest,
) -> Vec<DatasetProgress> {
    let mut steps = steps(wanted);
    if let Some(stalled_at) = steps.iter().position(|step| step.stage == STALL) {
        steps.truncate(stalled_at + 1);
    }
    steps
}

/// What `answers` answers used: Sonnet alone. A dry run asks nothing and used nothing.
fn spent_on(answers: u32) -> Usage {
    if answers == 0 {
        return Usage::default();
    }
    let (input, output) = TOKENS_OF_AN_ANSWER;
    let answers_given = u64::from(answers);
    Usage {
        models: vec![ModelTokens {
            model: "claude-sonnet-5-5".to_owned(),
            input: input * answers_given,
            output: output * answers_given,
            ..ModelTokens::default()
        }],
        cost_usd: Some(COST_OF_AN_ANSWER * f64::from(answers)),
    }
}

pub(in crate::backend::fake) fn report(wanted: &DatasetRequest) -> DatasetReport {
    let report = DatasetReport {
        name: wanted.name.clone(),
        folder: Path::new("datasets").join(&wanted.name),
        documents: DOCUMENTS as usize,
        sections_train: 4,
        sections_heldout: 2,
        rejected: 1,
        ..DatasetReport::default()
    };
    if wanted.dry_run {
        return DatasetReport {
            planned_calls: Some(PLANNED_CALLS),
            ..report
        };
    }
    DatasetReport {
        synthetic: SYNTHETIC_TEXTS as usize,
        sft_think: 5,
        sft_direct: 3,
        usage: spent_on(SYNTHETIC_TEXTS + SFT_EXAMPLES),
        ..report
    }
}
