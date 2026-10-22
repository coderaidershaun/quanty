//! Turns a chapter PDF, a converted chapter, or a picture that stands alone, into stored,
//! searchable items and a graph of their concepts, and can relabel a stored media or document,
//! delete a stored document or a whole media, and say what the two API keys can do.

mod delete;
pub mod health;
mod ingest;
mod keys;
mod labels;
mod media;
mod pdf;
mod stores;
#[cfg(feature = "testing")]
pub mod testing;
mod uploads;

pub use delete::{
    DeleteError, DocumentDeleteSummary, MediaDeleteSummary, delete_document, delete_media,
};
pub use ingest::{
    ASK_SCORE, ArtifactAssessment, ArtifactError, ArtifactState, ArtifactSummary, ArtifactWriter,
    Cache as AnswerCache, ChapterChoice, ChapterFolder, ConceptError, ConceptExtractor,
    ConceptSummary, IngestError, IngestStep, IngestSummary, InsightsNeed, InsightsOutcome,
    InsightsSummary, InsightsWriter, Item, ItemCounts, LINK_SCORE, LoneImage, Models, SkippedItem,
    StoredChapter, StoredChaptersError, assess_artifacts, chapter_items, context_line,
    document_title, image_items, ingest_chapter, ingest_image, stored_chapters, usage_of,
};
pub use keys::{KEY_CHECK_LIMIT, KeyClients, KeyReadFailure, KeyState, KeyStates, read_key_states};
pub use labels::{RelabelError, Relabelled, TagChange, relabel_document_tags};
pub use media::{MediaChange, MediaRelabelled, relabel_media};
pub use pdf::{
    ChapterPdf, NamePdfError, NamedPdf, PdfError, PdfOutcome, PdfSummary, UnnamedPdf, ingest_pdf,
    items_of_ingested_document,
};
pub use stores::Stores;

pub use uploads::{UploadNameError, check_upload_file_name, save_uploaded_pdf, uploaded_pdf_path};
