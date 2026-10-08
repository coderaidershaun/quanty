//! Checks that a formula asked for through the media code ends as a picture or as its LaTeX
//! source.

use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gui::media::Offload;
use gui::media::math::{self, Display, Math, MathImage, MathRef, MathState};
use gui::testkit::{self, Host};
use gui::theme::TextRole;

fn formula(latex: &str, size: f32) -> MathRef<'_> {
    MathRef {
        latex,
        display: Display::Block,
        size,
    }
}

/// The window code runs one pass for every frame. A change of scale reaches `Context` only at
/// the start of the next pass.
fn pass(ctx: &egui::Context) {
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
    output.textures_delta.clear();
}

fn math_textures(ctx: &egui::Context) -> Vec<usize> {
    let manager = ctx.tex_manager();
    let manager = manager.read();
    manager
        .allocated()
        .filter(|(_, meta)| meta.name.starts_with("math:"))
        .map(|(_, meta)| meta.bytes_used())
        .collect()
}

fn ready(math: &mut Math, ctx: &egui::Context, asked: &MathRef<'_>) -> MathImage {
    assert_eq!(math.get(asked), MathState::Loading, "a new formula waits");
    math.run_pending();
    math.poll(ctx);
    match math.get(asked) {
        MathState::Ready(image) => image,
        other => panic!("`{}` ended as {other:?}", asked.latex),
    }
}

#[test]
fn a_formula_is_typeset_once_for_each_pixel_size() {
    let ctx = egui::Context::default();
    let mut math = Math::new(&ctx, Offload::Manual);
    let small = formula(r"\frac{a}{b}", 14.0);
    let first = ready(&mut math, &ctx, &small);
    for _ in 0..30 {
        math.poll(&ctx);
        match math.get(&small) {
            MathState::Ready(image) => assert_eq!(image.texture, first.texture),
            other => panic!("a formula that is ready changed to {other:?}"),
        }
    }
    math.run_pending();
    math.poll(&ctx);
    assert_eq!(math_textures(&ctx).len(), 1, "one picture for one formula");

    let big = ready(&mut math, &ctx, &formula(r"\frac{a}{b}", 28.0));
    assert_ne!(big.texture, first.texture);
    assert_eq!(math_textures(&ctx).len(), 2, "one picture for each size");

    let ctx = egui::Context::default();
    let mut math = Math::new(&ctx, Offload::Manual);
    let asked = formula(r"\frac{a}{b}", 20.0);
    let at_one = ready(&mut math, &ctx, &asked);
    ctx.set_pixels_per_point(2.0);
    pass(&ctx);
    math.poll(&ctx);
    let at_two = ready(&mut math, &ctx, &asked);
    assert_ne!(at_two.texture, at_one.texture);
    assert_eq!(math_textures(&ctx).len(), 2, "one picture for each scale");
    let (one, two) = (at_one.texture_size, at_two.texture_size);
    assert!(
        (one - two).abs().max_elem() <= 1.0,
        "the same formula is as large in points at both scales: {one:?} and {two:?}"
    );
    assert!(
        (at_one.width - at_two.width).abs() <= 1.0,
        "the box is as wide in points at both scales"
    );
}

#[test]
fn the_box_metrics_place_the_baseline() {
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(2.0);
    pass(&ctx);
    let mut math = Math::new(&ctx, Offload::Manual);
    let pixel = 1.0 / ctx.pixels_per_point();
    let mut measure = |latex: &str| ready(&mut math, &ctx, &formula(latex, 20.0));

    let x = measure("x");
    let subscript = measure("x_1");
    let fraction = measure(r"\frac{a}{b}");

    assert!(x.descent.abs() <= pixel / 2.0, "x sits on the baseline");
    assert!(subscript.descent > 0.0, "a subscript hangs below it");
    assert!(fraction.descent > 0.0, "a fraction hangs below it");
    assert!(
        fraction.ascent > x.ascent,
        "a fraction stands higher than x"
    );
    for image in [x, subscript, fraction] {
        let boxed = image.size() + egui::Vec2::splat(2.0 * image.bleed);
        assert!(
            (image.texture_size - boxed).abs().max_elem() <= pixel,
            "the texture is the box and a margin on every side: {image:?}"
        );
    }
}

