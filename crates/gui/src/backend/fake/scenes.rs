//! The scenes the fake backend can play, and what each one is for.

use crate::contract::{FailureKind, Service};

/// One state the whole app can be shown in, from the name given to `--fixture`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Scene {
    pub name: &'static str,
    pub about: &'static str,
    /// True when the app settles and asks for no repaint once it has opened, and settles again
    /// after any of its controls is pressed. The tests that press every control, or that wait for
    /// rest, take only these scenes.
    pub rests: bool,
    pub(super) script: Script,
}

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
    pub(super) ingest: Ingest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Search {
    Found,
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

/// How a started ingest ends. Nothing is stored either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Ingest {
    Done,
    Fails(FailureKind),
    /// The ingest plays to page 3 of 12 and never ends.
    Stalls,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Opening {
    Nothing,
    Asks,
    AsksForResultsOnly,
    AsksWithALabelNoDocumentHas,
    /// Asks, and opens page 5 of the third sample chapter, which holds a figure.
    AsksAndOpensASource,
    ChecksAChapter,
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
    ingest: Ingest::Done,
};

static SCENES: [Scene; 20] = [
    Scene {
        name: "idle",
        about: "Healthy, with the three sample chapters and nothing asked.",
        rests: true,
        script: Script {
            opening: Opening::Nothing,
            ..HEALTHY
        },
    },
    Scene {
        name: "black-scholes",
        about: "A question asked: the results, then the graph, then the answer.",
        rests: true,
        script: HEALTHY,
    },
    Scene {
        name: "searching",
        about: "A question asked and the search never comes back.",
        rests: false,
        script: Script {
            search: Search::Never,
            ..HEALTHY
        },
    },
    Scene {
        name: "answering",
        about: "The results and the graph are shown and the answer never comes.",
        rests: false,
        script: Script {
            answer: Answer::Never,
            ..HEALTHY
        },
    },
    Scene {
        name: "no-sources",
        about: "A question asked and the search finds nothing.",
        rests: true,
        script: Script {
            search: Search::Nothing,
            ..HEALTHY
        },
    },
    Scene {
        name: "no-answer",
        about: "Results found, and an answer that says they do not answer the question.",
        rests: true,
        script: Script {
            answer: Answer::NoBlock,
            ..HEALTHY
        },
    },
    Scene {
        name: "search-failed",
        about: "The search fails because the embedding service does not answer.",
        rests: true,
        script: Script {
            search: Search::Fails(FailureKind::EmbeddingFailed),
            ..HEALTHY
        },
    },
    Scene {
        name: "answer-failed",
        about: "The results are shown and the answer fails on the usage limit of claude.",
        rests: true,
        script: Script {
            answer: Answer::Fails(FailureKind::ClaudeUsageLimit),
            ..HEALTHY
        },
    },
    Scene {
        name: "stores-down",
        about: "Qdrant and FalkorDB are down: the catalogue and every search fail, pages still load.",
        rests: true,
        script: Script {
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
    },
    Scene {
        name: "first-run",
        about: "No settings file: an empty library, and every search says the embedding key is missing.",
        rests: true,
        script: Script {
            search: Search::Fails(FailureKind::EmbeddingKeyMissing),
            library: Library::Empty,
            down: &[
                (Service::EmbeddingKey, FailureKind::EmbeddingKeyMissing),
                (Service::ConverterKey, FailureKind::ConverterKeyMissing),
            ],
            opening: Opening::Nothing,
            ..HEALTHY
        },
    },
    Scene {
        name: "empty-library",
        about: "Healthy, with no document stored: a question finds nothing.",
        rests: true,
        script: Script {
            search: Search::Nothing,
            library: Library::Empty,
            opening: Opening::Nothing,
            ..HEALTHY
        },
    },
    Scene {
        name: "no-labels",
        about: "A question asked with a label that no document carries.",
        rests: true,
        script: Script {
            opening: Opening::AsksWithALabelNoDocumentHas,
            ..HEALTHY
        },
    },
    Scene {
        name: "source-missing",
        about: "The results are shown and the chapter's files cannot be found.",
        rests: true,
        script: Script {
            pages: Pages::SourceMissing,
            concepts: Concepts::Empty,
            opening: Opening::AsksAndOpensASource,
            ..HEALTHY
        },
    },
    Scene {
        name: "graph-failed",
        about: "The results and the answer are shown and the concept graph fails.",
        rests: true,
        script: Script {
            graph: Graph::Fails(FailureKind::FalkorDbDown),
            down: &[(Service::FalkorDb, FailureKind::FalkorDbDown)],
            ..HEALTHY
        },
    },
    Scene {
        name: "no-concepts",
        about: "Results found, with no concept in the graph to follow.",
        rests: true,
        script: Script {
            search: Search::FoundWithoutConcepts,
            graph: Graph::Empty,
            ..HEALTHY
        },
    },
    Scene {
        name: "results-only",
        about: "A question asked for its results alone: no answer is written.",
        rests: true,
        script: Script {
            opening: Opening::AsksForResultsOnly,
            ..HEALTHY
        },
    },
    Scene {
        name: "ingest-ready",
        about: "The Ingest tab, with a chapter checked and ready to start.",
        rests: true,
        script: Script {
            opening: Opening::ChecksAChapter,
            ..HEALTHY
        },
    },
    Scene {
        name: "ingest-failed",
        about: "The Ingest tab, with a chapter checked and a start that fails at a page.",
        rests: true,
        script: Script {
            ingest: Ingest::Fails(FailureKind::PageFailed),
            opening: Opening::ChecksAChapter,
            ..HEALTHY
        },
    },
    Scene {
        name: "ingest-running",
        about: "The Ingest tab, with a chapter checked and a start that stops at page 3 of 12 and never ends.",
        rests: false,
        script: Script {
            ingest: Ingest::Stalls,
            opening: Opening::ChecksAChapter,
            ..HEALTHY
        },
    },
    Scene {
        name: "gallery",
        about: "The widget kit in every state, in the whole window.",
        rests: false,
        script: Script {
            opening: Opening::Nothing,
            ..HEALTHY
        },
    },
];

/// Every scene, in the order `--fixture list` prints them.
pub fn scenes() -> &'static [Scene] {
    &SCENES
}
