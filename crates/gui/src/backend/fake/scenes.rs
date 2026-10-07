//! The scenes the fake backend can play, and what each one is for.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Scene {
    pub name: &'static str,
    pub about: &'static str,
    /// True when the app settles and asks for no repaint once it has opened.
    pub rests: bool,
}

static SCENES: [Scene; 1] = [Scene {
    name: "idle",
    about: "Healthy, with an empty library and nothing asked.",
    rests: true,
}];

pub fn scenes() -> &'static [Scene] {
    &SCENES
}
