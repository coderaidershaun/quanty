# Fonts

The files here are built into the program with `include_bytes!`. Each is pinned by the commit
of its source and by its SHA-256. A change of either is a deliberate update, not a drive-by.

Base of the files of Source Sans 3: `https://raw.githubusercontent.com/google/fonts/`

| File | Source | Version | Licence | SHA-256 |
| --- | --- | --- | --- | --- |
| `SourceSans3[wght].ttf` | `66928797f271fed5ee428544e7229685e2d90127/ofl/sourcesans3/SourceSans3%5Bwght%5D.ttf` | 3.052 | SIL OFL 1.1 (`SourceSans3-OFL.txt`) | `042fe2cc0b933e328410d7acbd0aa6a1873dca5aef81875f4bc214b08825c7b9` |
| `SourceSans3-OFL.txt` | `66928797f271fed5ee428544e7229685e2d90127/ofl/sourcesans3/OFL.txt` | | | `09746787287a289323b0ec3cff4d1a4a801331b82b7207c1e186f5d26619a392` |

Check with `shasum -a 256 *` in this folder.

- Source Sans 3 sets the controls, in three weights (400, 500, 600). It is a variable file whose
  axis starts at 200, so the program always sets the weight and never relies on the default of
  the file.
- Literata, Source Code Pro and STIX Two Math, with their licence texts, are in
  `crates/export/assets/fonts/`: the `export` crate prints its PDFs with them, and the app draws
  its text with the same files.
- Each font file must stay under 1 048 576 bytes, the limit of the repository's large-file check.
- The licence texts are copied as they are, byte for byte.
- Hack and the emoji fallbacks are the fonts that eframe already bundles, and they come after
  STIX Two Math in every text family. Icons come from the `egui-phosphor` crate (its light
  variant), which carries its own font.
