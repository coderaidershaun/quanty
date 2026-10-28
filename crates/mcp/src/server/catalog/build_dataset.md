# build_dataset

Builds a training dataset for Gemma 4 from the converted documents under the content folder: whole book sections for continued pre-training, rewritten texts and checked questions and answers for fine-tuning, and a manifest. It is the same build as `rag-dataset build`. It is PAID unless `dry_run` is true: the agent of the Datasets choice writes every rewritten text and answers every question, which spends its subscription usage and can take hours. A dry run exports, checks and plans, asks no model and costs nothing, and its `summary` says what the full build would ask: run one first.

The call answers when the build has begun, with a `job_id`: call `watch_job` with it (it waits for you) or `job_status`. The stages are `exporting`, `checking`, `planning`, `writing_synthetic`, `writing_sft` and `writing_manifest`. The result has the fields of a job, `usage` and `cost_usd` so far, `folder` and `summary` (what `rag-dataset build` prints).

A build with the name of an earlier one replaces that dataset's files once it knows it can run, and the answers a model already gave are kept (in `DATASET_CACHE_DIR`) and not paid for twice. Give a new `name` to keep the earlier dataset. The files are written in `<DATASET_DIR>/<name>`: `cpt/`, `sft/`, `plan.json` and `manifest.json`. `run_code` runs Python that the model wrote, on the server's machine, to check an answer; it is on by default.

Unified HTTP permits only one background job across the app and MCP. Stdio permits one job of each kind at a time.

## Arguments

- `name`: required. The name of the dataset: letters, digits, `-`, `_` and `.`, and not `.` or `..`. It is the name of its folder in the dataset folder.
- `media`: only the documents of these media, by title whatever the capitals. Every media when left out.
- `documents`: only these documents, each a `document_id` or the whole `title` from `list_documents`. Every document of the chosen media when left out.
- `holdout`: documents kept out of training for evaluation, named like `documents`. Their sections go to `cpt/heldout.jsonl`, and no synthetic text is written from them.
- `include_book_parts`: also export the parts of a book that are not chapters, such as its index. An appendix is always exported. False when left out.
- `rewrites`: how many rewritten texts are written from each passage of a training section (at most 2,500 estimated tokens). 3 when left out.
- `questions`: how many questions each section gets. 2 when left out.
- `max_sft`: the most fine-tuning examples the build writes. 300 when left out.
- `max_synthetic`: the most synthetic texts the build writes. No limit when left out.
- `samples`: how many times the model answers each question, to check that its answers agree. 3 when left out.
- `run_code`: run the Python that the model writes for an answer, to check it. True when left out.
- `dry_run`: export, check and plan only. It asks no model and costs nothing. False when left out.
- `min_page_score`: a section whose worst page matches less of the PDF's own text than this share, from 0 to 1, is rejected. 0.90 when left out.
- `keep_unrendered`: keep a section whose formulas do not render, instead of rejecting it. False when left out.

## Examples

```json
{ "name": "options-v1", "dry_run": true }
```

## Errors

- A blank or bad `name`, or a `media`, `documents` or `holdout` entry that no converted document matches, is a tool error that says what to give instead.
- A graph that is down, or a model that is not signed in for a build that is not a dry run, is a tool error that says to call `health`.
- Another background job is running: ask `job_status` with its job_id and start this build when it has ended.

## Cost

PAID unless `dry_run` is true: the subscription usage of the agent of the Datasets choice, and it can take hours. A dry run is free.

## Related

- `watch_job`
- `job_status`
- `stop_job`
- `list_documents`
