//! Connects to a new graph the way every program does, on a throwaway graph, because only a real
//! FalkorDB shows the indexes it keeps and that it refuses an index it already has.

use std::collections::BTreeSet;

use graph::FalkorGraph;
use graph::testing::indexes;

use crate::support::throwaway_config;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local FalkorDB from docker compose and bills nothing; run with: cargo test -p graph --test integration -- --ignored indexes::"]
async fn two_programs_that_connect_to_a_new_graph_at_once_both_get_it_with_its_indexes() {
    let (_throwaway, config) = throwaway_config("indexes");

    // At the same moment, so that both can find an index missing and the second one to make it is
    // refused.
    let (first, second) =
        tokio::join!(FalkorGraph::connect(&config), FalkorGraph::connect(&config));
    let graph = first.expect("the first program should connect");
    second.expect("the second program should connect too");

    let expected = [
        ("Concept", "id"),
        ("Concept", "normalised_aliases"),
        ("Concept", "normalised_name"),
        ("Document", "id"),
        ("Item", "id"),
    ]
    .map(|(label, property)| (label.to_owned(), property.to_owned()));
    assert_eq!(indexes(&graph).await, BTreeSet::from(expected));
}
