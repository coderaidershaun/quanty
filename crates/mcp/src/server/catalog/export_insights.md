# export_insights

Writes the saved insights, takeaways and cheat sheet of one document to a file that stands alone, the same file that Export in the Insights view of the browser app writes: the title, a line that names the model, the insights with their pages, the takeaways and the cheat sheet. The PDF has its formulas typeset, the HTML file has its pictures inside it, and the Markdown file keeps its formulas as `$…$`. It writes to `path` on the machine the server runs on, and never replaces a file unless `overwrite` is true.

Every check runs before anything is written: `format`, that `path` is absolute, that it ends with the extension of the format, that its folder exists (none is made), that no file is at `path` unless `overwrite` is true, and then the saved insights. A document with no saved insights gets them from `write_insights` first. Gives `document_id`, `format`, `path` and `bytes`. The tool says destructive to the client because `overwrite` can replace a file.

## Arguments

- `document_id`: from `list_documents` or a search result.
- `format`: required. `pdf`, `markdown` or `html`, whatever the capitals.
- `path`: required. The absolute path of the file to write, ending in `.pdf`, `.md` or `.html` as `format` is. Its folder must exist.
- `overwrite`: true to replace a file that is at `path`. False when left out: a file that is there is kept and the call fails.

## Examples

```json
{ "document_id": "…", "format": "markdown", "path": "/srv/exports/chapter-1.md" }
```

## Errors

- A bad `format`, a `path` that is not absolute, has the wrong extension or has no folder is refused; nothing is written.
- A file at `path` without `overwrite: true`: give another path, or `overwrite: true`.
- A document with no saved insights: call `write_insights` with this `document_id` (paid), then export again. A part of a book that is not a chapter gets none.

## Cost

Free. It asks no model and embeds nothing; it writes one file.

## Related

- `chapter_insights`
- `write_insights`
