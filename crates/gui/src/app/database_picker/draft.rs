//! What the person has typed into the fields of the sheet, and the row of the dropdown it was
//! opened from.

use std::path::PathBuf;

use choices::ApiKey;

use crate::contract::Database;

/// What a draft was opened from, which is also the row of the dropdown that shows it.
#[derive(Clone, PartialEq, Eq)]
pub(super) enum Row {
    Local,
    Saved(String),
    New,
}

/// The fields of the sheet, as typed.
pub(super) struct Draft {
    pub(super) row: Row,
    pub(super) name: String,
    pub(super) qdrant_url: String,
    pub(super) qdrant_api_key: String,
    pub(super) falkordb_url: String,
    pub(super) falkordb_password: String,
    pub(super) falkordb_graph: String,
    pub(super) content_folder: String,
}

impl Draft {
    pub(super) fn of(database: &Database, row: Row) -> Draft {
        let secret =
            |key: Option<&ApiKey>| key.map_or(String::new(), |key| key.expose().to_owned());
        Draft {
            row,
            name: database.name.clone(),
            qdrant_url: database.qdrant_url.clone(),
            qdrant_api_key: secret(database.qdrant_api_key.as_ref()),
            falkordb_url: database.falkordb_url.clone(),
            falkordb_password: secret(database.falkordb_password.as_ref()),
            falkordb_graph: database.falkordb_graph.clone(),
            content_folder: database.content_folder.to_string_lossy().into_owned(),
        }
    }

    pub(super) fn blank(graph: &str) -> Draft {
        Draft::of(
            &Database {
                falkordb_graph: graph.to_owned(),
                ..Database::default()
            },
            Row::New,
        )
    }

    /// Spaces around a pasted word are dropped, and an empty key is no key.
    pub(super) fn database(&self) -> Database {
        let key = |text: &str| {
            let text = text.trim();
            (!text.is_empty()).then(|| ApiKey::new(text))
        };
        Database {
            name: self.name.trim().to_owned(),
            qdrant_url: self.qdrant_url.trim().to_owned(),
            qdrant_api_key: key(&self.qdrant_api_key),
            falkordb_url: self.falkordb_url.trim().to_owned(),
            falkordb_password: key(&self.falkordb_password),
            falkordb_graph: self.falkordb_graph.trim().to_owned(),
            content_folder: PathBuf::from(self.content_folder.trim()),
        }
    }
}
