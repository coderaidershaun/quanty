//! The insights, takeaways and cheat sheet of one document, as they were saved in its chapter
//! folder when it was ingested or written for it later.

/// One insight: a short Markdown paragraph that may hold `$…$` formulas, and the pages it rests on.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Insight {
    pub text: String,
    /// Page positions, from 1.
    pub pages: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ChapterInsights {
    pub insights: Vec<Insight>,
    /// One-line Markdown points.
    pub takeaways: Vec<String>,
    /// Markdown: short headed sections, formulas and small tables.
    pub cheat_sheet: String,
    /// The model that wrote them, when the file says.
    pub model: Option<String>,
}
