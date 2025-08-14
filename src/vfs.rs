//! Defines the Virtual File System (VFS) type alias.
//!
//! The VFS is represented as a `HashMap` where keys are file paths (as `String`)
//! and values are the file contents (as `String`). This allows for in-memory
//! patch application and testing without accessing the physical file system.
//! Follows the one-item-per-file guideline.

pub type Vfs = std::collections::HashMap<std::string::String, std::string::String>;