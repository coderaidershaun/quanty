# list_media

The media of the library, such as books and papers, sorted by title. A media is the book or paper that one or more documents (chapters or parts) belong to. With `detail: summary` each media has its `title`, `category`, `authors`, `tags`, how many `documents` it has, the `pages` of its converted chapters, the `items` stored for its documents (formulas, figures, tables, code and text chunks) and how many documents have saved insights (`with_insights`). With `detail: names` each media has its `title` only, and the result lists nodes that a host can open to browse without asking a model: media, then the documents of a media (`list_documents`), then the outline of a document (`document_outline`), then its sections (`read_section`).

It reads FalkorDB for the media and their documents and the content folder for the pages.

## Arguments

- `category`: only media of this category: `book`, `paper` or `other`.
- `author`: only media by this author, whatever the capitals.
- `tag`: only media with this tag, whatever the capitals.
- `detail`: `summary` (default) or `names`.

## Examples

```json
{ "detail": "names" }
```

## Errors

- A graph that is down is a tool error that says to call `health`.
- A `category` that is not `book`, `paper` or `other`, or a blank `tag`, is refused with the allowed words.

## Cost

Free. It reads FalkorDB and the content folder.

## Related

- `list_documents`
- `document_overview`
- `health`
