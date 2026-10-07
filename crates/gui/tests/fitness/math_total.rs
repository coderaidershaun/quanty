//! Checks that every formula that is asked for ends as a picture or as its source, and none
//! makes the app stop.
//!
//! This is a fitness test because the failure is silent: a formula that kills the worker, or
//! one that the typesetter never answers, leaves a spinner on the screen for ever with no
//! message, and the text that models write is not a space that a list of examples can cover.

use eframe::egui;
use gui::media::Offload;
use gui::media::math::{Display, Math, MathRef, MathState};
use proptest::prelude::*;

/// Formulas the books and the page reader write, halves of them, and negative space.
const CORPUS: &[&str] = &[
    r"\frac{a}{b}",
    r"\sqrt{T}",
    r"\begin{aligned} a &= b \\ c \end{aligned}",
    r"\left(",
    r"\right)",
    r"x_{i|j}^{2}",
    r"\text{of } \$5",
    r"\mathbb{E}",
    r"\sum_{i=1}^{n}",
    r"\begin{aligned} a &= 1 \\{b} &= \left( d \right) \end{aligned}",
    r"\begin{gathered} a \\ b \end{gathered}",
    r"x_{1} \le 8\%",
    r"\$5",
    r"\!",
    r"\kern-3em",
];

fn piece() -> impl Strategy<Value = String> {
    prop_oneof![
        6 => prop::sample::select(CORPUS).prop_map(str::to_owned),
        2 => "[{}\\\\^_&%$#~ ]{1,3}",
        2 => "\\PC{0,6}",
        1 => (1usize..400).prop_map(|depth| r"\frac{".repeat(depth)),
        1 => (1usize..2500).prop_map(|terms| "x+".repeat(terms)),
    ]
}

/// Pieces spliced together, and sometimes cut short in the middle of a construct.
fn latex() -> impl Strategy<Value = String> {
    (
        prop::collection::vec(piece(), 0..8),
        prop::option::of(0usize..80),
    )
        .prop_map(|(pieces, cut)| {
            let spliced = pieces.concat();
            match cut {
                Some(chars) => spliced.chars().take(chars).collect(),
                None => spliced,
            }
        })
}

fn display() -> impl Strategy<Value = Display> {
    prop_oneof![Just(Display::Inline), Just(Display::Block)]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn any_latex_ends_ready_or_failed(
        source in latex(),
        display in display(),
        size in 6.0f32..96.0,
    ) {
        let ctx = egui::Context::default();
        let mut math = Math::new(&ctx, Offload::Manual);
        let asked = MathRef { latex: &source, display, size };
        let plain = MathRef { latex: "x", display: Display::Inline, size: 14.0 };

        prop_assert_eq!(math.get(&asked), MathState::Loading);
        prop_assert_eq!(math.get(&plain), MathState::Loading);
        math.run_pending();
        math.poll(&ctx);
        prop_assert!(math.is_idle(), "a formula is still loading after its job ran");

        match math.get(&asked) {
            MathState::Ready(image) => {
                let lengths = [
                    image.texture_size.x,
                    image.texture_size.y,
                    image.bleed,
                    image.width,
                    image.ascent,
                    image.descent,
                ];
                prop_assert!(
                    lengths.iter().all(|length| length.is_finite() && *length >= 0.0),
                    "a ready formula has a size that is not a length: {:?}", image
                );
                let limit = ctx.input(|input| input.max_texture_side);
                let manager = ctx.tex_manager();
                let manager = manager.read();
                let meta = manager.meta(image.texture);
                prop_assert!(
                    meta.is_some_and(|meta| meta.size.iter().all(|side| *side <= limit)),
                    "a ready formula has no texture, or one larger than {}", limit
                );
            }
            MathState::Failed => {}
            MathState::Loading => prop_assert!(false, "the formula is still loading"),
        }
        prop_assert!(
            matches!(math.get(&plain), MathState::Ready(_)),
            "a formula after it was not typeset"
        );
    }
}
