# chapter_insights

The insights, takeaways and cheat sheet written for one document when it was ingested: short Markdown paragraphs with the pages each rests on, one-line points, and a Markdown sheet of definitions, formulas and rules of thumb. They are the notes that the agent of the Ingest choice wrote from the whole converted text, saved in `insights.json`.

`state` is `saved` (the notes are there: `file`, `model`, `prompt_version`, `insights` each with `text` and `pages`, `takeaways` and `cheat_sheet`), `not_written_yet` (nobody has written them: call `write_insights` with this `document_id`, which is paid) or `none_for_book_part` (a part of a book that is not a chapter, such as its index, gets none). It reads the saved file and costs nothing; it reads the graph only for a document with a title of its own that has no file, to see whether it is a part of a book.

Long notes continue with `max_chars` (24000 when left out) and `cursor`; the units are each insight, each takeaway, then the cheat sheet.

## Arguments

- `document_id`: from `list_documents` or a search result.
- `max_chars`: at most this many characters of text. 24000 when left out.
- `cursor`: from a result that was `truncated`, with the same other arguments.

## Examples

```json
{ "document_id": "…" }
```

## Errors

- A file that cannot be read is a tool error that names the file and says to call `write_insights`, which writes it again and is paid.
- A document with no converted folder is a tool error that says to call `list_documents`.

## Cost

Free. It reads the saved file.

## Related

- `document_overview`
- `write_insights`
- `export_insights`
