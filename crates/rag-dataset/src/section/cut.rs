//! Cuts the blocks of a chapter along its headings into sections that are neither too short to
//! teach anything nor too long to train on whole.

use std::collections::HashMap;

use super::render::Block;

/// A section shorter than this is joined to a neighbour that shares its heading.
const MIN_TOKENS: usize = 500;
/// A section longer than this is cut in parts at the edges of blocks.
// SMELL: only the blocks are counted, so a part with its heading line and the blank lines between
// its blocks can come out a little over this.
const MAX_TOKENS: usize = 8_000;

/// The blocks that become one section, or several parts of one section when they are too long.
pub(super) struct Unit {
    /// The headings the unit sits under, by number, outermost first. Empty for the whole chapter.
    pub path: Vec<usize>,
    /// The numbers of the blocks of each part, in the order they are written.
    pub parts: Vec<Vec<usize>>,
}

/// A heading and what is written directly under it.
struct Node {
    path: Vec<usize>,
    /// The blocks that sit under exactly this heading, in reading order.
    own: Vec<usize>,
    children: Vec<usize>,
}

/// The blocks of one section, before a long one is cut in parts.
struct Run {
    path: Vec<usize>,
    blocks: Vec<usize>,
    tokens: usize,
}

pub(super) fn cut(blocks: &[Block]) -> Vec<Unit> {
    let nodes = tree_of(blocks);
    let mut runs = Vec::new();
    cut_node(&nodes, 0, blocks, &mut runs);
    merge_short_runs(&mut runs);
    runs.into_iter()
        .map(|run| Unit {
            parts: split(&run.blocks, blocks)
                .into_iter()
                .map(|part| lone_footnotes_last(part, blocks))
                .collect(),
            path: run.path,
        })
        .collect()
}

/// One node for each heading, and a root with no heading for what comes before the first one.
fn tree_of(blocks: &[Block]) -> Vec<Node> {
    let mut nodes = vec![Node {
        path: Vec::new(),
        own: Vec::new(),
        children: Vec::new(),
    }];
    let mut index_of: HashMap<&[usize], usize> = HashMap::from([(&[][..], 0)]);
    for (position, block) in blocks.iter().enumerate() {
        // A heading comes before everything under it, so the parent of each new path is already
        // in the map.
        for depth in 1..=block.path.len() {
            let path = &block.path[..depth];
            if !index_of.contains_key(path) {
                let parent = index_of[&block.path[..depth - 1]];
                let node = nodes.len();
                index_of.insert(path, node);
                nodes[parent].children.push(node);
                nodes.push(Node {
                    path: path.to_vec(),
                    own: Vec::new(),
                    children: Vec::new(),
                });
            }
        }
        nodes[index_of[block.path.as_slice()]].own.push(position);
    }
    nodes
}

/// A node that fits in one section is one. A node that does not gives what sits directly under
/// its heading as one section, and then cuts each heading under it.
fn cut_node(nodes: &[Node], index: usize, blocks: &[Block], runs: &mut Vec<Run>) {
    let node = &nodes[index];
    let subtree: Vec<usize> = (0..blocks.len())
        .filter(|&position| blocks[position].path.starts_with(&node.path))
        .collect();
    if tokens_of(&subtree, blocks) <= MAX_TOKENS {
        push_run(runs, node.path.clone(), subtree, blocks);
        return;
    }
    push_run(runs, node.path.clone(), node.own.clone(), blocks);
    for &child in &node.children {
        cut_node(nodes, child, blocks, runs);
    }
}

fn push_run(runs: &mut Vec<Run>, path: Vec<usize>, positions: Vec<usize>, blocks: &[Block]) {
    if positions.is_empty() {
        return;
    }
    runs.push(Run {
        path,
        tokens: tokens_of(&positions, blocks),
        blocks: positions,
    });
}

fn tokens_of(positions: &[usize], blocks: &[Block]) -> usize {
    positions
        .iter()
        .map(|&position| blocks[position].tokens)
        .sum()
}

/// One pass from the first run to the last. A short run is joined to the next run when that one
/// sits under the same parent heading, and otherwise to the one before it. The joined run sits
/// under the headings they share, and is joined again if it is still short.
fn merge_short_runs(runs: &mut Vec<Run>) {
    let mut at = 0;
    while at < runs.len() {
        if runs[at].tokens >= MIN_TOKENS {
            at += 1;
            continue;
        }
        let path = &runs[at].path;
        let parent = &path[..path.len().saturating_sub(1)];
        if runs
            .get(at + 1)
            .is_some_and(|next| next.path.starts_with(parent))
        {
            let next = runs.remove(at + 1);
            runs[at] = joined(&runs[at], next);
        } else if at > 0 && runs[at - 1].path.starts_with(parent) {
            let short = runs.remove(at);
            runs[at - 1] = joined(&runs[at - 1], short);
        } else {
            at += 1;
        }
    }
}

fn joined(first: &Run, second: Run) -> Run {
    let shared = first
        .path
        .iter()
        .zip(&second.path)
        .take_while(|(one, other)| one == other)
        .count();
    Run {
        path: first.path[..shared].to_vec(),
        blocks: first.blocks.iter().copied().chain(second.blocks).collect(),
        tokens: first.tokens + second.tokens,
    }
}

/// A run over the limit is cut into parts at the edges of blocks. A text that leads into a
/// formula goes with it, and a block that is over the limit alone is a part by itself.
fn split(run_blocks: &[usize], blocks: &[Block]) -> Vec<Vec<usize>> {
    let mut parts: Vec<Vec<usize>> = Vec::new();
    let mut tokens_in_part = 0;
    for group in groups_that_stay_together(run_blocks, blocks) {
        let group_tokens = tokens_of(&group, blocks);
        let fits = tokens_in_part + group_tokens <= MAX_TOKENS;
        match parts.last_mut() {
            Some(part) if fits => {
                part.extend(group);
                tokens_in_part += group_tokens;
            }
            _ => {
                parts.push(group);
                tokens_in_part = group_tokens;
            }
        }
    }
    parts
}

fn groups_that_stay_together(run_blocks: &[usize], blocks: &[Block]) -> Vec<Vec<usize>> {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut previous_keeps_with_next = false;
    for &position in run_blocks {
        match groups.last_mut() {
            Some(group) if previous_keeps_with_next => group.push(position),
            _ => groups.push(vec![position]),
        }
        previous_keeps_with_next = blocks[position].keeps_with_next;
    }
    groups
}

fn lone_footnotes_last(part: Vec<usize>, blocks: &[Block]) -> Vec<usize> {
    let (lone, others): (Vec<usize>, Vec<usize>) = part
        .into_iter()
        .partition(|&position| blocks[position].is_lone_footnote);
    others.into_iter().chain(lone).collect()
}
