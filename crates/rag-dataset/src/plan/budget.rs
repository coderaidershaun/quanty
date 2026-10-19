//! How many texts of each kind and how many questions each section gets, and the caps on the
//! totals.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::documents::Split;
use crate::options::BuildOptions;
use crate::sample::CptKind;
use crate::section::Section;

use super::rarity::{SectionRarity, TYPICAL_FACTOR};
use super::{PlanTotals, SectionPlan};

/// The most questions a held-out section gets: enough to measure a model, too few to train on.
const HELDOUT_QUESTIONS: u32 = 2;
/// The kinds that the rewrites of one section take turns at.
const REWRITE_KINDS: [CptKind; 3] = [
    CptKind::RephraseWiki,
    CptKind::RephraseLecture,
    CptKind::RephraseQa,
];
/// About one section with a display formula in this many gets its formulas rewritten in another
/// notation, so that the model meets each idea in more than one notation without most of the
/// texts changing their symbols.
const NOTATION_ONE_IN: u64 = 20;

/// Plans every section from what the graph says about it, then cuts the totals to the caps of the
/// options, giving the rarest training sections their share first.
pub(super) fn plan_sections(
    sections: &[Section],
    rarities: Vec<SectionRarity>,
    options: &BuildOptions,
) -> Vec<SectionPlan> {
    let factors: Vec<f64> = rarities.iter().map(|rarity| rarity.factor).collect();
    let mut plans: Vec<SectionPlan> = sections
        .iter()
        .zip(rarities)
        .map(|(section, rarity)| match section.document.split {
            Split::Train => train_plan(section, rarity, options),
            Split::Heldout => heldout_plan(section, rarity, options),
        })
        .collect();
    let order = rarest_first(sections, &factors);
    cap_questions(&mut plans, &order, options.max_sft);
    if let Some(max) = options.max_synthetic {
        cap_texts(&mut plans, &order, max);
    }
    plans
}

fn heldout_plan(section: &Section, rarity: SectionRarity, options: &BuildOptions) -> SectionPlan {
    SectionPlan {
        section_id: section.id.clone(),
        concept_ids: rarity.concept_ids,
        rewrites: BTreeMap::new(),
        relations: Vec::new(),
        questions: options.questions.min(HELDOUT_QUESTIONS),
    }
}

fn train_plan(section: &Section, rarity: SectionRarity, options: &BuildOptions) -> SectionPlan {
    let factor = rarity.factor;
    let scaled = |count: u32| (f64::from(count) * factor).round() as u32;
    let rephrase = scaled(options.rewrites);
    let mut rewrites = rewrites_for(rephrase);
    // The relations come rarest first, so the ones that are left out are the commonest.
    let mut relations = rarity.relations;
    relations.truncate(rephrase as usize);
    if !relations.is_empty() {
        rewrites.insert(CptKind::Entigraph, relations.len() as u32);
    }
    if factor >= TYPICAL_FACTOR {
        rewrites.insert(CptKind::Exercise, 1);
    }
    if is_notation_pick(&section.id) && has_display_formula(&section.markdown) {
        rewrites.insert(CptKind::NotationVariant, 1);
    }
    SectionPlan {
        section_id: section.id.clone(),
        concept_ids: rarity.concept_ids,
        rewrites,
        relations,
        questions: scaled(options.questions),
    }
}

/// A number made from the id alone. It is the same on every run, machine and version of Rust,
/// which a standard hasher does not promise, so a rerun plans, and finds in its cache, the same
/// texts.
fn id_hash(section_id: &str) -> u64 {
    Sha256::digest(section_id.as_bytes())
        .iter()
        .take(8)
        .fold(0, |hash, byte| (hash << 8) | u64::from(*byte))
}

fn is_notation_pick(section_id: &str) -> bool {
    id_hash(section_id).is_multiple_of(NOTATION_ONE_IN)
}

/// A display formula is written as `$$`, the LaTeX and `$$`, each on a line of its own. Only a
/// whole line counts, because a `$$` inside a line can be an escaped dollar sign followed by
/// inline maths.
fn has_display_formula(markdown: &str) -> bool {
    markdown.lines().any(|line| line == "$$")
}

