//! The scenes the fake backend can play, and what each one is for.

use crate::contract::{FailureKind, Service};

/// One state the whole app can be shown in, from the name given to `--fixture`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Scene {
    pub name: &'static str,
    pub about: &'static str,
    /// True when the app settles and asks for no repaint once it has opened.
    pub rests: bool,
    pub(super) script: Script,
}

/// How a scene plays each command. What a scene does not name is played from the samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct Script {
    pub(super) search: Search,
    pub(super) graph: Graph,
    pub(super) answer: Answer,
    pub(super) library: Library,
    pub(super) pages: Pages,
    pub(super) concepts: Concepts,
    /// The services a health check finds down, and why.
    pub(super) down: &'static [(Service, FailureKind)],
    pub(super) opening: Opening,
}

/// What the search of an ask comes back with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Search {
    Found,
    /// Found, but with no concept in the graph to follow.
    FoundWithoutConcepts,
    Nothing,
    Fails(FailureKind),
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Graph {
    Drawn,
    Empty,
    Fails(FailureKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Answer {
    Written,
    /// An answer with no block: the results do not answer the question.
    NoBlock,
    Fails(FailureKind),
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Library {
    Samples,
    Empty,
    Fails(FailureKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Pages {
    Samples,
    SourceMissing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Concepts {
    Samples,
    Empty,
    Fails(FailureKind),
}

/// What the scene does when the window opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Opening {
    Nothing,
    Asks,
    AsksForResultsOnly,
    AsksWithALabelNoDocumentHas,
    /// Asks, and opens page 5 of the third sample chapter, which holds a figure.
    AsksAndOpensASource,
}

const HEALTHY: Script = Script {
    search: Search::Found,
    graph: Graph::Drawn,
    answer: Answer::Written,
    library: Library::Samples,
    pages: Pages::Samples,
    concepts: Concepts::Samples,
    down: &[],
    opening: Opening::Asks,
};

const fn scene(name: &'static str, about: &'static str, rests: bool, script: Script) -> Scene {
    Scene {
        name,
        about,
        rests,
        script,
    }
}

static SCENES: [Scene; 17] = [
    scene(
        "idle",
        "Healthy, with the three sample chapters and nothing asked.",
        true,
        Script {
            opening: Opening::Nothing,
            ..HEALTHY
        },
    ),
    scene(
        "black-scholes",
        "A question asked: the results, then the graph, then the answer.",
        true,
        HEALTHY,
    ),
    scene(
        "searching",
        "A question asked and the search never comes back.",
        false,
        Script {
            search: Search::Never,
            ..HEALTHY
        },
    ),
    scene(
        "answering",
        "The results and the graph are shown and the answer never comes.",
        false,
        Script {
            answer: Answer::Never,
            ..HEALTHY
        },
    ),
    scene(
        "no-sources",
        "A question asked and the search finds nothing.",
        true,
        Script {
            search: Search::Nothing,
            ..HEALTHY
        },
    ),
    scene(
        "no-answer",
        "Results found, and an answer that says they do not answer the question.",
        true,
        Script {
            answer: Answer::NoBlock,
            ..HEALTHY
        },
    ),
    scene(
        "search-failed",
        "The search fails because the embedding service does not answer.",
        true,
        Script {
            search: Search::Fails(FailureKind::EmbeddingFailed),
            ..HEALTHY
        },
    ),
    scene(
        "answer-failed",
        "The results are shown and the answer fails on the usage limit of claude.",
        true,
        Script {
            answer: Answer::Fails(FailureKind::ClaudeUsageLimit),
            ..HEALTHY
        },
    ),
    scene(
        "stores-down",
        "Qdrant and FalkorDB are down: the catalogue and every search fail, pages still load.",
        true,
        Script {
            search: Search::Fails(FailureKind::QdrantDown),
            library: Library::Fails(FailureKind::QdrantDown),
            concepts: Concepts::Fails(FailureKind::FalkorDbDown),
            down: &[
                (Service::Qdrant, FailureKind::QdrantDown),
                (Service::FalkorDb, FailureKind::FalkorDbDown),
            ],
            opening: Opening::AsksAndOpensASource,
            ..HEALTHY
        },
    ),
    scene(
        "first-run",
        "No settings file: an empty library, and every search says the embedding key is missing.",
        true,
        Script {
            search: Search::Fails(FailureKind::EmbeddingKeyMissing),
            library: Library::Empty,
            down: &[
                (Service::EmbeddingKey, FailureKind::EmbeddingKeyMissing),
                (Service::ConverterKey, FailureKind::ConverterKeyMissing),
            ],
            opening: Opening::Nothing,
            ..HEALTHY
        },
    ),
    scene(
        "empty-library",
        "Healthy, with no document stored: a question finds nothing.",
        true,
        Script {
            search: Search::Nothing,
            library: Library::Empty,
            opening: Opening::Nothing,
            ..HEALTHY
        },
    ),
    scene(
        "no-labels",
        "A question asked with a label that no document carries.",
        true,
        Script {
            opening: Opening::AsksWithALabelNoDocumentHas,
            ..HEALTHY
        },
    ),
    scene(
        "source-missing",
        "The results are shown and the chapter's files cannot be found.",
        true,
        Script {
            pages: Pages::SourceMissing,
            concepts: Concepts::Empty,
            opening: Opening::AsksAndOpensASource,
            ..HEALTHY
        },
    ),
    scene(
        "graph-failed",
        "The results and the answer are shown and the concept graph fails.",
        true,
        Script {
            graph: Graph::Fails(FailureKind::FalkorDbDown),
            down: &[(Service::FalkorDb, FailureKind::FalkorDbDown)],
            ..HEALTHY
        },
    ),
    scene(
        "no-concepts",
        "Results found, with no concept in the graph to follow.",
        true,
        Script {
            search: Search::FoundWithoutConcepts,
            graph: Graph::Empty,
            ..HEALTHY
        },
    ),
    scene(
        "results-only",
        "A question asked for its results alone: no answer is written.",
        true,
        Script {
            opening: Opening::AsksForResultsOnly,
            ..HEALTHY
        },
    ),
    scene(
        "gallery",
        "The widget kit in every state, in the whole window.",
        false,
        Script {
            opening: Opening::Nothing,
            ..HEALTHY
        },
    ),
];

/// Every scene, in the order `--fixture list` prints them.
pub fn scenes() -> &'static [Scene] {
    &SCENES
}
