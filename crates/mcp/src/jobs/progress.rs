//! How much of a whole job is done, from the place of its stage among the stages of its kind.

use super::background::JobKind;
use super::view::Stage;

/// The stages of a job that count as one stretch of its bar.
struct Part {
    names: &'static [&'static str],
    /// How many hundredths of the whole job this stretch takes.
    weight: u32,
}

const fn part(names: &'static [&'static str], weight: u32) -> Part {
    Part { names, weight }
}

// Each weight follows the time its stretch takes: about eight seconds a page for the pages, a few
// seconds a model call after them, and little for the steps that only store. Both page stages are
// one stretch because their shares are on one scale of pages.
const INGEST: [Part; 10] = [
    part(&["checking_stored"], 1),
    part(&["opening_pdf"], 2),
    part(&["preparing_pages", "converting"], 60),
    part(&["writing_artifacts"], 10),
    part(&["writing_graph"], 1),
    part(&["embedding"], 4),
    part(&["storing"], 1),
    part(&["writing_insights"], 8),
    part(&["reading_concepts"], 10),
    part(&["linking_concepts"], 3),
];

const INSIGHTS: [Part; 1] = [part(&["writing_insights"], 100)];

// The two writing stretches make one model call for each text. The others read or write files.
const DATASET: [Part; 6] = [
    part(&["exporting"], 10),
    part(&["checking"], 5),
    part(&["planning"], 5),
    part(&["writing_synthetic"], 40),
    part(&["writing_sft"], 35),
    part(&["writing_manifest"], 5),
];

// SMELL: the tables name the stages as text, and each kind of job writes the same names in its
// own code. A name changed in one place and not in the other still compiles, and the bar of the
// job then stops rising in that stage.
fn parts_of(kind: JobKind) -> &'static [Part] {
    match kind {
        JobKind::Ingest => &INGEST,
        JobKind::Insights => &INSIGHTS,
        JobKind::Dataset => &DATASET,
    }
}

/// How much of the whole job is done while it is in `stage`, from 0 to 1, to three decimals.
/// `None` when no part of the kind has a stage of that name.
pub(super) fn overall(kind: JobKind, stage: &Stage) -> Option<f64> {
    let parts = parts_of(kind);
    let index = parts
        .iter()
        .position(|part| part.names.contains(&stage.name))?;
    let before: u32 = parts[..index].iter().map(|part| part.weight).sum();
    let inside = f64::from(parts[index].weight) * stage.share.unwrap_or(0.0);
    let hundredths = f64::from(before) + inside;
    Some((hundredths * 10.0).round() / 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KINDS: [JobKind; 3] = [JobKind::Ingest, JobKind::Insights, JobKind::Dataset];

    #[test]
    fn walking_the_parts_of_a_kind_never_goes_down_and_ends_at_one() {
        for kind in KINDS {
            let mut previous = 0.0;
            for part in parts_of(kind) {
                let first = part.names.first().unwrap();
                let last = part.names.last().unwrap();
                for (name, share) in [(first, None), (last, Some(1.0))] {
                    let mut stage = Stage::new(name, String::new());
                    stage.share = share;
                    let reached = overall(kind, &stage).unwrap();
                    assert!(
                        reached >= previous,
                        "{kind:?} {name} {share:?}: {reached} < {previous}"
                    );
                    previous = reached;
                }
            }
            assert_eq!(previous, 1.0, "{kind:?}");
        }
        let unknown = Stage::new("no_such_stage", String::new());
        assert_eq!(overall(JobKind::Ingest, &unknown), None);
    }
}
