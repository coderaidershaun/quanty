# Costs

Six tools are paid. Their description says PAID as the second word, and `catalog` shows `paid: true`.

| Tool | Who pays | Free when |
| --- | --- | --- |
| `search` | One Gemini embedding of the question: a fraction of a cent | never |
| `answer` | One Gemini embedding and the subscription usage of the Ask agent; a minute or two | never |
| `find_concepts` | One Gemini embedding for `query` | `name` is used |
| `ingest_pdf` | The Ingest agent, Jev and Gemini; minutes per chapter | the PDF is already ingested with insights |
| `write_insights` | The Ingest agent, a minute or more per document | `dry_run: true` |
| `build_dataset` | The Datasets agent; can take hours | `dry_run: true` |

Everything else is free: reading, listing, the graph tools, `similar_items`, `check_pdf`, the job tools, `edit_media`, `edit_document_tags`, `export_insights` and `health`.

## Before you pay

- `check_pdf` before `ingest_pdf`: pages, state and a cost range.
- `dry_run: true` before `write_insights` and `build_dataset`: it lists what would be asked and costs nothing.
- `search` before `answer`, and write the answer yourself when you can.
- A paid call returns `usage` and `cost_usd`; `cost_usd` is about the US dollars at API prices, or null when a model has no price.

## Not paid twice

Converted pages, kept answers of a dataset build and saved insights are kept. After a failure or a stop, the same work started again goes on from there.

## Credit

When a provider says its prepaid credit is used up, the error names the billing page. `health` shows the state of the keys.
