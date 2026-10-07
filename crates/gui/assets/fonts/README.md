# Fonts

The files here are built into the program with `include_bytes!`. Each is pinned by the commit
of its source and by its SHA-256. A change of either is a deliberate update, not a drive-by.

Base: `https://raw.githubusercontent.com/google/fonts/`

| File | Source | Version | Licence | SHA-256 |
| --- | --- | --- | --- | --- |
| `Inter-Variable.ttf` | `e1d6480102fed30739fead0faee463101f892c8f/ofl/inter/Inter%5Bopsz%2Cwght%5D.ttf` | 4.001 | SIL OFL 1.1 (`Inter-OFL.txt`) | `29160a80ff49ddcab2c97711247e08b1fab27a484a329ce8b813d820dc559031` |
| `NotoSansMath-Regular.ttf` | `dbd1ab6e65dc59bcda3ca8de9fd372f58f98e0af/ofl/notosansmath/NotoSansMath-Regular.ttf` | 3.000 | SIL OFL 1.1 (`NotoSansMath-OFL.txt`) | `3f495fe933c06786e4d5f6d86b8ee70b6753a68ee3b9d87528726de0f6e2c47d` |
| `Inter-OFL.txt` | `main/ofl/inter/OFL.txt` | | | `5b9321a4298cfeb6b34354164a1c3afc3db114569984c502b9b35d988fd58c57` |
| `NotoSansMath-OFL.txt` | `main/ofl/notosansmath/OFL.txt` | | | `403a95275b469061b7d4371c328e0ada3bc7d63328abe2e88aad5cd243b2fe21` |

Check with `shasum -a 256 *` in this folder.

- Inter is one variable file. The program draws it at three weights (400, 500, 600) by setting
  the `wght` axis, so there is one font file for text, labels and headings.
- Noto Sans Math is the fallback for the math and letter-like signs that Inter lacks.
- Both TTF files must stay under 1 048 576 bytes, the limit of the repository's large-file
  check.
- The licence texts are copied as they are. One line of each ends in a space, and must stay so.
- Monospace and the emoji fallbacks are the fonts that eframe already bundles. Icons come
  from the `egui-phosphor` crate, which carries its own font.
