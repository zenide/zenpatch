//! Defines the state used during the backtracking patch application process.
//!
//! This struct holds information about the current progress of the search for a valid
//! patch application sequence, including which chunks have been applied and which
//! original file lines have been affected. The search mutates ONE shared instance
//! in place (do/undo on enter/exit) instead of cloning per node. Conforms to
//! rust_guidelines.

/// Represents the state of the backtracking search for applying patch chunks.
#[derive(Debug, Clone)]
pub struct BacktrackingState {
    /// Set of indices of the patch chunks applied on the current search path.
    pub applied_chunks: std::collections::HashSet<usize>,
    /// Set of original line indices affected (deleted) by applied chunks.
    pub modified_indices: std::collections::HashSet<usize>,
    /// Counter for the number of *distinct* final results found. Used to detect ambiguity.
    pub solution_count: usize,
    /// The first unique resulting file after applying all chunks (distinct results).
    pub first_solution_result: std::option::Option<std::vec::Vec<String>>,
    /// One sequence of (chunk index, match position) pairs for the first solution.
    pub solution_path: std::option::Option<std::vec::Vec<(usize, usize)>>,
    /// Canonical key of the first solution: (position, chunk content class)
    /// sorted by position. Mappings with equal keys yield identical files,
    /// so they are deduplicated without materializing the result.
    pub first_solution_key: std::option::Option<std::vec::Vec<(usize, usize)>>,
    /// Number of search nodes visited; the search aborts as ambiguous past a cap.
    pub nodes_visited: usize,
}

impl BacktrackingState {
    /// Creates a new initial state for the backtracking algorithm.
    pub fn new() -> Self {
        Self {
            applied_chunks: std::collections::HashSet::new(),
            modified_indices: std::collections::HashSet::new(),
            solution_count: 0,
            first_solution_result: std::option::Option::None,
            solution_path: std::option::Option::None,
            first_solution_key: std::option::Option::None,
            nodes_visited: 0,
        }
    }
}

impl std::default::Default for BacktrackingState {
    fn default() -> Self {
        Self::new()
    }
}

// No tests defined here as it's a simple data structure.
// Tests involving state will be in the main backtracking_patcher tests.
