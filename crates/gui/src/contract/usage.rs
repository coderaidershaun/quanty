//! What the paid models of one run used: the tokens of each model and about what they cost.

#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub struct Usage {
    /// One for each model, by name.
    pub models: Vec<ModelTokens>,
    /// At the prices of the backend's table. `None` when a model has no price.
    pub cost_usd: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ModelTokens {
    pub model: String,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    /// Worked out from the length of the text, because the API gives no count.
    pub estimated: bool,
}

impl ModelTokens {
    /// The four counts added.
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_read + self.cache_write
    }
}

impl Usage {
    /// Every count of every model added.
    pub fn tokens(&self) -> u64 {
        self.models.iter().map(ModelTokens::total).sum()
    }
}
