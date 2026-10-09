//! What the paid models of a run used, model by model, and what that comes to at the prices of
//! one table.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::AddAssign;

/// US dollars for one million tokens of each model that a run can pay for, as the price pages of
/// Anthropic and Google gave them on 9 October 2026. The cache write is the five-minute write, and
/// Haiku 5.5 is priced for prompts of up to 100,000 tokens.
const PRICES: [(&str, ModelPrice); 6] = [
    (
        "claude-fable-5-1",
        ModelPrice::new(10.00, 50.00, 0.25, 12.50),
    ),
    ("claude-opus-5-5", ModelPrice::new(4.00, 20.00, 0.20, 5.00)),
    (
        "claude-sonnet-5-5",
        ModelPrice::new(2.00, 10.00, 0.10, 2.50),
    ),
    ("claude-haiku-5-5", ModelPrice::new(0.10, 0.50, 0.01, 0.125)),
    ("claude-haiku-4-5", ModelPrice::new(1.00, 5.00, 0.10, 1.25)),
    ("gemini-embedding-2", ModelPrice::new(0.20, 0.0, 0.0, 0.0)),
];

/// The short names that `claude --model` takes, and the model each one stands for.
const ALIASES: [(&str, &str); 3] = [
    ("haiku", "claude-haiku-5-5"),
    ("sonnet", "claude-sonnet-5-5"),
    ("opus", "claude-opus-5-5"),
];

/// The tokens that one model read and wrote.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

impl Usage {
    /// The four counts added.
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.cache_read_tokens + self.cache_write_tokens
    }
}

impl AddAssign for Usage {
    fn add_assign(&mut self, other: Usage) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_read_tokens += other.cache_read_tokens;
        self.cache_write_tokens += other.cache_write_tokens;
    }
}

/// US dollars for one million tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelPrice {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

impl ModelPrice {
    const fn new(input: f64, output: f64, cache_read: f64, cache_write: f64) -> ModelPrice {
        ModelPrice {
            input,
            output,
            cache_read,
            cache_write,
        }
    }

    pub fn cost_usd(&self, usage: &Usage) -> f64 {
        let dollars_per_million = usage.input_tokens as f64 * self.input
            + usage.output_tokens as f64 * self.output
            + usage.cache_read_tokens as f64 * self.cache_read
            + usage.cache_write_tokens as f64 * self.cache_write;
        dollars_per_million / 1_000_000.0
    }
}

/// The price of the model, found by its id, by an alias that `claude` takes (`haiku`, `sonnet`,
/// `opus`), or by an id with a date (`-20251001`) or a context mark (`[1m]`) after it.
pub fn price_of(model: &str) -> Option<ModelPrice> {
    table_row(model).map(|(_, price)| *price)
}

/// No other matching is tried: a name that only starts like an id of the table is not that model.
fn table_row(model: &str) -> Option<&'static (&'static str, ModelPrice)> {
    let id = without_date(without_context_mark(model));
    let id = ALIASES
        .iter()
        .find(|(alias, _)| *alias == id)
        .map_or(id, |(_, full_id)| full_id);
    PRICES.iter().find(|(table_id, _)| *table_id == id)
}

fn without_context_mark(model: &str) -> &str {
    match model
        .strip_suffix(']')
        .and_then(|rest| rest.rsplit_once('['))
    {
        Some((id, _)) => id,
        None => model,
    }
}

fn without_date(model: &str) -> &str {
    match model.rsplit_once('-') {
        Some((id, date)) if date.len() == 8 && date.bytes().all(|byte| byte.is_ascii_digit()) => id,
        _ => model,
    }
}

/// What one model used over one or more calls.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ModelUsage {
    pub tokens: Usage,
    /// What `claude` said the calls cost. `None` when nothing said so, as for the embedder.
    pub reported_usd: Option<f64>,
    /// The API gives no count, so the tokens were worked out from the length of the text.
    pub estimated: bool,
}

impl AddAssign for ModelUsage {
    fn add_assign(&mut self, other: ModelUsage) {
        self.tokens += other.tokens;
        self.reported_usd = match (self.reported_usd, other.reported_usd) {
            (Some(mine), Some(theirs)) => Some(mine + theirs),
            (mine, theirs) => mine.or(theirs),
        };
        self.estimated |= other.estimated;
    }
}

/// What the models of a run used, model by model.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UsageTally {
    /// Sorted by model, so every listing of a tally is in one order.
    models: BTreeMap<String, ModelUsage>,
}

impl UsageTally {
    pub fn of(model: &str, usage: ModelUsage) -> UsageTally {
        let mut tally = UsageTally::default();
        tally.add(model, usage);
        tally
    }

    /// Files the model under its id in the table when the table knows it (an alias resolved, a
    /// date or a context mark dropped: `haiku` and `claude-haiku-5-5` are one line,
    /// `claude-haiku-4-5-20251001` is `claude-haiku-4-5`); a model the table does not know keeps
    /// its name.
    pub fn add(&mut self, model: &str, usage: ModelUsage) {
        let name = table_row(model).map_or(model, |(id, _)| id);
        *self.models.entry(name.to_owned()).or_default() += usage;
    }

