//! The picture of one graph for one canvas, made once and kept: who is drawn, where it stands,
//! the view it opens in, and the words and widths the painter needs. Holds numbers and text,
//! never a laid-out galley.

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{self, Color32, FontId, Rect, Vec2};

use super::place::{Placed, place};
use super::plan::{Plan, is_concept, plan};
use super::seats::{Metrics, Sizes};
use super::view::View;
use crate::theme::{Kind, TextRole, Tone, color, radius, size, space};
use contract::{ConceptGraph, ConceptId, EdgeKind, GraphNode, NodeId};

/// These are the heights of a node with two lines of text and of a node with a single line.
const NODE_ROOMY: f32 = size::CONTROL_LG;
const NODE_COMPACT: f32 = size::CONTROL_SM;
/// A canvas this high or higher has room for three nodes of two lines, two bands and margins.
const ROOMY_FROM: f32 = 168.0;
/// This is the room between two nodes of a row.
const GAP: f32 = space::LG;
/// This is the room on each side of a link's label where it stands between the centre and the
/// node beside it.
const LABEL_PAD_ROOMY: f32 = space::MD;
const LABEL_PAD_COMPACT: f32 = space::SM;
/// The role of the words on a link, both when they are measured and when they are painted.
pub(super) const LINK_TEXT: TextRole = TextRole::Caption;
const BAND_MIN: f32 = LINK_TEXT.line_height();
const BAND_MAX: f32 = size::TAB;
const MARGIN: f32 = space::XS;
pub(super) const CORNER: f32 = radius::SM;
/// The most width a node's text may take, before it wraps or ends in "…".
const LABEL_MAX: f32 = 108.0;
const CENTRE_LABEL_MAX: f32 = 124.0;
/// The parts of a node besides its text, left to right: padding, the kind's icon, a gap, and
/// padding.
pub(super) const PAD_LEFT: f32 = space::SM;
const GLYPH_GAP: f32 = 6.0;
const PAD_RIGHT: f32 = 10.0;
/// Where the text of a node starts, measured from its left edge.
pub(super) const TEXT_LEFT: f32 = PAD_LEFT + size::GLYPH + GLYPH_GAP;
const CHROME: f32 = TEXT_LEFT + PAD_RIGHT;
/// How long an arrowhead is, and how wide at its base.
pub(super) const ARROW: f32 = 6.0;
/// The room between the end of a link and the outline of the node it reaches. It grows and
/// shrinks with the zoom, like the rest of the picture.
pub(super) const LINK_GAP: f32 = 3.0;
/// How much of its colour a node or link keeps while another node is lit.
pub(super) const DIM: f32 = 0.3;
/// Points of wheel scroll that double or halve the size.
pub(super) const SCROLL_PER_DOUBLING: f32 = 240.0;
pub(super) const LEGEND_HEIGHT: f32 = TextRole::Small.line_height();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Density {
    /// A node holds two rows of text.
    Roomy,
    /// A node holds one row of text.
    Compact,
}

impl Density {
    fn of(canvas: Vec2) -> Density {
        if canvas.y >= ROOMY_FROM {
            Density::Roomy
        } else {
            Density::Compact
        }
    }

