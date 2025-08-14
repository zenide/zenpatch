//! Defines the structure representing a single action within a patch.
//!
//! This struct encapsulates the details of a file operation described in a patch,
//! such as adding, updating, or deleting a file. It includes the type of action,
//! potential new file path (for additions/renames), change chunks (for updates),
//! and optional move path (for renames). Corresponds to the TypeScript `PatchAction`.
//! Conforms to the one-item-per-file rule and uses fully qualified paths.

/// Represents a single file operation derived from a patch.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PatchAction {
    /// The type of action (Add, Delete, Update).
    pub type_: crate::data::action_type::ActionType,
    /// The primary file path associated with the action.
    /// For `Add`, this is the path of the new file.
    /// For `Delete`, this is the path of the file to delete.
    /// For `Update`, this is the path of the file to update.
    pub path: std::string::String,
    /// The destination path for a move/rename operation. Only used with `Update`.
    pub new_path: std::option::Option<std::string::String>,
    /// The list of changes (hunks) to apply for an `Update` or `Add` action.
    pub chunks: std::vec::Vec<crate::data::chunk::Chunk>,
}

impl PatchAction {
    pub fn new(action_type: crate::data::action_type::ActionType, path: std::string::String) -> Self {
        Self {
            type_: action_type,
            path,
            new_path: std::option::Option::None,
            chunks: std::vec::Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    // Access struct and types via `super::` and fully qualified paths.

    #[test]
    fn test_patch_action_add() {
        // Test creating an 'Add' action.
        let action = super::PatchAction {
            type_: crate::data::action_type::ActionType::Add,
            path: std::string::String::from("new/path/file.txt"),
            chunks: std::vec::Vec::new(), // Typically empty or has only insertions for Add
            new_path: std::option::Option::None,
        };

        std::assert_eq!(action.type_, crate::data::action_type::ActionType::Add);
        std::assert_eq!(action.path, "new/path/file.txt");
        std::assert!(action.chunks.is_empty());
        std::assert!(action.new_path.is_none());
    }

   #[test]
   fn test_patch_action_update() {
       // Test creating an 'Update' action with chunks.
        let chunk = crate::data::chunk::Chunk {
            orig_index: 5,
            lines: std::vec![(crate::data::line_type::LineType::Deletion, std::string::String::from("old line")),
                  (crate::data::line_type::LineType::Insertion, std::string::String::from("new line"))],
            del_lines: std::vec![std::string::String::from("old line")],
            ins_lines: std::vec![std::string::String::from("new line")],
        };
        let action = super::PatchAction {
            type_: crate::data::action_type::ActionType::Update,
            path: "file.txt".to_string(),
            chunks: std::vec![chunk],
            new_path: std::option::Option::None,
        };

        std::assert_eq!(action.type_, crate::data::action_type::ActionType::Update);
        std::assert_eq!(action.chunks.len(), 1);
        std::assert!(action.new_path.is_none());
    }

    #[test]
    fn test_patch_action_delete() {
        // Test creating a 'Delete' action.
        let action = super::PatchAction {
            type_: crate::data::action_type::ActionType::Delete,
            path: "file_to_delete.txt".to_string(),
            chunks: std::vec::Vec::new(), // Typically empty or has only deletions for Delete
            new_path: std::option::Option::None,
        };

        std::assert_eq!(action.type_, crate::data::action_type::ActionType::Delete);
        std::assert_eq!(action.path, "file_to_delete.txt");
        std::assert!(action.chunks.is_empty());
        std::assert!(action.new_path.is_none());
    }

    #[test]
   fn test_patch_action_update_with_move() {
       // Test an 'Update' action that also represents a rename/move.
         let chunk = crate::data::chunk::Chunk {
            orig_index: 1,
            lines: std::vec![(crate::data::line_type::LineType::Insertion, std::string::String::from("added line"))],
            del_lines: std::vec::Vec::new(),
            ins_lines: std::vec![std::string::String::from("added line")],
        };
        let action = super::PatchAction {
            type_: crate::data::action_type::ActionType::Update, // Or could be Add depending on patch format interpretation for moves
            path: "old/location.txt".to_string(),
            new_path: std::option::Option::Some(std::string::String::from("new/location.txt")),
            chunks: std::vec![chunk],
        };

        std::assert_eq!(action.type_, crate::data::action_type::ActionType::Update);
        std::assert_eq!(action.path, "old/location.txt");
        std::assert_eq!(action.new_path.as_deref(), std::option::Option::Some("new/location.txt"));
        std::assert_eq!(action.chunks.len(), 1);
    }

    #[test]
    fn test_patch_action_clone_and_equality() {
        // Test cloning and equality.
       let action1 = super::PatchAction {
            type_: crate::data::action_type::ActionType::Update,
            path: "file.rs".to_string(),
            new_path: std::option::Option::None,
            chunks: std::vec![crate::data::chunk::Chunk {
                orig_index: 1,
                lines: std::vec![(crate::data::line_type::LineType::Insertion, std::string::String::from("a"))],
                del_lines: std::vec::Vec::new(),
                ins_lines: std::vec![std::string::String::from("a")],
            }],
        };
        let action2 = action1.clone();
        let action3 = super::PatchAction {
            type_: crate::data::action_type::ActionType::Add, // Different type
            path: "file.txt".to_string(),
            new_path: std::option::Option::None,
            chunks: std::vec![],
        };

        std::assert_eq!(action1, action2); // Cloned should be equal
        std::assert_ne!(action1, action3); // Different actions should not be equal
    }
}
