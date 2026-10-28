# search

Finds the stored items (text chunks, formulas, figures, tables and code listings) that are nearest to a question, with their document, page and the reason each was found. It embeds the question with the Gemini API, which is a small paid call: `usage` and `cost_usd` in the result say what it used. Give the `document_id` and `page` of a result to `read_page` to read around it; for a code listing with `last_page`, read each page from `page` to `last_page`.

Gives `results`, best first. Each has `number`, `document_id`, `document` (its title), `media`, `category`, `authors`, `media_tags`, `tags`, `page` (the place of the page in the document, from 1), `printed_page` (the number printed on the page: cite with it), `last_page` and `last_printed_page` (only for a code listing that page breaks cut), `kind`, `label`, `language`, `score`, `reason`, `text` and, for a figure, `picture`. `reason` is `{"why": "nearest"}`, `{"why": "concept", "concept": …}` or `{"why": "cited", "by": …, "label": …}`. With `explain`, `trace` says how each step got to the results. `detail: full` also gives `item_id` (give it to `read_item`) and `section` (the `index` and `path` of the section of `document_outline` that holds the result).

`truncated` is true when a text was cut or a result left out for `max_chars`, and `cursor` is there whenever results are left for the next call. A page of results (`offset` or `cursor`) searches again, which embeds the question again. A title or an id that no stored document has gives no results.

## Arguments

- `question`: required. The question, in plain words, as a person would ask it.
- `kind`: only items of this kind: `chunk` (running text), `formula`, `figure`, `table` or `code`.
- `document`: only one document: its `title` or its `document_id` from `list_documents`. The title is the whole stored title, whatever the capitals.
- `media`: only documents of this media, whatever the capitals. The older name `book` is understood too.
- `author`: only documents by this author, whatever the capitals. It matches any author of a document.
- `category`: only documents of this category: `book`, `paper` or `other`.
- `tags`: only documents that have every one of these tags, each as a tag of the media or as an own tag of the document.
- `limit`: keep only the first `limit` results (from 1).
- `explain`: true adds `trace`.
- `detail`: `full` (default), or `ids` for `number`, `item_id`, `document_id`, `page`, `kind` and `score` only.
- `offset`: skip this many results first. Not with `cursor`.
- `cursor`: from a result that has one, with the same question and filters. Not with `offset`.
- `section`: only inside one section of `document`: its `index` from `document_outline`. `document` must then be a `document_id`.
- `max_chars`: with `detail: full`, at most this many characters of result texts. 24000 when left out.

## Examples

```json
{ "question": "What is the Black–Scholes formula for a call option?" }
```

```json
{ "question": "What is a straddle?", "kind": "formula", "limit": 5, "detail": "ids" }
```

## Errors

- A blank question, an unknown `kind` or `category`, a blank tag, a `limit` of 0 or `offset` with `cursor` is refused with what is allowed.
- A store or the Gemini key that is not ready is a tool error that says to call `health`.
- A used-up Gemini credit is a tool error that names the billing page.

## Cost

PAID: one Gemini embedding of the question, a fraction of a cent. It uses the Ask choice. `usage` and `cost_usd` are in the result.

## Related

- `read_page`
- `read_item`
- `answer`
- `document_outline`