    fn metrics(self) -> Metrics {
        let (node_height, label_pad) = match self {
            Density::Roomy => (NODE_ROOMY, LABEL_PAD_ROOMY),
            Density::Compact => (NODE_COMPACT, LABEL_PAD_COMPACT),
        };
        Metrics {
            node_height,
            gap: GAP,
            label_pad,
            band_min: BAND_MIN,
            band_max: BAND_MAX,
            margin: MARGIN,
            corner: CORNER,
            label_height: LINK_TEXT.line_height(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct TextOpts {
    pub is_centre: bool,
    pub density: Density,
    /// The size of the picture: 1.0 is full size.
    pub scale: f32,
    /// How much of its colour the text keeps: 1.0 is all of it.
    pub fade: f32,
    /// The first line of a selected node takes the rubric red, the colour of the thread from a
    /// claim to its source.
    pub is_selected: bool,
}

impl TextOpts {
    fn plain(&self, role: TextRole, colour: Color32) -> TextFormat {
        let mut format = role.format(colour.gamma_multiply(self.fade));
        format.font_id.size *= self.scale;
        format.line_height = format.line_height.map(|height| height * self.scale);
        format
    }
}

/// The text of a node: its name, and under it a result's detail when there is room.
#[derive(Debug)]
pub(super) struct NodeText {
    pub first: LayoutJob,
    pub second: Option<LayoutJob>,
}

#[derive(Clone, Copy)]
struct Limit {
    width: f32,
    rows: usize,
}

fn job(text: &str, format: TextFormat, limit: Limit) -> LayoutJob {
    let mut job = LayoutJob::single_section(text.to_owned(), format);
    job.wrap.max_width = limit.width;
    job.wrap.max_rows = limit.rows;
    job.wrap.break_anywhere = limit.rows == 1;
    job
}

/// The text of a node, for measuring (at scale 1) and for painting, so that the two cannot
/// drift apart.
pub(super) fn node_text(node: &GraphNode, opts: TextOpts) -> NodeText {
    let is_result = !is_concept(node.kind);
    let roomy = opts.density == Density::Roomy;
    let ink = if opts.is_selected {
        Tone::Rubric.swatch().text
    } else {
        color::TEXT
    };
    let name = opts.plain(TextRole::Label, ink);
    if is_result && roomy {
        let limit = Limit {
            width: LABEL_MAX * opts.scale,
            rows: 1,
        };
        let first = job(&node.label, name, limit);
        let second = node.detail.as_deref().map(|detail| {
            let format = opts.plain(TextRole::Small, color::TEXT_SECONDARY);
            job(detail, format, limit)
        });
        return NodeText { first, second };
    }
    let width = if opts.is_centre {
        CENTRE_LABEL_MAX
    } else {
        LABEL_MAX
    };
    let limit = Limit {
        width: width * opts.scale,
        rows: if roomy { 2 } else { 1 },
    };
    NodeText {
        first: job(&node.label, name, limit),
        second: None,
    }
}

/// The text is measured at full size and not selected. Selected text differs only in colour, so
/// it takes the same room.
fn node_widths(ui: &egui::Ui, nodes: &[&GraphNode], density: Density) -> Vec<f32> {
    nodes
        .iter()
        .enumerate()
        .map(|(place, node)| {
            let opts = TextOpts {
                is_centre: place == 0,
                density,
                scale: 1.0,
                fade: 1.0,
                is_selected: false,
            };
            let text = node_text(node, opts);
            let width = |job| ui.painter().layout_job(job).size().x;
            let second = text.second.map_or(0.0, width);
            (width(text.first).max(second) + CHROME).ceil()
        })
        .collect()
}

pub(super) fn scaled_font(role: TextRole, scale: f32) -> FontId {
    let mut font = role.font();
    font.size *= scale;
    font
}

/// What an edge of this kind says, read from its first end to its second: "A assumes B".
fn words(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::DerivedFrom => "derived from",
        EdgeKind::Assumes => "assumes",
        EdgeKind::Generalises => "generalises",
        EdgeKind::PartOf => "part of",
        EdgeKind::UsedFor => "used for",
        EdgeKind::Mentions => "mentions",
    }
}

fn link_words(kinds: &[EdgeKind]) -> String {
    let words: Vec<&str> = kinds.iter().map(|kind| words(*kind)).collect();
    words.join(", ")
}

fn label_widths(ui: &egui::Ui, link_words: &[String]) -> Vec<f32> {
    link_words
        .iter()
        .map(|words| {
            let galley = LINK_TEXT.galley(ui, words, color::TEXT_SECONDARY);
            galley.size().x.ceil()
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SceneKey {
    pub generation: u64,
    pub focus: Option<ConceptId>,
    /// The canvas is measured in whole points.
    pub canvas: [i32; 2],
    pub pixels_per_point: u32,
    /// How many nodes and edges the graph has, so that a graph that changes under one
    /// generation is never drawn with the places of another.
    pub graph_size: [usize; 2],
}

/// The words that say how many nodes the picture leaves out, made when the scene is.
#[derive(Debug)]
pub(super) struct NotDrawn {
    pub count: String,
    pub hover: String,
}

#[derive(Debug)]
pub(super) struct Scene {
    key: SceneKey,
    pub plan: Plan,
    pub placed: Placed,
    /// The canvas the picture was laid out for, in whole points. The view is opened, moved and
    /// kept by this size alone, so a view that was never moved stays equal to the opening view.
    pub canvas: Vec2,
    pub density: Density,
    pub opening: View,
    /// The words on each of `Plan::links`.
    pub link_words: Vec<String>,
    pub not_drawn: Option<NotDrawn>,
    /// The kinds that are drawn, in the order of the legend.
    pub kinds: Vec<Kind>,
}

const LEGEND_ORDER: [Kind; 5] = [
    Kind::Concept,
    Kind::RelatedConcept,
    Kind::Formula,
    Kind::Figure,
    Kind::Table,
];

/// The kinds that the nodes of the graph have, in the order of the legend. The scene draws at
/// most these, so the legend of a scene never needs more room than the legend of its graph.
pub(super) fn legend_kinds(graph: &ConceptGraph) -> impl Iterator<Item = Kind> {
    LEGEND_ORDER.into_iter().filter(|kind| {
        graph
            .nodes
            .iter()
            .any(|node| Kind::from(node.kind) == *kind)
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Refresh {
    /// The scene fitted the key already, or no scene can be made.
    Unchanged,
    /// A scene for another question or another centre.
    Replaced,
    /// The same picture, laid out for another size of canvas.
    Resized { was_open_at: View },
}

impl Refresh {
    pub(super) fn follow(self, view: View, scene: &Scene) -> View {
        match self {
            Refresh::Unchanged => view,
            Refresh::Replaced => scene.opening,
            Refresh::Resized { was_open_at } if view == was_open_at => scene.opening,
            Refresh::Resized { .. } => view.held(scene.placed.world, scene.canvas),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Picture<'a> {
    pub scene: &'a Scene,
    pub view: View,
}

impl Picture<'_> {
    pub(super) fn is_moved(&self) -> bool {
        self.view != self.scene.opening
    }
}

impl Scene {
    pub(super) fn refresh(
        slot: &mut Option<Scene>,
        ui: &egui::Ui,
        graph: &ConceptGraph,
        key: SceneKey,
    ) -> Refresh {
        if slot.as_ref().is_some_and(|scene| scene.key == key) {
            return Refresh::Unchanged;
        }
        let old = slot.take();
        *slot = Scene::build(ui, graph, key);
        match (old, slot.as_ref()) {
            (_, None) => Refresh::Unchanged,
            (Some(old), Some(new))
                if old.key.generation == new.key.generation && old.key.focus == new.key.focus =>
            {
                Refresh::Resized {
                    was_open_at: old.opening,
                }
            }
            _ => Refresh::Replaced,
        }
    }

    fn build(ui: &egui::Ui, graph: &ConceptGraph, key: SceneKey) -> Option<Scene> {
        let plan = plan(graph, key.focus)?;
        let canvas = egui::vec2(key.canvas[0] as f32, key.canvas[1] as f32);
        let density = Density::of(canvas);
        let nodes: Vec<&GraphNode> = plan
            .nodes
            .iter()
            .map(|index| &graph.nodes[*index])
            .collect();
        let node_widths = node_widths(ui, &nodes, density);
        let link_words: Vec<String> = plan
            .links
            .iter()
            .map(|link| link_words(&link.kinds))
            .collect();
        let label_widths = label_widths(ui, &link_words);
        let sizes = Sizes {
            nodes: &node_widths,
            labels: &label_widths,
            canvas,
        };
        let placed = place(&plan, &sizes, &density.metrics());
        let opening = View::opening(placed.world, canvas, placed.nodes[0].center());
        let not_drawn = (plan.left_out > 0).then(|| NotDrawn {
            count: format!("+{} not drawn", plan.left_out),
            hover: format!("The {} least connected nodes are left out.", plan.left_out),
        });
        let drawn = |kind: &Kind| nodes.iter().any(|node| Kind::from(node.kind) == *kind);
        let kinds = LEGEND_ORDER.into_iter().filter(drawn).collect();
        Some(Scene {
            key,
            plan,
            placed,
            canvas,
            density,
            opening,
            link_words,
            not_drawn,
            kinds,
        })
    }

    pub(super) fn rect_of(&self, graph: &ConceptGraph, id: NodeId) -> Option<Rect> {
        let place = self
            .plan
            .nodes
            .iter()
            .position(|index| graph.nodes[*index].id == id)?;
        Some(self.placed.nodes[place])
    }
}
