//! The message of an error often leaves out the reason, which is in the causes under it.

/// The error and every cause under it, joined with ": ", so the text alone shows the real reason.
pub fn error_chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}
