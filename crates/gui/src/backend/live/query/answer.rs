//! Turns the answer of the model into what the answer pane draws: headings, paragraphs with the
//! numbers they cite, and one card for each formula, figure or table.

use std::collections::HashSet;

use rag_core::ItemKind;
use rag_retrieval::Claim;

use crate::contract::{self, AnswerBlock};

pub(super) fn view(answer: rag_retrieval::Answer) -> contract::Answer {
    contract::Answer {
        title: answer.title,
        blocks: blocks(&answer.claims),
        follow_ups: answer.follow_ups,
    }
}

fn blocks(claims: &[Claim]) -> Vec<AnswerBlock> {
    let mut blocks = Vec::new();
    let mut heading_in_force: Option<&str> = None;
    let mut carded = HashSet::new();
    for claim in claims {
        if let Some(heading) = claim.heading.as_deref()
            && heading_in_force != Some(heading)
        {
            blocks.push(AnswerBlock::Heading(heading.to_owned()));
            heading_in_force = Some(heading);
        }
        blocks.push(AnswerBlock::Paragraph {
            text: claim.text.clone(),
            cites: claim.sources.iter().map(|source| source.number).collect(),
        });
        for source in &claim.sources {
            if source.payload.kind != ItemKind::Chunk && carded.insert(source.number) {
                blocks.push(AnswerBlock::Item(source.number));
            }
        }
    }
    blocks
}
