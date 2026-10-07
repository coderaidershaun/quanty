//! Checks the words of a `claude` run that failed, which need no `claude` to be started.

use ocr::convert::services::ClaudeError;

#[test]
fn a_failed_run_with_a_blank_result_says_no_reason_was_given() {
    let failed = ClaudeError::RunFailed {
        subtype: "error_during_execution".to_owned(),
        result: Some(String::new()),
        errors: Vec::new(),
        api_error_status: None,
    };

    let message = failed.to_string();

    assert!(
        message.ends_with("and no reply: no reason given"),
        "{message}"
    );
}
