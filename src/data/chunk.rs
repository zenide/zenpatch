//! Defines the structure representing a hunk or chunk within a patch action.
//!
//! A chunk details a specific change within a file update, including the
//! Mirrors the `Chunk` interface from the TypeScript reference implementation conceptually.
//! Conforms to the one-item-per-file rule and uses fully qualified paths.

/// Represents a single contiguous block of changes (context/additions/deletions) within a file patch.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Chunk {
    /// The line index in the original file where this chunk's changes apply.
    /// Note: This corresponds to the line number before the first deletion or insertion.
    pub orig_index: usize,
    /// Structured lines with type and content
    pub lines: std::vec::Vec<(crate::data::line_type::LineType, std::string::String)>,
    /// Lines to be deleted. Populated by the parser.
    pub del_lines: std::vec::Vec<std::string::String>,
    /// Lines to be inserted. Populated by the parser.
    pub ins_lines: std::vec::Vec<std::string::String>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            orig_index: 0,
            lines: std::vec::Vec::new(),
            del_lines: std::vec::Vec::new(),
            ins_lines: std::vec::Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    // Access struct and types via `super::` and fully qualified paths.

    #[test]
    fn test_chunk_creation_empty() {
        // Test creating an empty Chunk.
        let chunk = super::Chunk {
            orig_index: 0,
            lines: std::vec::Vec::new(),
            del_lines: std::vec::Vec::new(),
            ins_lines: std::vec::Vec::new(),
        };
        std::assert_eq!(chunk.orig_index, 0);
        std::assert!(chunk.lines.is_empty());
        std::assert!(chunk.del_lines.is_empty());
        std::assert!(chunk.ins_lines.is_empty());
    }

    #[test]
    fn test_chunk_creation_with_data() {
        // Test creating a Chunk with context, deletion, and insertion lines.
        let lines_data = std::vec![
            (crate::data::line_type::LineType::Context, std::string::String::from("context line 1")),
            (crate::data::line_type::LineType::Deletion, std::string::String::from("line to delete")),
            (crate::data::line_type::LineType::Insertion, std::string::String::from("new line 1")),
            (crate::data::line_type::LineType::Insertion, std::string::String::from("new line 2")),
            (crate::data::line_type::LineType::Context, std::string::String::from("context line 2")),
        ];
        let del_lines_data = std::vec![std::string::String::from("line to delete")];
        let ins_lines_data = std::vec![
            std::string::String::from("new line 1"),
            std::string::String::from("new line 2"),
        ];

        let chunk = super::Chunk {
            orig_index: 10,
            lines: lines_data.clone(), // Clone for comparison
            del_lines: del_lines_data.clone(),
            ins_lines: ins_lines_data.clone(),
        };

        std::assert_eq!(chunk.orig_index, 10);
        std::assert_eq!(chunk.lines.len(), 5);
        std::assert_eq!(chunk.lines, lines_data);
        std::assert_eq!(chunk.del_lines, del_lines_data);
        std::assert_eq!(chunk.ins_lines, ins_lines_data);

        // Verify specific line types
        std::assert_eq!(chunk.lines[0].0, crate::data::line_type::LineType::Context);
        std::assert_eq!(chunk.lines[1].0, crate::data::line_type::LineType::Deletion);
        std::assert_eq!(chunk.lines[2].0, crate::data::line_type::LineType::Insertion);
        std::assert_eq!(chunk.lines[3].0, crate::data::line_type::LineType::Insertion);
        std::assert_eq!(chunk.lines[4].0, crate::data::line_type::LineType::Context);
    }

     #[test]
    fn test_chunk_equality_and_clone() {
        // Test cloning and equality comparison.
        let chunk1 = super::Chunk {
            orig_index: 5,
            lines: std::vec![(crate::data::line_type::LineType::Context, std::string::String::from("a"))],
            del_lines: std::vec::Vec::new(),
            ins_lines: std::vec::Vec::new(),
        };
        let chunk2 = chunk1.clone(); // Clone
        let chunk3 = super::Chunk {
            orig_index: 6, // Different index
            lines: std::vec![(crate::data::line_type::LineType::Context, std::string::String::from("a"))],
            del_lines: std::vec::Vec::new(),
            ins_lines: std::vec::Vec::new(),
        };
         let chunk4 = super::Chunk {
            orig_index: 5,
            lines: std::vec![(crate::data::line_type::LineType::Deletion, std::string::String::from("a"))], // Different line type
            del_lines: std::vec![std::string::String::from("a")],
            ins_lines: std::vec::Vec::new(),
        };


        std::assert_eq!(chunk1, chunk2); // Cloned should be equal
        std::assert_ne!(chunk1, chunk3); // Different index should not be equal
        std::assert_ne!(chunk1, chunk4); // Different line type should not be equal
    }
}
