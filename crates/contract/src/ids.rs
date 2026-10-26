//! The ids that name a request, a notice, and the documents, items and concepts the stores hold.

use uuid::Uuid;

macro_rules! id {
    ($(#[$doc:meta])* $name:ident($inner:ty)) => {
        $(#[$doc])*
        #[derive(
            serde::Serialize,
            serde::Deserialize,
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            Hash,
            PartialOrd,
            Ord,
            Default,
        )]
        pub struct $name(pub $inner);
    };
}

id!(
    /// Issued by the shared state, counting from 1.
    RequestId(u64)
);
id!(
    /// Issued by the shared state, counting from 1.
    NoticeId(u64)
);
id!(DocId(Uuid));
id!(ItemId(Uuid));
id!(ConceptId(Uuid));
id!(
    /// A PDF kept on the server until its ingest begins.
    UploadId(Uuid)
);
