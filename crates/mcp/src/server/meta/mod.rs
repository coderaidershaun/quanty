//! What a tool says about itself beyond its name: the marks in its `_meta` that hosts read, and
//! the short schemas of its arguments and of its result.

mod marks;
mod schema;

pub(super) use marks::{COST_KEY, GROUP_KEY, Payer};
pub(crate) use marks::{Cost, Group, ToolMarks};
pub(super) use schema::input_schema;
pub(crate) use schema::output_schema;
