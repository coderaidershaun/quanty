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

#[cfg(test)]
mod tests {
    use rag_core::{DocId, DocumentLabels, ItemPayload};
    use rag_retrieval::Source;

    use super::*;

    fn source(number: usize, kind: ItemKind) -> Source {
        Source {
            number,
            payload: ItemPayload {
                doc_id: DocId::from_source_sha256("a-document"),
                doc_title: "A book".to_owned(),
                page: 1,
                printed_page: None,
                kind,
                text: "text".to_owned(),
                image_path: None,
                label: None,
                cites: Vec::new(),
                document_labels: DocumentLabels::default(),
            },
        }
    }

    fn claim(heading: Option<&str>, text: &str, sources: Vec<Source>) -> Claim {
        Claim {
            heading: heading.map(str::to_owned),
            text: text.to_owned(),
            sources,
        }
    }

    fn paragraph(text: &str, cites: &[usize]) -> AnswerBlock {
        AnswerBlock::Paragraph {
            text: text.to_owned(),
            cites: cites.to_vec(),
        }
    }

    fn heading(text: &str) -> AnswerBlock {
        AnswerBlock::Heading(text.to_owned())
    }

    #[test]
    fn a_card_follows_the_first_paragraph_that_cites_its_result_and_a_heading_is_written_once() {
        let claims = vec![
            claim(None, "A", vec![source(1, ItemKind::Formula)]),
            claim(
                Some("Key assumptions"),
                "B",
                vec![
                    source(2, ItemKind::Chunk),
                    source(3, ItemKind::Table),
                    source(1, ItemKind::Formula),
                ],
            ),
            claim(
                Some("Key assumptions"),
                "C",
                vec![source(3, ItemKind::Table)],
            ),
            claim(None, "D", vec![source(4, ItemKind::Figure)]),
            claim(Some("Limits"), "E", vec![source(2, ItemKind::Chunk)]),
        ];

        let answer = view(rag_retrieval::Answer {
            title: Some("Pricing".to_owned()),
            claims,
            follow_ups: vec!["What is a put?".to_owned()],
        });

        assert_eq!(
            answer,
            contract::Answer {
                title: Some("Pricing".to_owned()),
                blocks: vec![
                    paragraph("A", &[1]),
                    AnswerBlock::Item(1),
                    heading("Key assumptions"),
                    paragraph("B", &[2, 3, 1]),
                    AnswerBlock::Item(3),
                    paragraph("C", &[3]),
                    paragraph("D", &[4]),
                    AnswerBlock::Item(4),
                    heading("Limits"),
                    paragraph("E", &[2]),
                ],
                follow_ups: vec!["What is a put?".to_owned()],
            }
        );
    }
}
