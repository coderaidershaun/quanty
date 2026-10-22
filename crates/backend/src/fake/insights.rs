//! The saved insights of a sample chapter as the fake reads them: the `insights.json` in its
//! folder, when there is one.

use std::io::ErrorKind;
use std::path::Path;

use serde::Deserialize;

use super::{Fake, PAGE_WAIT, Pages};
use crate::Reply;
use contract::{ChapterInsights, DocId, Event, Failure, Insight, RequestId};

const INSIGHTS_FILE: &str = "insights.json";

impl Fake {
    /// Answers as the live backend does: a document with no folder is a failure, and a folder with
    /// no file holds no insights yet. The folder is the one the fake's own catalogue lists.
    pub(super) async fn load_chapter_insights(
        &self,
        request: RequestId,
        doc: DocId,
        reply: &Reply,
    ) {
        let folder = self
            .catalogue()
            .document(doc)
            .and_then(|document| document.folder.clone());
        self.wait(PAGE_WAIT).await;
        let result = match self.scene.script.pages {
            Pages::SourceMissing => Err(self
                .source_missing("the scene says this document's files are not there".to_owned())),
            Pages::Samples => match folder {
                Some(folder) => read_insights(doc, &folder),
                None => {
                    Err(self.source_missing(format!("the sample document {} has no folder", doc.0)))
                }
            },
        };
        reply.send(Event::ChapterInsights { request, result });
    }
}

fn read_insights(doc: DocId, folder: &Path) -> Result<Option<ChapterInsights>, Failure> {
    let path = folder.join(INSIGHTS_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            let detail = format!("could not read {}: {error}", path.display());
            return Err(ChapterInsights::unreadable(doc, detail));
        }
    };
    let file: InsightsFile = serde_json::from_str(&text).map_err(|error| {
        let detail = format!(
            "{} is not valid json of the expected shape: {error}",
            path.display()
        );
        ChapterInsights::unreadable(doc, detail)
    })?;
    Ok(Some(file.into()))
}

// SMELL: the converter keeps its own reader of this file, and the fake may not name that crate,
// so a change to the saved format must be made in both.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct InsightsFile {
    model: String,
    insights: Vec<InsightFile>,
    takeaways: Vec<String>,
    cheat_sheet: String,
}

#[derive(Deserialize)]
struct InsightFile {
    text: String,
    pages: Vec<u32>,
}

impl From<InsightsFile> for ChapterInsights {
    fn from(file: InsightsFile) -> ChapterInsights {
        ChapterInsights {
            model: file.model,
            insights: file
                .insights
                .into_iter()
                .map(|insight| Insight {
                    text: insight.text,
                    pages: insight.pages,
                })
                .collect(),
            takeaways: file.takeaways,
            cheat_sheet: file.cheat_sheet,
        }
    }
}
