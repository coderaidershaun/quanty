# Quanty app icon

A cream Q with a vermilion bookmark tail on an ink tile. The palette follows
the app's paper-and-ink theme; the bookmark connects the monogram to its research
library. The tile has a transparent outer margin.

`quanty.svg` is the editable, hand-authored source. `icon.png` is the 1024 × 1024
RGBA master. The other PNGs cover 16, 32, 64, 128 and 256 pixels.
`icon.ico` includes 16, 24, 32, 48, 64, 128 and 256 pixel representations;
`icon.icns` contains macOS representations through 1024 pixels.

The eframe window setup in `src/app/shell.rs` embeds `icon.png` in the executable
and passes it to `ViewportBuilder::with_icon`. No external image file is needed
at runtime. Eframe also applies this icon to the macOS Dock, including when
launched with `cargo run`. Rebuild and restart the app to see an updated icon.

The ICO and ICNS exports are available for desktop packaging. A macOS app bundle
needs the ICNS file referenced in its `Info.plist` for its Finder icon; setting
the runtime icon alone does not give the executable a custom Finder icon.

The SVG was rendered with CairoSVG and the PNG, ICO and ICNS sizes were exported
with Pillow. All PNGs are RGBA with transparent corners. The image-generation
service rejected the initial request; the delivered design is an original vector
asset, not an AI-generated bitmap.
