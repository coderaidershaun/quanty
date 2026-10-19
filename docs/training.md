# Training Gemma 4 on a dataset

How to train Gemma 4 on a dataset that `rag-dataset build` made: continued pre-training (CPT) on the book text, then fine-tuning (SFT) on the checked questions and answers. How a dataset is built, and what every file and field holds, is in [Building a training dataset](reference.md#building-a-training-dataset).

This is a recipe, not a program. The numbers in it are starting points from the data spec that quanty was built to, and they are judgement: tune them with small runs before the full one. No paper fixes them for a small corpus of books on Gemma 4.

## 1. What quanty makes, and what it leaves to you

A build makes the dataset folder: the sections of the books as they are printed, texts that a model rewrote from them, and questions with answers that were checked. The folder and its nine files are in [The folder and the files](reference.md#the-folder-and-the-files). quanty does not train: it starts no run and holds no model weights.

What the build leaves to you:

- Replay data. The dataset holds no public text. Continued pre-training needs 20 to 25% of general text in the mix (part 2).
- Packing into sequences, and the Gemma tokenizer. The `n_tokens` of every record of `cpt/` is an estimate of four characters to a token.
- The chat format. A fine-tuning record has no system turn and no thinking marker (part 4).
- Multi-turn conversations, general instruction data, and any check of the model after training.

The dataset has a version. The `sha256` in `manifest.json` is the SHA-256 of one line `<file sha256>  <path>` for each file of records in `files[]` (not `plan.json` or `manifest.json`), so two builds that wrote the same files have the same `sha256`, whenever they ran. The `sha256` of each file is in `files[]`, and `prompt_versions` and `pipeline_version` say how the texts were made. Write the dataset's `sha256` in the notes of every training run. A build over the same documents with the same options, the same graph and the same kept answers gives the same `sha256`, because it asks no model and writes the same files. A change to a document, an option, the graph, a prompt or the model gives another one.

## 2. Continued pre-training

Start from the Gemma 4 base checkpoint, the `pt` one, and not from an instruction-tuned one.

Train on the `text` of:

- `cpt/raw.jsonl`: the sections of the training documents as the book prints them. At most 4 epochs: the data spec finds that up to about 4 repeats of the same data are nearly as good as new data.
- `cpt/synthetic.jsonl`: the texts that a model rewrote, each judged against its source. The spec fixes the epochs of the raw books only.
- Replay: general text that quanty does not make, about 20 to 25% of the tokens. The spec names FineWeb-Edu-type text, OpenWebMath and code. Replay limits forgetting.

Do not train on `cpt/heldout.jsonl` or `cpt/rejected.jsonl`. The first holds the sections of the documents that were held out, for measuring. The second holds the sections the checks turned down, each with its reason.

### The mix

The spec gives these shares of the tokens, "starting points to tune with small ablation runs", for a total of about 200 to 400 million tokens:

| Share | Content | In the dataset |
| --- | --- | --- |
| About 25% | Raw books, rendered. At most 4 epochs, and no book above about 10% of the raw tokens | `cpt/raw.jsonl` |
| About 40% | Rewrites, EntiGraph texts and notation variants, the notation variants about 5% of this | `cpt/synthetic.jsonl`, the kinds `rephrase_wiki`, `rephrase_lecture`, `entigraph` and `notation_variant` |
| About 10% | Grounded Q&A and reading comprehension, interleaved with the documents | `cpt/synthetic.jsonl`, the kind `rephrase_qa` |
| 20 to 25% | General replay | Not in the dataset |

The spec's table has no row for the kind `exercise`, so decide where to count it. The table gives shares of tokens, and `mix` in the manifest counts records. Work the token shares out from the records: add up the `n_tokens` of the lines of `cpt/raw.jsonl` and of `cpt/synthetic.jsonl` by `kind`. The `files[].n_tokens` of the manifest gives the total of each file. To keep a book under about 10% of the raw tokens, add up the `n_tokens` of `cpt/raw.jsonl` by `source.media`.

The dataset follows the spec's mix only if the build made enough texts. The default of a build is 3 rewrites from each passage of a section, scaled by how rare its concepts are, which is a few times the raw text and not the 10 to 30 times that the spec asks for. Raise `--rewrites`, and read the totals of a dry run before a paid build.

### Records and sequences

- A record is one whole section or one whole text. `text` is Markdown with maths between `$…$` and `$$…$$`, code in fenced listings, and figures as notes that say a model wrote the explanation.
- `images` lists the pictures of a section, each with the file and the offset in characters of the `[` that starts its figure note. A text-only run can leave `images` out.
- Pack whole records into sequences and never cut one. The spec asks for best-fit-decreasing packing into sequences of 8,000 tokens (16,000 to 32,000 if affordable), no attention across records, an end-of-sequence token after each record, and never overlapping windows. A section is at most about 8,000 estimated tokens, except a single block that is longer.
- Count the tokens with the Gemma tokenizer, not with `n_tokens`. Check that LaTeX and Python go through the tokenizer and come back the same. Text full of LaTeX can have more real tokens than the estimate, so a sequence length of 8,000 can be too short for a section the estimate calls 8,000.

### Learning rate and measuring

Re-warm the learning rate to about 10 to 30% of the peak the base model was trained with, then decay it with a cosine to near zero. The spec finds that this schedule with replay matches full retraining.

Measure the perplexity of `cpt/heldout.jsonl` after each checkpoint. It is cheap, it moves smoothly, and the documents in it were never trained on. Also keep a few problem sets of your own that the model never sees, and compare the model with the base on a general benchmark, to see forgetting.

## 3. Fine-tuning

Train on `sft/train.jsonl` for 2 to 3 epochs, with the loss on the model's turns only. The spec aims at 5,000 to 30,000 verified examples, and a build writes at most 300 unless `--max-sft` says more, so raise `--max-sft` and `--questions` for a real run.

Every record is one question and one answer. The `reasoning` field is `think` for the tasks `derivation`, `calculation` and `code_pot`, and `direct` for `explain` and `compare`. The spec aims at about 60% `think` and 40% `direct`, and gives this mix for the whole fine-tuning set:

| Share | Content | Mode | Made by quanty |
| --- | --- | --- | --- |
| About 35% | Quant maths derivations and calculations | think | `derivation` and `calculation` |
| About 25% | Grounded concept explanations | direct | `explain` and `compare` |
| About 20% | Code, program-of-thought, executed | think | `code_pot` |
| About 10% | Multi-turn tutoring | mixed | No |
| About 10% | General instruction replay, and about 1% pretraining text | mixed | No |

Count the shares from the records: `mix.sft_by_task`, `mix.think` and `mix.direct` in the manifest give the numbers of examples, training and held out together; count the records of `sft/train.jsonl` for the training shares alone. The spec cites a finding that as little as 1% of pretraining text keeps the model from forgetting while it is fine-tuned.

Evaluate on `sft/heldout.jsonl`: the verified questions of the documents that were held out. Never train on it. `sft/rejected.jsonl` holds the questions that failed a check, each with its reason. It is there to look at, not to train on.

The spec also names two steps that quanty does not do: dropping the examples that the continued-pre-trained model cannot already answer (`known_by_cpt_model` is null), and choosing the best examples by IFD (`ifd` is null). SFT on facts the model does not know raises hallucination, so if you have a way to measure what the model already knows, use it on `sft/train.jsonl` before training.

## 4. Rendering `think` and `direct`

A record has no system turn and no thinking marker. This is on purpose: the chat template of Gemma 4 has been patched since its release, and markers written into the text would go stale. A record holds the turns and says which mode they are, and you render them with the template of the tokenizer you train with.

- Render each record with the tokenizer's own chat template (`apply_chat_template`), not with a template you copied.
- `think`: switch thinking on the way the template does. The spec's example puts `<|think|>` in a system turn. Put the `thinking` of the assistant message in the thought channel, and its `content` after the channel.
- `direct`: leave thinking off. On the 12B, 26B and 31B models, prefill an empty thought channel, as Google's prompt-formatting guide recommends.
- A `think` record has `thinking` on its assistant message. A `direct` record has no `thinking` key.

The spec shows a `think` example rendered like this. It is an illustration of its time, so check every token against the chat template of your tokenizer before you train:

```
<bos><|turn>system
<|think|><turn|>
<|turn>user
S=100, K=100, r=5%, σ=20%, T=1y. Price the European call.<turn|>
<|turn>model
<|channel>thought
d1=0.35, d2=0.15 … C=10.45<channel|>The call is worth ≈ $10.45 …<turn|>
```

Map `thinking` to the field the template reads. When the template reads none, write the thought channel by hand. Render a few records of each mode, read the text that comes out, and check that the loss mask covers the model's turn and nothing else.

The answer of a `code_pot` record holds a Python program. When the build ran code, which is the default when `python3` can run, `verification.method` is `python_exec`: the program ran and printed the value that the answer states. With `--no-run-code` the method is `judge`, and the judge alone checked the answer.

## 5. A sketch with TRL

This is a sketch to check against the current libraries, and it was not run. The names of the options `max_length`, `packing`, `packing_strategy` and `assistant_only_loss` of `SFTConfig` were checked against the documentation of TRL v1.0.0 on 19 October 2026. Everything else is a placeholder or from memory: the checkpoint, the replay file, the learning rates and the field that the Gemma 4 template reads for a thought.

```python
# SKETCH, not tested. Check every name against the current TRL, transformers and
# Gemma 4 documentation before you use it. Option names checked: TRL v1.0.0 docs, 19 Oct 2026.
import json
from datasets import Dataset
from trl import SFTConfig, SFTTrainer

BASE = "<the Gemma 4 base (pt) checkpoint>"
DATASET = "datasets/small"          # the folder of the dataset; cite its manifest sha256
SEQUENCE = 8192


def lines(path):
    with open(path) as file:
        return [json.loads(line) for line in file]


# 1. Continued pre-training on plain text, packed.
raw = [{"text": r["text"]} for r in lines(f"{DATASET}/cpt/raw.jsonl")]
synthetic = [{"text": r["text"]} for r in lines(f"{DATASET}/cpt/synthetic.jsonl")]
replay = [{"text": r["text"]} for r in lines("replay.jsonl")]  # your own, 20-25% of the tokens
RAW_EPOCHS = 4                      # at most 4
cpt_texts = raw * RAW_EPOCHS + synthetic + replay

cpt = SFTTrainer(
    model=BASE,
    train_dataset=Dataset.from_list(cpt_texts).shuffle(seed=0),
    args=SFTConfig(
        output_dir="gemma4-cpt",
        dataset_text_field="text",
        max_length=SEQUENCE,
        packing=True,
        # "bfd" drops the tokens past max_length. "bfd_split" splits a long text instead.
        packing_strategy="bfd",
        learning_rate=2e-5,         # placeholder: 10-30% of the base model's peak
        lr_scheduler_type="cosine",
        num_train_epochs=1,         # the epochs are in the list above
    ),
)
cpt.train()
cpt.save_model("gemma4-cpt/final")


# 2. Fine-tuning on the checked conversations.
def to_chat(record):
    messages = []
    if record["reasoning"] == "think":
        # From the spec's example. Check how the Gemma 4 template switches thinking on.
        messages.append({"role": "system", "content": "<|think|>"})
    for message in record["messages"]:
        turn = {"role": message["role"], "content": message["content"]}
        if "thinking" in message:
            # PLACEHOLDER: the field that the Gemma 4 template reads for a thought.
            turn["reasoning_content"] = message["thinking"]
        messages.append(turn)
    return {"messages": messages}


sft_rows = [to_chat(r) for r in lines(f"{DATASET}/sft/train.jsonl")]

sft = SFTTrainer(
    model="gemma4-cpt/final",
    train_dataset=Dataset.from_list(sft_rows).shuffle(seed=0),
    args=SFTConfig(
        output_dir="gemma4-sft",
        max_length=SEQUENCE,
        # Needs a chat template that marks the model's tokens. If the Gemma 4 template does
        # not, build the loss mask yourself.
        assistant_only_loss=True,
        learning_rate=1e-5,         # placeholder
        num_train_epochs=2,         # 2-3
    ),
)
sft.train()
```

Two things the sketch does not do. It does not check that packed sequences keep their records apart in attention, which depends on your attention implementation and your version of the library: check this before a long run. And it does not mix in the multi-turn data or the general instruction data of part 3.

## 6. Caveats

- The ratios, the budgets and the thresholds are judgement. This holds for the cap of about 10% of the raw tokens for one book, the 60 to 40 split of `think` and `direct`, and the page gate of the build (its default is 0.90).
- Small models gain less from long reasoning traces. Models of 3 billion parameters or fewer gain little from long chains of thought, so on the E2B and E4B models use shorter traces or more data.
- Synthetic text can be wrong. Every text, and every answer but a program that was run to check it, was judged against its source, and a text was kept only when the judge found it faithful, but a judge is a model. Read about 50 random records and the 50 lowest-scored records of `cpt/synthetic.jsonl` and `sft/train.jsonl` by hand before you train.
- The checks are not complete. No near-duplicate is looked for (`dedup.minhash_cluster` is null), no text or example is compared with any benchmark, and no answer is checked with sympy. The manifest's `notes` lists these.
- Check the Gemma 4 chat template against the tokenizer you train with. It has been patched since its release.
- Copyright is a separate risk. Training on books that are under copyright carries a legal risk that has nothing to do with the quality of the data.
