# Troubleshooting

| What you see | What to do |
| --- | --- |
| The login page keeps asking for a password | Use `QUANTY_PASSWORD` from the server's `.env`, then restart quanty if you changed it. Check that the browser accepts cookies. The MCP token is a separate credential. |
| Login returns 429 | Five failed passwords within a minute trigger a one-minute pause. Wait before trying again. |
| A reload signs you out after every restart | Set a persistent `QUANTY_SESSION_KEY` to 64 hex characters (`openssl rand -hex 32`) and restart quanty. Changing it invalidates existing sessions. |
| `not connected to the server`, **Reconnecting** or **Lost** | Check the quanty unit and Caddy logs. Caddy must proxy WebSocket upgrades to the same server. The browser retries at 1, 2, 4, 8 and then 15-second intervals. Retry your command once connected; the catalogue and health refresh automatically. |
| `The UI bundle is not built` with HTTP 503 | Run `trunk build --release` in `crates/web-ui`, then rebuild `cargo build --release -p server` and reinstall/restart quanty. The UI is embedded during the server build. |
| HTTP 413 when choosing a PDF | The file exceeds `QUANTY_UPLOAD_LIMIT_MB` or Caddy's `request_body max_size`. The default server limit is 1024 MiB, and the supplied Caddyfile caps requests at `1GB`. Raise both deliberately if larger uploads are needed, then restart/reload the services. |
| Upload is refused as a non-PDF | Choose a real PDF whose bytes start with `%PDF-`; renaming another file does not convert it. |
| An export link no longer downloads | Each export is consumed once. Export again for a fresh link. Startup cleanup removes exports older than a day. |
| An ingest or dataset build is already running | Wait for the current server job. Closing a tab leaves it running; reloading does not reattach to progress. A second browser tab cannot start another paid job. |
| `/mcp` returns 404 | Set `QUANTY_MCP_TOKEN` on the quanty server and restart it. |
| `/mcp` returns 401 | Supply `Authorization: Bearer <QUANTY_MCP_TOKEN>` from the same server. A browser password cookie does not authorize MCP. |
| `could not reach Qdrant at …` or `could not connect to FalkorDB at …` | On Ubuntu, inspect `systemctl status qdrant redis-server` and their journals. On a Mac, run `docker compose up -d`. Check that `QDRANT_URL` names gRPC port 6334 and that the API key/password match the stores. |
| Qdrant's unit fails its pre-start check | Set a nonempty `QDRANT_API_KEY` in `/srv/quanty/.env`. The unit refuses to start Qdrant without authentication. |
| Redis cannot load FalkorDB or write its files | Use the supported Ubuntu 24.04 LTS x86_64 host; Ubuntu 22.04 cannot load the pinned prebuilt module. Check Redis 8 or newer, `libgomp1`, `/opt/falkordb/falkordb.so`, the included configuration, `redis` ownership of `data/falkordb`, directory traversal permissions and the systemd writable-path override. See [server.md](server.md#redis-8-and-falkordb). |
| `did not answer like FalkorDB; another program may be using that port` | Check that port 6379 belongs to Redis with the FalkorDB `graph` module. `redis-cli MODULE LIST` must list it; ordinary Redis alone cannot answer graph queries. |
| The Library is empty after a move | Check the new server's `.env`, graph name and all three restored library folders. Browser tabs all see the one library of their server. Restart quanty after changing settings. |
| `EMBEDDING_GEMINI_API_KEY is not set` or `CONVERTER_JEV_API_KEY is not set` | Add the key to the server home folder's `.env` and restart quanty. For command-line tools, start them in that home folder. |
| A model CLI is signed out on the server | Sign in as the account from `quanty.service`, not your administrative account. Use `codex login --device-auth` and `codex login status`, or `claude setup-token` and `CLAUDE_CODE_OAUTH_TOKEN` in the service environment. Check the unit's `PATH`; [server.md](server.md#3-model-tools-and-headless-sign-in) covers credential files. |
| `ANTHROPIC_API_KEY is set, so claude could bill the API` | Remove that variable from the service environment and `.env`, then restart quanty. Use the Claude subscription sign-in. |
| `The page was not found` or a content image is refused | Check that converted files are under `CONTENT_DIR` on the server and readable by its account. Picture requests allow only PNG/JPEG inside that folder; paths outside it are refused. |
| `… cannot be ingested as a book chapter; rename it to chapter-<number>-<name>.pdf …` | Use that file name for a numbered chapter, or choose **Other part** and give a title. A paper or another media can use any PDF name. |

For Ubuntu service failures, start with:

```bash
sudo systemctl status quanty qdrant redis-server caddy
sudo journalctl -u quanty -u qdrant -u redis-server -u caddy -n 100 --no-pager
```

`GET /healthz` returning `ok` proves that quanty's HTTP process is running. Use the app's health check, MCP `health`, or `rag-ingest health` to check model tools, keys and stores.
