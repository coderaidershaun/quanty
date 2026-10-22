//! The databases the fake knows, and its answer to using one: the answer its catalogue gives.

use std::path::Path;

use choices::LOCAL_DATABASE;

use super::scenes::Choice;
use super::{FALKORDB_URL, Fake, QDRANT_URL};
use crate::backend::Reply;
use crate::contract::{Database, Databases, Event, RequestId};

impl Fake {
    /// The database called Local, as the stores and the folders of a home describe it.
    pub fn local_database(home: &Path) -> Database {
        Database {
            name: LOCAL_DATABASE.to_owned(),
            qdrant_url: QDRANT_URL.to_owned(),
            falkordb_url: FALKORDB_URL.to_owned(),
            falkordb_graph: "quanty".to_owned(),
            content_folder: home.join("content"),
            ..Database::default()
        }
    }

    /// The saved choices the scene starts with.
    pub fn databases(&self) -> Databases {
        match self.scene.script.database {
            Choice::Local => Databases::choosing_local(),
            Choice::Nothing => Databases::default(),
        }
    }

    /// Answers as the catalogue does, so a scene whose library fails is the scene whose stores
    /// are down.
    pub(super) async fn use_database(&self, request: RequestId, reply: &Reply) {
        let result = self.after_wait(|| Ok(())).await;
        reply.send(Event::DatabaseUsed { request, result });
    }
}
