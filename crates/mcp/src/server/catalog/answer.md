# answer

Writes an answer to a question from what `search` finds, with the items that each statement rests on. It uses the subscription of the agent of the Ask choice and embeds the question with Gemini. It can take a minute or two. An agent that can write its own answer should call `search` instead.

Gives `answered` (false when nothing was found, and then the agent is not asked, or when what was found does not answer the question), `title`, `claims` (each with `heading`, `text` and the `sources` it rests on, by number), `sources` (each item that a claim names, once: `number` and the fields of a search result without `score` and `reason`), `follow_ups`, and `usage` and `cost_usd`. The sign-in of the agent and `ANTHROPIC_API_KEY` are checked before the search, which costs nothing, so nothing is embedded when the agent cannot answer.

## Arguments

- `question`: required. The question, in plain words.
- `kind`: only items of this kind: `chunk` (running text), `formula`, `figure`, `table` or `code`.
- `document`: only one document: its `title` or its `document_id` from `list_documents`. The title is the whole stored title, whatever the capitals.
- `media`: only documents of this media, whatever the capitals.
- `author`: only documents by this author, whatever the capitals. It matches any author of a document.
- `category`: only documents of this category: `book`, `paper` or `other`.
- `tags`: only documents that have every one of these tags, each as a tag of the media or as an own tag of the document.

## Examples

```json
{ "question": "How does a long straddle make money?" }
```

## Errors

- A blank question or a bad filter is refused with what is allowed.
- An agent that is not signed in, or a store that is not ready, is a tool error that says to call `health`.
- A used-up credit is a tool error that names the billing page.

## Cost

PAID: one Gemini embedding and the subscription usage of the agent of the Ask choice. A minute or two.

## Related

- `search`
- `health`
