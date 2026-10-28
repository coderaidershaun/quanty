# Architecture

The browser runs the egui UI as WebAssembly. One native `quanty` server serves its bundle, login, WebSocket API, uploads, downloads, pictures and MCP. Caddy terminates HTTPS and proxies to quanty on loopback; quanty runs beside Qdrant, Redis 8 with FalkorDB, the content folder and the signed-in model CLIs.

<p align="center">
  <img src="img/architecture.webp" width="880" alt="Earlier quanty architecture: ingestion, retrieval, shared stores and outside services">
</p>

`img/architecture.webp` predates the server architecture. The owner will replace it.

| Crate | Kind and target | Holds |
| --- | --- | --- |
| `contract` | Library, native and wasm | Serializable commands, events, startup facts and WebSocket frames |
| `backend` | Library, native | Live and fixture handlers, request runtime and exports |
| `gui` | Library, wasm and native tests | UI panels, state, media, widgets, transport interface and test helpers |
| `web-ui` | Binary, wasm | WebRunner entry, WebSocket transport, HTTP picture fetching and PDF upload |
| `server` | Binary `quanty`, native | HTTP routes, login and cookies, job guard, files, embedded UI bundle and remote MCP |
| `mcp` | Library and `quanty-mcp` binary, native | Agent tools, HTTP router for quanty and standalone stdio |
| `choices` | Library, native and wasm | Model choices and API key types |
| `export` | Library, native and wasm | Documents and self-contained PDF, Markdown and HTML output |
| `rag-core` | Library, native | Shared types, configuration, model interfaces and Qdrant access |
| `graph` | Library, native | FalkorDB graph access |
| `ocr` | Library and binary, native | PDF conversion to saved pages and artifacts |
| `rag-ingestion` | Library and `rag-ingest`, native | Ingestion, concept extraction, insights and library edits |
| `rag-retrieval` | Library and `rag-query`, native | Vector search, graph expansion, ranking and cited answers |
| `rag-dataset` | Library and binary, native | Training datasets and saved model work |

| Folder | Holds |
| --- | --- |
| `deploy/` | Ubuntu installer, systemd units, store configuration and Caddyfile |
| `docs/` | User guide, reference, MCP, server setup, training and troubleshooting |
| `samples/` | Sample PDF and converted fixture chapters |
| `skills/` | Agent skill files that teach an agent to use quanty |
| `content/` | Converted documents, pictures, source uploads and insights |
| `data/` | Stores, kept answers, temporary uploads and exports |
| `datasets/` | Built training datasets |

The server owns one library through its `.env`. The browser uses server-relative picture URLs and uploaded PDF ids, and exports become browser downloads. Paid ingest and dataset jobs survive a closed tab; one runs at a time. [reference.md](reference.md#the-server-and-the-app) lists the routes, settings and wire format; [server.md](server.md) describes deployment and backups.
