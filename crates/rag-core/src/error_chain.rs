//! The message of an error often leaves out the reason, which is in the causes under it.

use std::iter::successors;

/// The error and every cause under it, joined with ": ", so the text alone shows the real reason.
pub fn error_chain(error: &dyn std::error::Error) -> String {
    successors(error.source(), |cause| cause.source())
        .fold(error.to_string(), |text, cause| format!("{text}: {cause}"))
}

/// The text with every run of white space made one space, so it fits on one line.
pub fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
