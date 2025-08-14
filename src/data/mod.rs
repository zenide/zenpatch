//! Contains all data structures used for representing patches and their components.
//!
//! This module defines the core building blocks for patch representation,
//! including `Patch`, `PatchAction`, `Chunk`, and `FileChange`. These structures
//! are designed to be serializable and mirror the logic from the reference
//! TypeScript implementation, adapted to Rust's type system and coding standards.
pub mod action_type;
pub mod chunk;
pub mod line_type;
pub mod patch_action;
