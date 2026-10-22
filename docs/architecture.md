# Architecture

The whole system on one page: how a PDF becomes items, concepts and points, and how a question comes back as an answer with citations.

<p align="center">
  <img src="img/architecture.webp" width="880" alt="The architecture of quanty: the three front doors, the write path that ingests one PDF, the shared state in content/, FalkorDB and Qdrant, the read path that answers a question, the shared crates, the outside services and the cargo dependencies">
</p>

What each crate is, and how a search works step by step, is in [reference.md](reference.md#how-it-works). The folders of the repository:

| Folder      | What it holds                                                                                                                                                                   |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/`   | Ten crates: `ocr`, `rag-core`, `choices`, `graph`, `rag-ingestion`, `rag-retrieval`, `rag-dataset`, `export`, `gui` and `mcp`                                                   |
| `docs/`     | [guide.md](guide.md), how to use quanty, [reference.md](reference.md), every command in full, [mcp.md](mcp.md), the MCP server in full, [server.md](server.md), how to keep the stores on a server, [training.md](training.md), how to train Gemma 4 on a dataset, [troubleshooting.md](troubleshooting.md) and this page |
| `samples/`  | The sample PDF and converted sample chapters                                                                                                                                    |
| `content/`  | The converted documents, in one folder for each media. Made on your machine, not in git                                                                                         |
| `data/`     | The stores and the kept answers. Made on your machine, not in git                                                                                                               |
| `datasets/` | The built training datasets (`DATASET_DIR`). Made on your machine, not in git                                                                                                   |
