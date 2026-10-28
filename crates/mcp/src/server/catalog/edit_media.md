# edit_media

Changes the labels of a stored media: its category, its authors or its tags. A label that is given replaces the old one and one that is left out stays; `[]` takes every author or every tag away. Give at least one. Every document of the media, and every stored item of those documents, gets the new labels in both stores, so `search` and `answer` filter by them at once. It embeds nothing and asks no model. The title names the media and cannot change. The category says how the next PDF of the media must be named: a book's chapter PDFs must be named `chapter-<number>-<name>.pdf`.

Gives `media`, `category`, `authors`, `tags` and `documents` (how many documents now carry them). The arguments are checked before any store is reached, so a bad edit changes nothing, and the old values give the labels back. The tool says destructive to the client because a given list replaces the old one. It does what `rag-ingest media` does.

## Arguments

- `media`: required. The title of the stored media, whatever the capitals, as `list_documents` gives it.
- `category`: `book`, `paper` or `other`.
- `authors`: the authors, in order. They replace the old ones; `[]` takes every author away.
- `tags`: the tags of the media. They replace the old ones; `[]` takes every tag away.

## Examples

```json
{ "media": "Option Volatility and Pricing", "tags": ["options", "volatility"] }
```

## Errors

- Nothing to change, a blank tag, a bad `category` or a blank `media` is refused; nothing changes.
- A media that is not stored is a tool error that says to call `list_documents`.

## Cost

Free.

## Related

- `edit_document_tags`
- `list_media`
