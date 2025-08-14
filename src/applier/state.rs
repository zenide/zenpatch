//! Defines the state used during the backtracking patch application process.
//!
//! This struct holds information about the current progress of the search for a valid
//! patch application sequence, including which chunks have been applied and which
//! original file lines have been affected. Conforms to rust_guidelines.

/// Represents the state of the backtracking search for applying patch chunks.
#[derive(Debug, Clone)]
pub struct BacktrackingState {
    /// The index in the original file lines from where to start searching for the next chunk's context.
    pub current_line_index: usize,
    /// Set of indices of the patch chunks that have been successfully applied in the current path.
    pub applied_chunks: std::collections::HashSet<usize>,
    /// Set of original line indices that have been affected (context matched, deleted) by applied chunks.
    pub modified_indices: std::collections::HashSet<usize>,
    /// Counter for the number of *distinct* final results found. Used to detect ambiguity.
    pub solution_count: usize,
    /// The first unique resulting file after applying all chunks (distinct results).
    pub first_solution_result: std::option::Option<std::vec::Vec<String>>,
    /// Optional: Tracks one sequence of (chunk index, match position) pairs for the first solution.
    /// Not used for distinctness detection but can reconstruct order if needed.
    pub solution_path: std::option::Option<std::vec::Vec<(usize, usize)>>,
}

impl BacktrackingState {
    /// Creates a new initial state for the backtracking algorithm.
    pub fn new() -> Self {
        Self {
            current_line_index: 0,
            applied_chunks: std::collections::HashSet::new(),
            modified_indices: std::collections::HashSet::new(),
            solution_count: 0,
            first_solution_result: std::option::Option::None,
            solution_path: std::option::Option::None,
        }
    }
}

// No tests defined here as it's a simple data structure.
// Tests involving state will be in the main backtracking_patcher tests.
