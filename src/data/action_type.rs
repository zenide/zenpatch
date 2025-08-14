//! Defines the type of action represented in a patch operation.
//!
//! Represents whether a patch file indicates adding, deleting, or updating a file.
//! Used within the PatchAction structure to categorize changes.
//! Derived traits support serialization, comparison, and debugging.
//! Conforms to the one-item-per-file rule.

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ActionType {
    Add,
    Delete,
    Update,
}

#[cfg(test)]
mod tests {
    // Access the enum under test via `super::`.
    // Use fully qualified paths for standard library items as per guidelines (e.g., assert!).

    #[test]
    fn test_action_type_variants_exist() {
        // Test instantiation of each variant.
        let add = super::ActionType::Add;
        let delete = super::ActionType::Delete;
        let update = super::ActionType::Update;

        // Basic check using debug format to ensure they are distinct enum variants.
        std::assert_eq!(std::format!("{:?}", add), "Add");
        std::assert_eq!(std::format!("{:?}", delete), "Delete");
        std::assert_eq!(std::format!("{:?}", update), "Update");
    }

    #[test]
    fn test_action_type_equality() {
        // Test equality and inequality comparisons.
        let add1 = super::ActionType::Add;
        let add2 = super::ActionType::Add;
        let delete = super::ActionType::Delete;

        std::assert_eq!(add1, add2); // Same variants should be equal.
        std::assert_ne!(add1, delete); // Different variants should not be equal.
    }

    #[test]
    fn test_action_type_cloning() {
        // Test cloning.
        let original = super::ActionType::Update;
        let cloned = original.clone();

        std::assert_eq!(original, cloned); // Cloned value should be equal to original.
    }
}
