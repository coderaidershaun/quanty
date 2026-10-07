//! Tests of `rag-core` that go through its public interface, with a stand-in server, a stand-in
//! `claude` program, or the real ones.

mod claude_cli;
mod claude_live;
mod concept_store;
mod gemini_live;
mod gemini_stub;
mod gemini_wire;
mod item_store;
mod throwaway;
