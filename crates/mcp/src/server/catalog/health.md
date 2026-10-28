# health

Says whether Qdrant, FalkorDB, the sign-in of the Ingest choice's agent and the two API keys are ready, and which model and thinking each kind of work uses. It runs every check, so one report names everything that is wrong. Each key is checked with the one free call its provider allows, both at once and for at most 5 seconds; neither Google nor Jev gives a balance to a key, so a billing page is given instead.

Gives `healthy` (true when the stores and the agent are ready and both keys are `accepted`), `report` (one line each), `keys` (`key`, `service`, `state`, `detail`, `billing_page`) and `models` (the model and thinking that `ask`, `ingest` and `datasets` use, with the `tools` that spend each). A key `state` is `accepted`, `missing`, `refused`, `out_of_credit` or `unread` (the check could not tell; `detail` says which). Spent credit shows only when the provider says so in answer to a paid call, so a key can be `accepted` and out of credit. Keys are read when the server starts: a key fixed in `.env` needs a restart. No tool shows a key.

## Arguments

None.

## Examples

```json
{}
```

## Errors

- None: a service that is not ready is reported, not raised.

## Cost

Free.

## Related

- `catalog`
- `list_documents`
