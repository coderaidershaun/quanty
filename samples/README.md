# Samples

`chapter-1-sample-pages.pdf` is a seven page chapter built with `pdfunite` from sample pages of one book, in this order: a text page, a footnote page, a tables page, a math page, a line chart page, a mixed page and a 3-D surface chart page.

Never rebuild it. A rebuilt file has another hash, and the converter refuses a PDF that differs from the one a chapter was made from.

`content/` is that chapter converted, so other features can test against it without spending quota:

```bash
cargo run -p converter -- --book "Option Volatility and Pricing" --out samples/content samples/chapter-1-sample-pages.pdf
```
