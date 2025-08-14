//! Provides the `text_to_patch` function for parsing patch text.
//!
//! This function is the main entry point for parsing a text-based patch. It takes
//! patch text and returns a structured `PatchAction` object.
//! Adheres to the one-item-per-file rule and uses fully qualified paths.

/// Parses patch text into a structured `PatchAction` object.
///
/// Validates the patch format (start/end markers) and delegates the core parsing
/// logic to the `Parser`. It expects the patch to contain exactly one file operation.
///
/// # Arguments
///
/// * `text` - The patch content as a string slice.
///
/// # Returns
///
/// * `Ok(PatchAction)` - The parsed `PatchAction` if successful.
/// * `Err(ZenpatchError)` - An error if the patch text is invalid or parsing fails.
pub fn text_to_patch(
    text: &str,
) -> std::result::Result<std::vec::Vec<crate::data::patch_action::PatchAction>, crate::error::ZenpatchError>
{
    let trimmed_text = text.trim();
    let lines: std::vec::Vec<&str> = trimmed_text.lines().collect();

    if lines.len() < 2 {
        return std::result::Result::Err(crate::error::ZenpatchError::InvalidPatchFormat(
            "Patch text is too short (must include start and end markers).".to_string(),
        ));
    }
    if lines[0] != "*** Begin Patch" {
        return std::result::Result::Err(crate::error::ZenpatchError::InvalidPatchFormat(
            "Patch must start with '*** Begin Patch'".to_string(),
        ));
    }
    if lines[lines.len() - 1] != "*** End Patch" {
        return std::result::Result::Err(crate::error::ZenpatchError::InvalidPatchFormat(
            "Patch must end with '*** End Patch'".to_string(),
        ));
    }

    let mut parser = crate::parser::parser::Parser::new(trimmed_text);
    let mut actions = parser.parse()?;

    // Post-process chunks to populate del_lines and ins_lines
    for action in &mut actions {
        for chunk in &mut action.chunks {
            chunk.del_lines = chunk
                .lines
                .iter()
                .filter_map(|(lt, content)| {
                    if *lt == crate::data::line_type::LineType::Deletion {
                        std::option::Option::Some(content.clone())
                    } else {
                        std::option::Option::None
                    }
                })
                .collect();

            chunk.ins_lines = chunk
                .lines
                .iter()
                .filter_map(|(lt, content)| {
                    if *lt == crate::data::line_type::LineType::Insertion {
                        std::option::Option::Some(content.clone())
                    } else {
                        std::option::Option::None
                    }
                })
                .collect();
        }
    }

    std::result::Result::Ok(actions)
}

#[cfg(test)]
mod tests {
    use super::text_to_patch;
    use crate::data::action_type::ActionType;
    use crate::data::line_type::LineType;

    #[test]
    fn test_text_to_patch_valid_add() {
        let patch_text = "*** Begin Patch\n*** Add File: new.txt\n+content\n*** End Patch";
        let result = text_to_patch(patch_text);
        assert!(result.is_ok());
        let actions = result.unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].type_, ActionType::Add);
        assert_eq!(actions[0].path, "new.txt");
    }

    #[test]
    fn test_text_to_patch_valid_empty_with_whitespace() {
        // Test trimming of leading/trailing whitespace.
        let patch_text = "  \n*** Begin Patch\n*** Add File: a.txt\n+a\n*** End Patch\n  ";
        let result = text_to_patch(patch_text);
        assert!(result.is_ok());
    }

    #[test]
    fn test_text_to_patch_missing_start_marker() {
        let patch_text = "Invalid start\n*** End Patch";
        let result = text_to_patch(patch_text);
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::ZenpatchError::InvalidPatchFormat(msg) => {
                assert!(
                    msg.contains("must start with"),
                    "Incorrect error message: {}",
                    msg
                );
            }
            _ => std::panic!("Expected InvalidPatchFormat error for missing start marker"),
        }
    }

    #[test]
    fn test_text_to_patch_missing_end_marker() {
        let patch_text = "*** Begin Patch\nInvalid end";
        let result = text_to_patch(patch_text);
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::ZenpatchError::InvalidPatchFormat(msg) => {
                assert!(
                    msg.contains("must end with"),
                    "Incorrect error message: {}",
                    msg
                );
            }
            _ => std::panic!("Expected InvalidPatchFormat error for missing end marker"),
        }
    }

    #[test]
    fn test_text_to_patch_too_short() {
        let patch_text = "*** Begin Patch";
        let result = text_to_patch(patch_text);
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::ZenpatchError::InvalidPatchFormat(msg) => {
                assert!(msg.contains("too short"), "Incorrect error message: {}", msg);
            }
            _ => std::panic!("Expected InvalidPatchFormat error for short patch"),
        }
    }

    #[test]
    fn test_text_to_patch_update_with_context() {
        let patch_text = "*** Begin Patch\n\
*** Update File: file.txt\n\
@@\n\
-old line 2\n\
+new line 2a\n\
*** End Patch";

        let result = text_to_patch(patch_text);
        assert!(result.is_ok(), "Parsing failed: {:?}", result);

        let actions = result.unwrap();
        assert_eq!(actions.len(), 1);
        let action = &actions[0];
        assert_eq!(action.type_, ActionType::Update);
        assert_eq!(action.path, "file.txt");
        assert_eq!(action.chunks.len(), 1, "Update action should have one chunk");
        let chunk = &action.chunks[0];
        assert_eq!(chunk.lines.len(), 2);
        assert_eq!(chunk.del_lines, vec!["old line 2"]);
        assert_eq!(chunk.ins_lines, vec!["new line 2a"]);
        assert_eq!(chunk.lines[0], (LineType::Deletion, "old line 2".to_string()));
        assert_eq!(chunk.lines[1], (LineType::Insertion, "new line 2a".to_string()));
    }

    #[test]
    fn test_text_to_patch_multiple_actions_succeeds() {
        let patch_text = "*** Begin Patch\n\
*** Add File: new_file.txt\n\
+New content\n\
*** Delete File: old_file.txt\n\
*** End Patch";

        let actions = text_to_patch(patch_text).unwrap();
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].type_, ActionType::Add);
        assert_eq!(actions[1].type_, ActionType::Delete);
        assert_eq!(actions[1].path, "old_file.txt");
    }
}