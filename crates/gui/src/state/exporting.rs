//! The one export the person can have at a time: the save dialog asked for, the file written, and
//! the notice that says how it ended.

use std::path::PathBuf;

use export::Format;

use super::shared::Shared;
use crate::contract::{
    Command, Effect, ExportFile, ExportPlace, ExportRequest, Failure, Notice, NoticeId, NoticeKind,
    RequestId,
};

/// The export that is being written or that ended last, and where in the window it was asked.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct ExportJob {
    pub place: ExportPlace,
    pub state: ExportState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ExportState {
    /// The backend is writing the file and its event is still to come.
    Writing(RequestId),
    /// The notice that says how it ended.
    Ended(NoticeId),
}

impl Shared {
    pub fn is_exporting(&self) -> bool {
        matches!(
            self.export,
            Some(ExportJob {
                state: ExportState::Writing(_),
                ..
            })
        )
    }

    /// The notice of the last export that was asked from `place`, while the notice is still kept.
    pub fn export_notice(&self, place: ExportPlace) -> Option<&Notice> {
        let Some(ExportJob {
            place: asked,
            state: ExportState::Ended(id),
        }) = &self.export
        else {
            return None;
        };
        if *asked != place {
            return None;
        }
        self.notices.iter().find(|notice| notice.id == *id)
    }

    pub(super) fn ask_where_to_export(&mut self, export: ExportRequest, effects: &mut Vec<Effect>) {
        if !self.is_exporting() {
            effects.push(Effect::PickSavePath(export));
        }
    }

    pub(super) fn start_export(
        &mut self,
        export: ExportRequest,
        path: PathBuf,
        effects: &mut Vec<Effect>,
    ) {
        if self.is_exporting() {
            return;
        }
        let request = self.issue_request();
        self.export = Some(ExportJob {
            place: export.place,
            state: ExportState::Writing(request),
        });
        effects.push(Effect::Send(Command::Export {
            request,
            file: ExportFile {
                format: export.format,
                document: export.document,
                path,
            },
        }));
    }

    pub(super) fn export_finished(
        &mut self,
        request: RequestId,
        format: Format,
        result: Result<PathBuf, Failure>,
    ) {
        let place = match &self.export {
            Some(ExportJob {
                place,
                state: ExportState::Writing(waiting),
            }) if *waiting == request => *place,
            _ => return,
        };
        let what = place.what();
        let name = format.name();
        let notice = match result {
            Ok(path) => self.add_notice(
                NoticeKind::Done,
                format!("Exported {what} as {name}"),
                format!("Saved to {}.", path.display()),
                None,
            ),
            Err(failure) => self.add_notice(
                NoticeKind::Failed,
                format!("Could not export {what} as {name}"),
                failure.hint.clone(),
                Some(failure),
            ),
        };
        self.export = Some(ExportJob {
            place,
            state: ExportState::Ended(notice),
        });
    }
}
