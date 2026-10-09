//! What an agent reads of the tokens a call used and about what they cost.

use rag_core::UsageTally;
use schemars::JsonSchema;
use serde::Serialize;

/// The tokens that one model read and wrote.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub(crate) struct UsageView {
    model: String,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    /// True when the count was worked out from the length of the text: the embeddings API gives
    /// none.
    estimated: bool,
}

/// The tokens a call used and about what they cost.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub(crate) struct Spent {
    /// The tokens of each model that the call used.
    usage: Vec<UsageView>,
    /// About what the call cost in US dollars, at the prices of the API. `null` when a model has
    /// no price.
    cost_usd: Option<f64>,
}

impl From<&UsageTally> for Spent {
    fn from(tally: &UsageTally) -> Spent {
        let usage = tally
            .models()
            .map(|(model, used)| UsageView {
                model: model.to_owned(),
                input_tokens: used.tokens.input_tokens,
                output_tokens: used.tokens.output_tokens,
                cache_read_tokens: used.tokens.cache_read_tokens,
                cache_write_tokens: used.tokens.cache_write_tokens,
                estimated: used.estimated,
            })
            .collect();
        Spent {
            usage,
            cost_usd: tally.cost_usd(),
        }
    }
}
