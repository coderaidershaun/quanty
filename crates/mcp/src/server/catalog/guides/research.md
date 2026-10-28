# Research

Quanty gives you the moves; you plan the research. Most are free, and none needs a model.

## Navigate

- `list_media`, then `list_documents` (filter by `media`, `category`, `author`, `tags`; `detail: names` is the smallest).
- `document_overview`: one call for a document's insights, takeaways, concepts and counts.
- `document_outline`: the sections with page ranges, item counts and concept counts.

## Read

- `read_section`: one section as Markdown or `as: pieces`. The best unit to read.
- `read_pages`: a run of pages. `read_page`: one page, to read around a search result.
- `read_item`: one formula, figure, table or code listing by `item_id`, or by `kind` and printed `label`, with the paragraphs that cite it.
- `chapter_contents`: every formula, figure, table and code listing of a document. `chapter_insights`: the saved insights, takeaways and cheat sheet.
- `read_many`: up to 20 of those reads in one call under one `max_chars`.

## Find

- `search`: items nearest a question, PAID (one embedding). Narrow it with `document`, `media`, `kind`, `section`. `detail: ids` returns ids only. Page with `cursor`.
- `answer`: a written answer with sources. PAID and slow; if you can write the answer yourself, use `search`.
- `similar_items`: items like an item, free.

## Walk the concepts

`find_concepts` (by `name` free, by `query` PAID) finds a concept. `concept` shows it. `concept_neighbours` follows its relations (`DERIVED_FROM`, `ASSUMES`, `GENERALISES`, `PART_OF`, `USED_FOR`). `concept_items` lists the items that mention it; read them with `read_item` or `read_pages`. `concept_path` links two concepts. `compare_documents` and `document_concepts` compare and list concepts per document.

## Keep within a budget

Every read takes `max_chars`. Prefer `detail: names` or `summary` first, read the outline before a section, and use `cursor` to continue instead of asking for more.