/// The positions of the training sections, the rarest first. Many sections tie (every section
/// does when the graph is not read), and the order of the sections would then spend a whole cap
/// on the first books. So a tie goes by a hash of the section id, which spreads a cut over the
/// library and is the same on every run.
fn rarest_first(sections: &[Section], factors: &[f64]) -> Vec<usize> {
    let mut order: Vec<usize> = sections
        .iter()
        .enumerate()
        .filter(|(_, section)| section.document.split == Split::Train)
        .map(|(position, _)| position)
        .collect();
    order.sort_by(|&a, &b| {
        factors[b]
            .total_cmp(&factors[a])
            .then_with(|| id_hash(&sections[a].id).cmp(&id_hash(&sections[b].id)))
    });
    order
}

fn total(counts: impl Iterator<Item = u32>) -> u32 {
    counts.fold(0, u32::saturating_add)
}

/// Gives the questions out in the order, each section keeping what it asked for until the cap is
/// used. The questions of held-out sections are not in the order, so they are not counted.
fn cap_questions(plans: &mut [SectionPlan], order: &[usize], max: u32) {
    let planned = total(order.iter().map(|&position| plans[position].questions));
    let mut left = max;
    let mut cut_sections = 0;
    for &position in order {
        let plan = &mut plans[position];
        let asked = plan.questions;
        let keep = asked.min(left);
        left -= keep;
        if keep < asked {
            cut_sections += 1;
        }
        plan.questions = keep;
    }
    let kept = total(order.iter().map(|&position| plans[position].questions));
    if kept < planned {
        tracing::info!(
            "the cap of {max} fine-tuning questions leaves out {} of the {planned} questions planned for the training sections, from {cut_sections} sections",
            planned - kept
        );
    }
}

fn texts_of(plan: &SectionPlan) -> u32 {
    total(plan.rewrites.values().copied())
}

/// Gives the texts out in the order, each section keeping what it asked for until the cap is used.
fn cap_texts(plans: &mut [SectionPlan], order: &[usize], max: u32) {
    let planned = total(order.iter().map(|&position| texts_of(&plans[position])));
    let mut left = max;
    let mut cut_sections = 0;
    for &position in order {
        let plan = &mut plans[position];
        let asked = texts_of(plan);
        let keep = asked.min(left);
        left -= keep;
        if keep < asked {
            cut_sections += 1;
        }
        keep_texts(plan, keep);
    }
    let kept = total(order.iter().map(|&position| texts_of(&plans[position])));
    if kept < planned {
        tracing::info!(
            "the cap of {max} synthetic texts leaves out {} of the {planned} texts planned for the training sections, from {cut_sections} sections",
            planned - kept
        );
    }
}

/// Cuts the section to `keep` texts. The rephrased texts are kept first, spread again over the
/// three kinds so that a cut keeps them balanced. Then come the texts about relations, with the
/// relations they explain, then the exercise, and last the notation variant.
fn keep_texts(plan: &mut SectionPlan, keep: u32) {
    let count = |kind: CptKind| plan.rewrites.get(&kind).copied().unwrap_or(0);
    let rephrased = total(REWRITE_KINDS.iter().map(|&kind| count(kind))).min(keep);
    let relations = count(CptKind::Entigraph).min(keep - rephrased);
    let exercise = count(CptKind::Exercise).min(keep - rephrased - relations);
    let notation = count(CptKind::NotationVariant).min(keep - rephrased - relations - exercise);
    plan.rewrites = rewrites_for(rephrased);
    plan.relations.truncate(relations as usize);
    for (kind, kept) in [
        (CptKind::Entigraph, relations),
        (CptKind::Exercise, exercise),
        (CptKind::NotationVariant, notation),
    ] {
        if kept > 0 {
            plan.rewrites.insert(kind, kept);
        }
    }
}

/// The rewrites take turns at the kinds, so three give one of each.
fn rewrites_for(count: u32) -> BTreeMap<CptKind, u32> {
    let kinds = REWRITE_KINDS.len() as u32;
    REWRITE_KINDS
        .iter()
        .zip(0..)
        .map(|(kind, turn)| (*kind, count / kinds + u32::from(turn < count % kinds)))
        .filter(|(_, texts)| *texts > 0)
        .collect()
}

pub(super) fn totals_of(sections: &[SectionPlan], samples: u32) -> PlanTotals {
    let mut totals = PlanTotals::default();
    for section in sections {
        for (kind, count) in &section.rewrites {
            let texts = totals.texts.entry(*kind).or_insert(0);
            *texts = texts.saturating_add(*count);
        }
        totals.questions = totals.questions.saturating_add(section.questions);
    }
    let texts = total(totals.texts.values().copied());
    totals.model_calls = texts
        .saturating_mul(2)
        .saturating_add(samples.saturating_add(2).saturating_mul(totals.questions));
    totals
}