    pub fn add_all(&mut self, other: &UsageTally) {
        for (model, usage) in other.models() {
            self.add(model, *usage);
        }
    }

    pub fn models(&self) -> impl Iterator<Item = (&str, &ModelUsage)> {
        self.models
            .iter()
            .map(|(model, usage)| (model.as_str(), usage))
    }

    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }

    /// The tokens of every model added.
    pub fn tokens(&self) -> Usage {
        let mut total = Usage::default();
        for usage in self.models.values() {
            total += usage.tokens;
        }
        total
    }

    /// Each model at the table's price, or at what `claude` reported for a model that the table
    /// does not have. `None` when a model has neither.
    pub fn cost_usd(&self) -> Option<f64> {
        self.models()
            .map(|(model, usage)| match price_of(model) {
                Some(price) => Some(price.cost_usd(&usage.tokens)),
                None => usage.reported_usd,
            })
            .sum()
    }
}

/// One line for each model, with no line break after the last, and nothing when the tally is
/// empty. An estimated model ends its line with ` (estimated)`.
impl fmt::Display for UsageTally {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, (model, usage)) in self.models().enumerate() {
            if index > 0 {
                writeln!(formatter)?;
            }
            let tokens = &usage.tokens;
            write!(
                formatter,
                "tokens of {model}: {} in, {} out, {} cache read, {} cache write",
                tokens.input_tokens,
                tokens.output_tokens,
                tokens.cache_read_tokens,
                tokens.cache_write_tokens
            )?;
            if usage.estimated {
                write!(formatter, " (estimated)")?;
            }
        }
        Ok(())
    }
}

/// A cost in words: "≈ $0.42", "under $0.01" for a cost that would round to nothing, and
/// "unknown" when a model has no price.
pub fn cost_text(usd: Option<f64>) -> String {
    match usd {
        None => "unknown".to_owned(),
        Some(usd) if usd > 0.0 && usd < 0.005 => "under $0.01".to_owned(),
        Some(usd) => format!("≈ ${usd:.2}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(input: u64, output: u64, cache_read: u64, cache_write: u64) -> Usage {
        Usage {
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: cache_read,
            cache_write_tokens: cache_write,
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn the_table_prices_ids_aliases_and_dated_ids_and_no_other_model() {
        let haiku = price_of("claude-haiku-5-5").expect("Haiku 5.5 is in the table");
        assert_eq!(price_of("haiku"), Some(haiku));
        let haiku_4_5 = price_of("claude-haiku-4-5-20251001").expect("a dated id is found");
        assert_eq!(price_of("claude-haiku-4-5"), Some(haiku_4_5));
        assert_eq!(haiku_4_5.input, 1.00);
        assert_eq!(
            price_of("claude-sonnet-5-5[1m]"),
            price_of("claude-sonnet-5-5")
        );
        assert!(price_of("claude-sonnet-5-5").is_some());
        assert_eq!(price_of("claude-unknown-9-9"), None);
        assert_eq!(price_of("haiku-5"), None, "no prefix is guessed");

        let sonnet = price_of("sonnet").unwrap();
        let cost = sonnet.cost_usd(&usage(1_000_000, 100_000, 2_000_000, 400_000));
        assert!(close(cost, 2.00 + 1.00 + 0.20 + 1.00), "{cost}");

        let mut tally = UsageTally::default();
        tally.add(
            "claude-sonnet-5-5",
            ModelUsage {
                tokens: usage(1_000_000, 0, 0, 0),
                reported_usd: Some(9.99),
                estimated: false,
            },
        );
        tally.add(
            "stand-in",
            ModelUsage {
                tokens: usage(10, 1, 0, 0),
                reported_usd: Some(0.25),
                estimated: false,
            },
        );
        let cost = tally
            .cost_usd()
            .expect("each model has a price or a report");
        assert!(
            close(cost, 2.00 + 0.25),
            "the table wins, the report fills a gap: {cost}"
        );

        tally.add("unpriced", ModelUsage::default());
        assert_eq!(tally.cost_usd(), None);

        let mut one_model = UsageTally::of(
            "haiku",
            ModelUsage {
                tokens: usage(100, 10, 0, 0),
                reported_usd: None,
                estimated: false,
            },
        );
        one_model.add(
            "claude-haiku-5-5",
            ModelUsage {
                tokens: usage(1, 1, 1, 1),
                reported_usd: Some(0.01),
                estimated: true,
            },
        );
        let models: Vec<(&str, &ModelUsage)> = one_model.models().collect();
        assert_eq!(
            models,
            [(
                "claude-haiku-5-5",
                &ModelUsage {
                    tokens: usage(101, 11, 1, 1),
                    reported_usd: Some(0.01),
                    estimated: true,
                }
            )]
        );
        assert_eq!(
            one_model.to_string(),
            "tokens of claude-haiku-5-5: 101 in, 11 out, 1 cache read, 1 cache write (estimated)"
        );

        assert_eq!(cost_text(None), "unknown");
        assert_eq!(cost_text(Some(0.004)), "under $0.01");
        assert_eq!(cost_text(Some(0.416)), "≈ $0.42");
        assert_eq!(cost_text(Some(0.0)), "≈ $0.00");
    }
}
