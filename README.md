# quanty

A multimodal RAG layer with a knowledge graph, built on top of `converter`.

PDFs are turned into searchable items (text, LaTeX formulas and figures). An LLM extracts the concepts each item mentions. Concepts are global, so two PDFs that both discuss Black–Scholes attach to the same node, and every new PDF links itself to everything already ingested with no manual work.

## How it works

- **Qdrant** stores every vector (items and concepts).
- **FalkorDB** stores the graph: documents, items and concepts, joined by `HAS_ITEM`, `NEXT`, `MENTIONS` and `RELATES_TO` edges.
- **Gemini Embedding 2** embeds text, LaTeX and images into one vector space.
- **Claude** (via `claude -p`) extracts and resolves concepts.
- Retrieval finds the closest items by vector search, then follows concepts across documents to pull in related material.

## Crates

| Crate | Purpose |
| --- | --- |
| `converter` | PDF to text, LaTeX, figures and figure descriptions |
| `rag-core` | Shared types, IDs, config, `Embedder` and `Llm` traits, Qdrant wrapper |
| `graph` | `GraphStore` trait and FalkorDB implementation |
| `rag-ingestion` | Chunking, embedding, concept extraction and resolution (`rag-ingest`) |
| `rag-retrieval` | Vector search, graph expansion, ranking (`rag-query`) |

## Getting started

```bash
cp .env.example .env   # add your API keys
docker compose up -d   # Qdrant + FalkorDB
```

The `claude` CLI must be on your `PATH` and signed in.
