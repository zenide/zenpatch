//! Implements the main `apply` function for the zenpatch crate.
//!
//! This file provides the public entry point for applying a patch to content.
//! It orchestrates parsing the patch and then applying it based on the action
//! type (Add, Update, Delete), handling different whitespace modes and retries.
//! Conforms to rust coding guidelines (one item per file).

/// Applies a text-based patch to a Virtual File System (VFS) and returns the new VFS.
///
/// This is the primary public API for the `zenpatch` crate. It handles patch
/// parsing and application for multiple file operations within a single patch.
///
/// # Arguments
///
/// * `patch_text` - A string slice containing the patch in the expected format.
/// * `vfs` - A reference to the initial Virtual File System.
///
/// # Returns
///
/// * `Ok(Vfs)` - The patched VFS on success.
/// * `Err(ZenpatchError)` - An error if parsing or application fails.
pub fn apply(
    patch_text: &str,
    vfs: &crate::vfs::Vfs,
) -> std::result::Result<crate::vfs::Vfs, crate::error::ZenpatchError> {
    let mut new_vfs = vfs.clone();
    let actions = crate::parser::text_to_patch::text_to_patch(patch_text)?;

    for action in actions {
        match action.type_ {
            crate::data::action_type::ActionType::Update => {
                let original_content = new_vfs
                    .get(&action.path)
                    .ok_or_else(|| crate::error::ZenpatchError::FileNotFound(action.path.clone()))?;

                let original_lines: std::vec::Vec<std::string::String> =
                    original_content.lines().map(std::string::String::from).collect();

                // First, try with strict whitespace matching.
                let result = crate::applier::backtracking_patcher::apply_patch_backtracking_mode(
                    &original_lines,
                    &action.chunks,
                    crate::applier::whitespace_mode::WhitespaceMode::Strict,
                );

                // If it fails with a conflict or ambiguity, retry with lenient whitespace matching.
                let applied_lines = match result {
                    Err(crate::error::ZenpatchError::PatchConflict(_))
                    | Err(crate::error::ZenpatchError::AmbiguousPatch(_)) => {
                        crate::applier::backtracking_patcher::apply_patch_backtracking_mode(
                            &original_lines,
                            &action.chunks,
                            crate::applier::whitespace_mode::WhitespaceMode::Lenient,
                        )?
                    }
                    Ok(lines) => lines,
                    Err(e) => return Err(e),
                };
                let updated_content = applied_lines.join("\n");

                if let Some(new_path) = &action.new_path {
                    // Handle rename
                    new_vfs.remove(&action.path);
                    new_vfs.insert(new_path.clone(), updated_content);
                } else {
                    new_vfs.insert(action.path.clone(), updated_content);
                }
            }
            crate::data::action_type::ActionType::Add => {
                if new_vfs.contains_key(&action.path) {
                    return std::result::Result::Err(crate::error::ZenpatchError::FileExists(
                        action.path.clone(),
                    ));
                }
                let content: std::vec::Vec<std::string::String> = action
                    .chunks
                    .iter()
                    .flat_map(|c| c.ins_lines.clone())
                    .collect();
                new_vfs.insert(action.path.clone(), content.join("\n"));
            }
            crate::data::action_type::ActionType::Delete => {
                let original_content = new_vfs
                    .get(&action.path)
                    .ok_or_else(|| crate::error::ZenpatchError::FileNotFound(action.path.clone()))?;

                let content_to_delete: std::vec::Vec<std::string::String> = action
                    .chunks
                    .iter()
                    .flat_map(|c| c.del_lines.clone())
                    .collect();

                let original_lines: std::vec::Vec<std::string::String> =
                    original_content.lines().map(std::string::String::from).collect();

                if content_to_delete == original_lines {
                    new_vfs.remove(&action.path);
                } else {
                    return std::result::Result::Err(crate::error::ZenpatchError::PatchConflict(
                        "Content to delete does not match original content.".to_string(),
                    ));
                }
            }
        }
    }

    std::result::Result::Ok(new_vfs)
}

#[cfg(test)]
mod tests {
    // Note: VFS-based tests.
    use crate::vfs::Vfs;

    fn vfs_from_str(path: &str, content: &str) -> Vfs {
        let mut vfs = Vfs::new();
        vfs.insert(path.to_string(), content.to_string());
        vfs
    }

    #[test]
    fn test_apply_add_simple() {
        let patch = "*** Begin Patch\n*** Add File: new.txt\n+hello\n+world\n*** End Patch";
        let vfs = Vfs::new();
        let result_vfs = super::apply(patch, &vfs).unwrap();
        assert_eq!(result_vfs.get("new.txt").unwrap(), "hello\nworld");
    }

