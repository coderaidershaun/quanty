//! The reads that take one converted document as a whole: its outline and overview, its sections,
//! pages and items, and several of these reads in one call.

mod items;
mod many;
mod mentions;
mod outline;
mod overview;
mod sections;

pub(crate) use items::{ItemRead, ReadItemArgs, read_item};
pub(crate) use many::{ManyRead, ReadManyArgs, read_many};
pub(crate) use outline::{DocumentOutlineArgs, Outline, document_outline};
pub(crate) use overview::{DocumentOverviewArgs, Overview, document_overview};
pub(crate) use sections::{
    PagesView, ReadPagesArgs, ReadSectionArgs, SectionRef, SectionView, items_of_section,
    read_pages, read_section, sections_of_items,
};
