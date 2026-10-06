# Samples

`chapter-1-sample-pages.pdf` is a seven page chapter built with `pdfunite` from sample pages of one book, in this order: a text page, a footnote page, a tables page, a math page, a line chart page, a mixed page and a 3-D surface chart page.

Never rebuild it. A rebuilt file has another hash, and `ocr` refuses a PDF that differs from the one a chapter was made from.

`content/` is that chapter converted, so other features can test against it without spending quota:

```bash
cargo run -p ocr -- --book "Option Volatility and Pricing" --out samples/content samples/chapter-1-sample-pages.pdf
```

The three figures in it were converted before the `unchecked` key existed, so their `image` entries in `page.json` do not carry it and it reads as false, which is what `ocr` would write for them.

`content/quanty-sample-notes/` holds two short chapters that are written by hand, for the tests and for the search and concept features that come later. There is no PDF behind them, they have no pictures, and their `source-sha256` values are made up. `chapter-1` explains what options are worth at the level of intuition and ends with the idea behind the Black–Scholes model. `chapter-2` derives the Black–Scholes equation and gives the pricing formulas as formula pieces. They are in the same format as a converted chapter, and `ocr::read_chapter` reads them.

`stand-in-claude/claude` is a small shell script that tests put first on the `PATH` in place of the real `claude` program, so that a test can see what the program is given and make it answer or fail in any way, with no model call. The environment variable `STAND_IN_CLAUDE` names a folder. The script writes there what it was given (`arguments`, `stdin`, `environment`) and one line in `calls` for each run, prints the file `stdout` of that folder, and exits with the number in the file `exit-code`, or 0 when there is none.
