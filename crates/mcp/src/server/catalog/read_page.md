# read_page

The pieces of one page of a chapter, in reading order, to read around a search result. Give the `document_id` and `page` of a result. It reads the saved chapter and needs no store.

It gives `truncated` (and `cursor`), `document_id`, `media`, `chapter_number` and `chapter_name` (for a book's chapter), `document_title` (for a document with a title of its own, which for a part of a book is "<book>, <part>"), `page`, `pages` (the page count of the document), `printed_page`, `page_image` (the whole-page picture URL relative to the app base) and `pieces`. Each piece has `number`, `kind` (`heading`, `text`, `formula`, `figure`, `table`, `footnote` or `code`), `section`, `label`, `caption`, `name`, `language`, `content` and, for a figure, `picture`.

A long page continues with `max_chars` (24000 when left out) and `cursor`. To read items that `concept_items` found, prefer `read_item` or `read_pages`.

## Arguments

- `document_id`: the `document_id` of a search result or of `list_documents`.
- `page`: the `page` of a search result: the place of the page in the chapter, from 1. It is not the number printed on the page.
- `max_chars`: at most this many characters of text. 24000 when left out.
- `cursor`: from a result that was `truncated`, with the same other arguments.

## Examples

```json
{ "document_id": "…", "page": 1 }
```

## Errors

- A document with no converted folder is a tool error that names the content folder and says to call `list_documents`.
- A page the document does not have is a tool error that says how many pages there are.

## Cost

Free. It reads the converted files and needs no store.

## Related

- `search`
- `read_pages`
- `chapter_contents`
