//! Asks the item store for the items nearest to a question, and prints what it found.

use std::fmt;

use rag_core::{EmbedError, Embedder, ItemHit, ItemKind, ItemStore, StoreError};

#[derive(thiserror::Error, Debug)]
pub enum SearchError {
    #[error("could not embed the question")]
    Embed(#[from] EmbedError),

    #[error("could not search the stored items")]
    Store(#[from] StoreError),
}

/// Finds the stored items that are nearest to a question.
pub struct Retriever<E> {
    embedder: E,
    store: ItemStore,
}

impl<E: Embedder> Retriever<E> {
    pub fn new(embedder: E, store: ItemStore) -> Self {
        Self { embedder, store }
    }

    /// The `limit` items nearest to the question, nearest first. With a `kind`, only items of that
    /// kind are looked at.
    ///
    /// # Errors
    /// - [`SearchError::Embed`] when the question cannot be embedded
    /// - [`SearchError::Store`] when the store cannot be searched
    pub async fn search(
        &self,
        question: &str,
        kind: Option<ItemKind>,
        limit: usize,
    ) -> Result<SearchResults, SearchError> {
        let vector = self.embedder.embed_query(question).await?;
        let hits = self.store.search(vector, kind, limit).await?;
        Ok(SearchResults { hits })
    }
}

/// What a search found, nearest first. It prints as one block for each hit.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResults {
    pub hits: Vec<ItemHit>,
}

impl fmt::Display for SearchResults {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.hits.is_empty() {
            return formatter.write_str("no items found");
        }
        for (index, hit) in self.hits.iter().enumerate() {
            if index > 0 {
                formatter.write_str("\n\n")?;
            }
            write_hit(formatter, index + 1, hit)?;
        }
        Ok(())
    }
}

fn write_hit(formatter: &mut fmt::Formatter<'_>, number: usize, hit: &ItemHit) -> fmt::Result {
    let payload = &hit.payload;
    writeln!(formatter, "result {number}")?;
    writeln!(formatter, "document: {}", payload.doc_title)?;
    match &payload.printed_page {
        Some(printed) => writeln!(formatter, "page: {printed}")?,
        None => writeln!(
            formatter,
            "page: {} (position in the chapter, no printed number)",
            payload.page
        )?,
    }
    writeln!(formatter, "kind: {}", payload.kind.as_str())?;
    writeln!(formatter, "score: {:.3}", hit.score)?;
    if let Some(picture) = &payload.image_path {
        writeln!(formatter, "picture: {}", picture.display())?;
    }
    writeln!(formatter, "text:")?;
    formatter.write_str(&payload.text)
}
