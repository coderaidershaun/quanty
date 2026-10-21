# Fonts

The files here are built into the `export` crate with `include_bytes!`. A PDF is printed with them,
and the desktop app draws its reading text with the same files, so the program holds one copy.
Each is pinned by the commit of its source and by its SHA-256. A change of either is a deliberate
update, not a drive-by.

Base of the files of Literata and Source Code Pro: `https://raw.githubusercontent.com/google/fonts/`

Base of the files of STIX Two Math: `https://raw.githubusercontent.com/stipub/stixfonts/`

| File | Source | Version | Licence | SHA-256 |
| --- | --- | --- | --- | --- |
| `Literata[opsz,wght].ttf` | `66928797f271fed5ee428544e7229685e2d90127/ofl/literata/Literata%5Bopsz%2Cwght%5D.ttf` | 3.103 | SIL OFL 1.1 (`Literata-OFL.txt`) | `b41138c9373112f32abb589cc22e8674b06ed4048b0c513be922bdd26f274440` |
| `Literata-Italic[opsz,wght].ttf` | `66928797f271fed5ee428544e7229685e2d90127/ofl/literata/Literata-Italic%5Bopsz%2Cwght%5D.ttf` | 3.103 | SIL OFL 1.1 (`Literata-OFL.txt`) | `d483dfaeba9cbf4ce71d32a52ee65df82f7e35b15fff8d1011cdb242d1fcd465` |
| `SourceCodePro[wght].ttf` | `66928797f271fed5ee428544e7229685e2d90127/ofl/sourcecodepro/SourceCodePro%5Bwght%5D.ttf` | 1.026 | SIL OFL 1.1 (`SourceCodePro-OFL.txt`) | `b400fc584e10aff25d0e775ce181b4fc1c5ea1b5dc37b81aeb2084375b945790` |
| `STIXTwoMath-Regular.otf` | `744a22a4dd626cd14d75728aef34fc8ad7c85db0/fonts/static_otf/STIXTwoMath-Regular.otf` | 2.13 b171 | SIL OFL 1.1 (`STIXTwoMath-OFL.txt`) | `3a5f3f26f40d5698b3c62dd085d48d6663696a3f80825aab8b553d5097518e8c` |
| `Literata-OFL.txt` | `66928797f271fed5ee428544e7229685e2d90127/ofl/literata/OFL.txt` | | | `8742963604cd89dc81437811a850018fc03b2bfad686d7422c8235967c87614e` |
| `SourceCodePro-OFL.txt` | `66928797f271fed5ee428544e7229685e2d90127/ofl/sourcecodepro/OFL.txt` | | | `cb30d3086a8b3ce0b9e3690bf48d6620402b61160bc658076f95180ccd9e9dae` |
| `STIXTwoMath-OFL.txt` | `744a22a4dd626cd14d75728aef34fc8ad7c85db0/OFL.txt` | | | `0c8825913b60d858aacdb33c4ca6660a7d64b0d6464702efbb19313f5765861a` |

Check with `shasum -a 256 *` in this folder.

- Literata is the serif that sets what a person reads. It comes as two variable files, upright and
  italic, each with a weight axis (`wght`) and an optical size axis (`opsz`). The app draws each
  file at the weights and the optical sizes of its text families (see `fonts.rs` of the `gui`
  crate), so a family is one file at one setting of its axes. The PDF sets the body at weight 400,
  the strong text at 600 and the optical size 14.
- Source Code Pro is the monospace font, at weight 400. It is a variable file whose axis starts at
  200, so the program always sets the weight and never relies on the default of the file.
- STIX Two Math is the fallback of every text family for the math and letter-like signs that the
  text fonts lack. It has CFF outlines, which the text renderer reads like TrueType outlines. Its
  line is shorter than that of the text fonts, so the app draws it a little lower under each of
  them, and its signs stand on their baseline.
- Each font file must stay under 1 048 576 bytes, the limit of the repository's large-file check.
- The licence texts are copied as they are, byte for byte.