#[test]
fn the_math_cache_stays_under_its_byte_budget() {
    const BUDGET: usize = 1024 * 1024;
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(2.0);
    pass(&ctx);
    let mut math = Math::new(&ctx, Offload::Manual).with_budget(BUDGET);

    let sources: Vec<String> = (1..=40)
        .map(|n| {
            format!(r"\frac{{\partial V}}{{\partial t}} + \frac{{1}}{{2}}\sigma^2 S^2 \frac{{\partial^2 V}}{{\partial S^2}} = {n}")
        })
        .collect();
    for latex in &sources {
        ready(&mut math, &ctx, &formula(latex, 20.0));
        let bytes: usize = math_textures(&ctx).iter().sum();
        assert!(bytes <= BUDGET, "{bytes} bytes of textures after a poll");
    }

    let last = sources.last().expect("forty formulas were asked");
    assert!(
        matches!(math.get(&formula(last, 20.0)), MathState::Ready(_)),
        "the formula that is on screen is kept"
    );
}

const GOOD_BLOCK: &str =
    r"d_1 = \frac{\ln(S/K) + \left(r + \frac{1}{2}\sigma^2\right)T}{\sigma\sqrt{T}}";
const GOOD_INLINE: &str = r"\sigma\sqrt{T}";
const BAD_BLOCK: &str = r"\frac{a";
/// Negative space alone: it can be read, but its box is narrower than nothing.
const BAD_INLINE: &str = r"\hspace{-2em}";

fn panel_with(block: &'static str, inline: &'static str) -> Harness<'static, Host> {
    testkit::panel([420.0, 140.0], testkit::asked("q"), move |ui, cx| {
        math::show(ui, cx.media, &MathRef::block(block));
        ui.horizontal(|ui| {
            ui.label(TextRole::Body.rich("The spread of the price is"));
            math::show(ui, cx.media, &MathRef::inline(inline, TextRole::Body));
        });
    })
}

fn state_of(harness: &mut Harness<'_, Host>, math: &MathRef<'_>) -> MathState {
    harness.state_mut().media.math.get(math)
}

fn assert_named_once_as_an_image(harness: &Harness<'_, Host>, latex: &str) {
    assert_eq!(
        harness.query_all_by_label(latex).count(),
        1,
        "`{latex}` names one node"
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Image, latex)
            .is_some(),
        "the node named `{latex}` is an image"
    );
}

#[test]
fn broken_latex_fails_and_the_widget_keeps_its_name() {
    let mut good = panel_with(GOOD_BLOCK, GOOD_INLINE);
    let mut broken = panel_with(BAD_BLOCK, BAD_INLINE);
    let block = |latex| MathRef::block(latex);
    let inline = |latex| MathRef::inline(latex, TextRole::Body);

    good.run();
    broken.run();
    for latex in [GOOD_BLOCK, GOOD_INLINE] {
        assert_named_once_as_an_image(&good, latex);
    }
    for latex in [BAD_BLOCK, BAD_INLINE] {
        assert_named_once_as_an_image(&broken, latex);
    }
    assert_eq!(state_of(&mut good, &block(GOOD_BLOCK)), MathState::Loading);
    assert_eq!(state_of(&mut broken, &block(BAD_BLOCK)), MathState::Loading);
    testkit::save_png(&mut good, "math-loading");

    for harness in [&mut good, &mut broken] {
        harness.state_mut().media.run_pending();
        harness.run();
    }
    for latex in [GOOD_BLOCK, GOOD_INLINE] {
        assert_named_once_as_an_image(&good, latex);
    }
    for latex in [BAD_BLOCK, BAD_INLINE] {
        assert_named_once_as_an_image(&broken, latex);
    }
    assert!(matches!(
        state_of(&mut good, &block(GOOD_BLOCK)),
        MathState::Ready(_)
    ));
    assert!(matches!(
        state_of(&mut good, &inline(GOOD_INLINE)),
        MathState::Ready(_)
    ));
    assert_eq!(state_of(&mut broken, &block(BAD_BLOCK)), MathState::Failed);
    assert_eq!(
        state_of(&mut broken, &inline(BAD_INLINE)),
        MathState::Failed
    );
    testkit::save_png(&mut good, "math-ready");
    testkit::save_png(&mut broken, "math-failed");
}
