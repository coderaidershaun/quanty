# Errors

A failed call is a result with `isError: true` and a text that says what to do. It never ends the connection. Read the text first; it names what was given and what is allowed.

## Common causes

- A store or a key is not ready: the text ends with "Call the `health` tool". Fix what `health` reports; the keys are read when the server starts.
- A bad argument: a blank question, an unknown `kind` or `category`, a blank tag, a `limit` of 0 or out of range, a `max_chars` of 0 or over 200000, a cursor from another read. Nothing is changed quietly; fix the argument.
- An id that is not stored: call `list_documents`, `find_concepts` or `catalog`. A page or a section that the document does not have names how many it has.
- A converted folder that is missing: `read_page` and its kin say to call `list_documents`.
- An unknown tool name for `describe_tool`: call `catalog`.
- An unknown `job_id`: job ids are forgotten when the server restarts. Send the PDF again, or start the work again.
- Another background job is running: the text names its `job_id`; ask `job_status` with it, and start yours when it ends.
- A PDF that is refused, or a `pdf_url` that is not allowed: the text gives the reason and the fix. `check_pdf` finds these before any cost.
- A file that `export_insights` cannot write: the path and the reason. A file at `path` needs `overwrite: true`.
- A used-up credit: the text names the billing page of Gemini or Jev.

## Not errors

A graph that is down does not fail `document_overview` or `document_outline`: the part that needs it carries the reason in an error field. A section the result says is `truncated` is not an error: continue with `cursor`.
