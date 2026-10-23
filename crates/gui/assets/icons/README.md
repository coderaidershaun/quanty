# Quanty app icon

A cream Q with a vermilion bookmark tail on an ink tile. The palette follows
the app's paper-and-ink theme; the bookmark connects the monogram to its research
library. The tile has a transparent outer margin.

`quanty.svg` is the editable, hand-authored source. `icon.png` is the 1024 × 1024
RGBA master. The other PNGs cover 16, 32, 64, 128 and 256 pixels.

These files preserve the app's artwork. The browser bundle has its own favicon
and manifest images under `crates/web-ui/assets/`; changing this artwork does
not update those images automatically.

The SVG was rendered with CairoSVG and the PNG sizes were exported
with Pillow. All PNGs are RGBA with transparent corners. The delivered design
is an original vector asset.
