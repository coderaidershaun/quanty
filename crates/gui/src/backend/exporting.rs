//! The export of a document to a file. Both backends do it the same way: off the threads that
//! wait on the network, with how it ended sent as one event.

use export::ExportError;

use super::Reply;
use crate::contract::{Event, ExportFile, Failure, FailureKind, RequestId};

/// A PDF with pictures takes long enough to freeze the window, so the file is made on a thread
/// of its own. Exactly one `Exported` event ends the export.
pub(super) async fn write_file(request: RequestId, file: ExportFile, reply: &Reply) {
    let format = file.format;
    let written = tokio::task::spawn_blocking(move || {
        export::write(&file.document, file.format, &file.path).map(|()| file.path)
    })
    .await;
    let result = match written {
        Ok(Ok(path)) => Ok(path),
        Ok(Err(error)) => Err(failure(&error)),
        Err(error) => Err(Failure::internal(format!("the export stopped: {error}"))),
    };
    reply.send(Event::Exported {
        request,
        format,
        result,
    });
}

fn failure(error: &ExportError) -> Failure {
    match error {
        ExportError::Write { path, source } => {
            Failure::new(FailureKind::WriteFailed, format!("{error}: {source}")).with_hint(format!(
                "{} could not be written: {source}. Choose another folder, then export again.",
                path.display()
            ))
        }
        ExportError::Pdf { reason } => Failure::internal(error.to_string()).with_hint(format!(
            "The PDF could not be made: {reason}. Try again, or export as HTML."
        )),
    }
}
