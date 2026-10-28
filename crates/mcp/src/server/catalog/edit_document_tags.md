# edit_document_tags

Adds tags to one stored document, or takes tags away, in both stores. The tags of its media stay: change those with `edit_media`. Adding a tag the document has, or taking away one it lacks, changes nothing. Give at least one tag. Gives `document_id`, `tags` (the own tags after the change) and `media_tags`. It embeds nothing and asks no model, and does what `rag-ingest tag` does.

## Arguments

- `document_id`: from `list_documents` or a search result.
- `add`: tags to give the document.
- `remove`: tags to take away, after `add`.

## Examples

```json
{ "document_id": "…", "add": ["reviewed"] }
```

## Errors

- No tag, or a blank tag, is refused; nothing changes.
- A document that is not stored is a tool error that says to call `list_documents`.

## Cost

Free.

## Related

- `edit_media`
- `list_documents`
