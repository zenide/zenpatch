//! Implements the main `apply` function for the zenpatch crate.
//!
//! This file provides the public entry point for applying a patch to content.
//! It orchestrates parsing the patch and then applying it based on the action
//! type (Add, Update, Delete), handling different whitespace modes and retries.
//! Conforms to rust coding guidelines (one item per file).

/// Applies a text-based patch to original content and returns the new content.
///
/// This is the primary public API for the `zenpatch` crate. It handles patch
/// parsing and application, including retry logic with lenient whitespace matching
/// for updates.
///
/// # Arguments
///
/// * `patch_text` - A string slice containing the patch in the expected format.
/// * `original_content` - A string slice of the original content to be patched.
///
/// # Returns
///
/// * `Ok(String)` - The patched content on success.
/// * `Err(ZenpatchError)` - An error if parsing or application fails.
pub fn apply(
    patch_text: &str,
    original_content: &str,
) -> std::result::Result<std::string::String, crate::error::ZenpatchError> {
    let action = crate::parser::text_to_patch::text_to_patch(patch_text)?;

    match action.type_ {
        crate::data::action_type::ActionType::Update => {
            let original_lines: std::vec::Vec<std::string::String> =
                original_content.lines().map(std::string::String::from).collect();

            // First, try with strict whitespace matching.
            let result = crate::applier::backtracking_patcher::apply_patch_backtracking_mode(
                &original_lines,
                &action.chunks,
                crate::applier::whitespace_mode::WhitespaceMode::Strict,
            );

            // If it fails with a conflict or ambiguity, retry with lenient whitespace matching.
            match result {
                std::result::Result::Err(crate::error::ZenpatchError::PatchConflict(_))
                | std::result::Result::Err(crate::error::ZenpatchError::AmbiguousPatch(_)) => {
                    let lenient_result = crate::applier::backtracking_patcher::apply_patch_backtracking_mode(
                        &original_lines,
                        &action.chunks,
                        crate::applier::whitespace_mode::WhitespaceMode::Lenient,
                    );
                    match lenient_result {
                        std::result::Result::Ok(applied_lines) => std::result::Result::Ok(applied_lines.join("\n")),
                        std::result::Result::Err(e) => std::result::Result::Err(e),
                    }
                }
                std::result::Result::Ok(applied_lines) => std::result::Result::Ok(applied_lines.join("\n")),
                std::result::Result::Err(e) => std::result::Result::Err(e),
            }
        }
        crate::data::action_type::ActionType::Add => {
            if !original_content.is_empty() {
                return std::result::Result::Err(crate::error::ZenpatchError::InvalidPatchFormat(
                    "Cannot 'Add' to non-empty content.".to_string(),
                ));
            }
            let content: std::vec::Vec<std::string::String> = action
                .chunks
                .iter()
                .flat_map(|c| c.ins_lines.clone())
                .collect();
            std::result::Result::Ok(content.join("\n"))
        }
        crate::data::action_type::ActionType::Delete => {
            let content_to_delete: std::vec::Vec<std::string::String> = action
                .chunks
                .iter()
                .flat_map(|c| c.del_lines.clone())
                .collect();
            
            let original_lines: std::vec::Vec<std::string::String> = original_content.lines().map(std::string::String::from).collect();

            if content_to_delete == original_lines {
                std::result::Result::Ok(std::string::String::new())
            } else {
                std::result::Result::Err(crate::error::ZenpatchError::PatchConflict(
                    "Content to delete does not match original content.".to_string(),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // Note: Comprehensive tests would require setting up various patch strings
    // and original content, which can be extensive. These are basic sanity checks.

    #[test]
    fn test_apply_add_simple() {
        let patch = "*** Begin Patch\n*** Add File: new.txt\n+hello\n+world\n*** End Patch";
        let original = "";
        let result = super::apply(patch, original);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello\nworld");
    }

    #[test]
    fn test_apply_add_to_existing_fails() {
        let patch = "*** Begin Patch\n*** Add File: new.txt\n+hello\n*** End Patch";
        let original = "i already exist";
        let result = super::apply(patch, original);
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::ZenpatchError::InvalidPatchFormat(msg) => {
                assert!(msg.contains("non-empty content"));
            }
            _ => panic!("Expected InvalidPatchFormat error"),
        }
    }

    #[test]
    fn test_apply_delete_simple() {
        let patch = "*** Begin Patch\n*** Delete File: old.txt\n-line1\n-line2\n*** End Patch";
        let original = "line1\nline2";
        let result = super::apply(patch, original);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_apply_delete_mismatch_fails() {
        let patch = "*** Begin Patch\n*** Delete File: old.txt\n-line1\n*** End Patch";
        let original = "different content";
        let result = super::apply(patch, original);
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::ZenpatchError::PatchConflict(msg) => {
                assert!(msg.contains("does not match"));
            }
            _ => panic!("Expected PatchConflict error"),
        }
    }

    #[test]
    fn test_apply_update_simple() {
        let patch = "*** Begin Patch\n*** Update File: a.txt\n@@\n-a\n+b\n*** End Patch";
        let original = "a";
        let result = super::apply(patch, original);
        assert!(result.is_ok(), "Update failed: {:?}", result);
        assert_eq!(result.unwrap(), "b");
    }

    #[test]
    fn test_apply_update_with_context() {
        let patch = "*** Begin Patch\n*** Update File: a.txt\n@@\n c\n-a\n+b\n d\n*** End Patch";
        let original = "c\na\nd";
        let result = super::apply(patch, original);
        assert!(result.is_ok(), "Update failed: {:?}", result);
        assert_eq!(result.unwrap(), "c\nb\nd");
    }
}