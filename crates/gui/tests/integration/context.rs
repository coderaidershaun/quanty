//! Checks how the live backend keeps its stores and what it answers when they are down.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use graph::FalkorGraph;
use gui::backend::live::{LiveContext, RealServices, Services};
use gui::contract::{Failure, FailureKind};
use ocr::ConvertError;
use ocr::convert::services::LiveServices;
use rag_core::{ClaudeCli, GeminiEmbedder};

use super::support;

/// The real services, which also count how many times the graph was asked for.
struct CountingServices {
    graph_calls: Arc<AtomicUsize>,
}

impl Services for CountingServices {
    type Embedder = GeminiEmbedder;
    type Llm = ClaudeCli;
    type Graph = FalkorGraph;
    type Pages = LiveServices;

    fn embedder(&self, config: &rag_core::Config) -> Result<GeminiEmbedder, Failure> {
        RealServices.embedder(config)
    }

    fn llm(&self, model: &str) -> ClaudeCli {
        RealServices.llm(model)
    }

    async fn graph(&self, config: &rag_core::Config) -> Result<FalkorGraph, Failure> {
        self.graph_calls.fetch_add(1, Ordering::SeqCst);
        RealServices.graph(config).await
    }

    async fn page_services(
        &self,
        jev_api_key: Option<&str>,
    ) -> Result<Arc<LiveServices>, ConvertError> {
        RealServices.page_services(jev_api_key).await
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_context_opens_nothing_until_asked_and_tries_again_after_a_store_failure() {
    let graph_calls = Arc::new(AtomicUsize::new(0));
    let context = LiveContext::new(
        support::closed_ports_config(),
        CountingServices {
            graph_calls: Arc::clone(&graph_calls),
        },
    );
    assert_eq!(
        graph_calls.load(Ordering::SeqCst),
        0,
        "a new context opens nothing"
    );

    // Nothing listens on port 1, so the graph refuses. The failure names the store and its address.
    let first = context
        .retriever()
        .await
        .err()
        .expect("nothing listens on that port");
    assert_eq!(first.kind, FailureKind::FalkorDbDown);
    assert!(first.hint.contains("falkor://127.0.0.1:1"), "{first:?}");
    assert_eq!(graph_calls.load(Ordering::SeqCst), 1);

    // A failed connection is not kept: the next command connects again, and so does a read of
    // the graph alone.
    let second = context
        .retriever()
        .await
        .err()
        .expect("nothing listens on that port");
    assert_eq!(second.kind, FailureKind::FalkorDbDown);
    assert_eq!(graph_calls.load(Ordering::SeqCst), 2);
    let alone = context
        .graph()
        .await
        .err()
        .expect("nothing listens on that port");
    assert_eq!(alone.kind, FailureKind::FalkorDbDown);
    assert_eq!(graph_calls.load(Ordering::SeqCst), 3);
}