    #[test]
    fn test_apply_add_to_existing_fails() {
        let patch = "*** Begin Patch\n*** Add File: new.txt\n+hello\n*** End Patch";
        let vfs = vfs_from_str("new.txt", "i already exist");
        let result = super::apply(patch, &vfs);
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::ZenpatchError::FileExists(path) => {
                assert_eq!(path, "new.txt");
            }
            _ => panic!("Expected FileExists error"),
        }
    }

    #[test]
    fn test_apply_delete_simple() {
        let patch = "*** Begin Patch\n*** Delete File: old.txt\n-line1\n-line2\n*** End Patch";
        let vfs = vfs_from_str("old.txt", "line1\nline2");
        let result_vfs = super::apply(patch, &vfs).unwrap();
        assert!(result_vfs.get("old.txt").is_none());
        assert!(result_vfs.is_empty());
    }

    #[test]
    fn test_apply_delete_mismatch_fails() {
        let patch = "*** Begin Patch\n*** Delete File: old.txt\n-line1\n*** End Patch";
        let vfs = vfs_from_str("old.txt", "different content");
        let result = super::apply(patch, &vfs);
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::ZenpatchError::PatchConflict(msg) => {
                assert!(msg.contains("does not match"));
            }
            _ => panic!("Expected PatchConflict error"),
        }
    }

    #[test]
    fn test_apply_delete_file_not_found() {
        let patch = "*** Begin Patch\n*** Delete File: old.txt\n-line1\n*** End Patch";
        let vfs = Vfs::new();
        let result = super::apply(patch, &vfs);
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::ZenpatchError::FileNotFound(path) => {
                assert_eq!(path, "old.txt");
            }
            _ => panic!("Expected FileNotFound error"),
        }
    }

    #[test]
    fn test_apply_update_simple() {
        let patch = "*** Begin Patch\n*** Update File: a.txt\n@@\n-a\n+b\n*** End Patch";
        let vfs = vfs_from_str("a.txt", "a");
        let result_vfs = super::apply(patch, &vfs).unwrap();
        assert_eq!(result_vfs.get("a.txt").unwrap(), "b");
    }

    #[test]
    fn test_apply_update_with_rename() {
        let patch =
            "*** Begin Patch\n*** Update File: a.txt\n*** Move to: b.txt\n@@\n-a\n+b\n*** End Patch";
        let vfs = vfs_from_str("a.txt", "a");
        let result_vfs = super::apply(patch, &vfs).unwrap();
        assert!(result_vfs.get("a.txt").is_none());
        assert_eq!(result_vfs.get("b.txt").unwrap(), "b");
    }

    #[test]
    fn test_apply_multiple_actions() {
        let patch = "*** Begin Patch\n\
*** Add File: new.txt\n+new content\n\
*** Update File: a.txt\n@@\n-a\n+b\n\
*** Delete File: old.txt\n-old\n\
*** End Patch";
        let mut vfs = vfs_from_str("a.txt", "a");
        vfs.insert("old.txt".to_string(), "old".to_string());

        let result_vfs = super::apply(patch, &vfs).unwrap();

        assert_eq!(result_vfs.get("new.txt").unwrap(), "new content");
        assert_eq!(result_vfs.get("a.txt").unwrap(), "b");
        assert!(result_vfs.get("old.txt").is_none());
        assert_eq!(result_vfs.len(), 2);
    }

    #[test]
    fn test_apply_add_to_non_empty_vfs() {
        let patch = "*** Begin Patch\n*** Add File: new.txt\n+new content\n*** End Patch";
        let vfs = vfs_from_str("existing.txt", "some content");
        let result_vfs = super::apply(patch, &vfs).unwrap();
        assert_eq!(result_vfs.len(), 2);
        assert_eq!(result_vfs.get("new.txt").unwrap(), "new content");
        assert_eq!(result_vfs.get("existing.txt").unwrap(), "some content");
    }

    #[test]
    fn test_apply_add_empty_file() {
        let patch = "*** Begin Patch\n*** Add File: empty.txt\n*** End Patch";
        let vfs = Vfs::new();
        let result_vfs = super::apply(patch, &vfs).unwrap();
        assert_eq!(result_vfs.len(), 1);
        assert_eq!(result_vfs.get("empty.txt").unwrap(), "");
    }

    #[test]
    fn test_apply_delete_from_multi_file_vfs() {
        let patch = "*** Begin Patch\n*** Delete File: b.txt\n-content b\n*** End Patch";
        let mut vfs = vfs_from_str("a.txt", "content a");
        vfs.insert("b.txt".to_string(), "content b".to_string());
        let result_vfs = super::apply(patch, &vfs).unwrap();
        assert_eq!(result_vfs.len(), 1);
        assert!(result_vfs.get("b.txt").is_none());
        assert_eq!(result_vfs.get("a.txt").unwrap(), "content a");
    }

    #[test]
    fn test_apply_delete_no_content_on_empty_file() {
        let patch = "*** Begin Patch\n*** Delete File: empty.txt\n*** End Patch";
        let vfs = vfs_from_str("empty.txt", "");
        let result_vfs = super::apply(patch, &vfs).unwrap();
        assert!(result_vfs.is_empty());
    }

    #[test]
    fn test_apply_delete_no_content_on_non_empty_file_fails() {
        let patch = "*** Begin Patch\n*** Delete File: file.txt\n*** End Patch";
        let vfs = vfs_from_str("file.txt", "i have content");
        let result = super::apply(patch, &vfs);
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::ZenpatchError::PatchConflict(msg) => {
                assert!(msg.contains("does not match original content."));
            }
            _ => panic!("Expected PatchConflict error"),
        }
    }
}
