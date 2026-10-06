# quanty

A multimodal RAG layer with a knowledge graph, built on top of `ocr`.

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
| `ocr` | PDF to text, LaTeX, figures and figure descriptions |
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

## Converting a chapter

```bash
cargo run -p ocr -- --book "Option Volatility and Pricing" chapter-1-sample-pages.pdf
```

The file must be named `chapter-<number>-<name>.pdf`. Every page is broken into its pieces (headings, text, formulas, figures, tables and footnotes) and saved under `content/<book-title>/chapter-<number>/page-num-<i>/`, with a `page.json` for each page and a `chapter.json` for the chapter. Each figure also gets its own picture, `NN-figure.png`, cut out of the page beside its `.md` file, and a figure that cannot be cut out keeps the whole page as its picture and is listed in the summary. `--out <folder>` saves somewhere other than `content`. The command only saves files: nothing is put into Qdrant or FalkorDB.

The command reads `CONVERTER_JEV_API_KEY` from `.env`, and it refuses to run while `ANTHROPIC_API_KEY` is set, so the work is billed to the `claude` subscription. A second run on a finished chapter makes no outside call and costs nothing. An interrupted run picks up at the pages that are missing. A different PDF for the same chapter is refused.

Later crates read a chapter back, in reading order, with `ocr::read_chapter`.
