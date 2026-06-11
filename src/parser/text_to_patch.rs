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
    let mut normalized = text.trim().to_string();

    // LLMs routinely wrap the whole patch in a markdown code fence
    // (```/```diff/```patch). Strip a leading fence line and, if present,
    // the matching trailing fence line.
    if normalized.starts_with("```") {
        let mut lines: std::vec::Vec<&str> = normalized.lines().collect();
        lines.remove(0);
        if lines
            .last()
            .is_some_and(|l| l.trim() == "```")
        {
            lines.pop();
        }
        normalized = lines.join("\n").trim().to_string();
    }

    // LLMs routinely omit the Begin/End envelope and start straight with a
    // file directive. When NEITHER marker is present and the text begins
    // with a directive, the intent is unambiguous — wrap it implicitly.
    // Deliberately narrow: if exactly one marker is present the patch is
    // malformed or truncated (a missing '*** End Patch' after a present
    // '*** Begin Patch' usually means the generation was cut off), and
    // auto-repairing it could apply half a patch — keep failing loudly.
    if (normalized.starts_with("*** Update File:")
        || normalized.starts_with("*** Add File:")
        || normalized.starts_with("*** Delete File:"))
        && !normalized.contains("*** Begin Patch")
        && !normalized.contains("*** End Patch")
    {
        normalized = std::format!("*** Begin Patch\n{normalized}\n*** End Patch");
    }

    let trimmed_text = normalized.as_str();

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

    /// LLMs often skip the Begin/End envelope — when neither marker is
    /// present and the text starts with a file directive, it is wrapped
    /// implicitly.
    #[test]
    fn test_implicit_envelope_for_bare_update() {
        let patch_text = "*** Update File: a.txt\n@@\n ctx\n-old\n+new";
        let actions = text_to_patch(patch_text).expect("bare update should parse");
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].type_, ActionType::Update);
    }

    #[test]
    fn test_implicit_envelope_for_bare_add() {
        let patch_text = "*** Add File: new.txt\n+content";
        let actions = text_to_patch(patch_text).expect("bare add should parse");
        assert_eq!(actions[0].type_, ActionType::Add);
    }

    /// LLMs routinely wrap the whole patch in a markdown code fence.
    #[test]
    fn test_markdown_fenced_patch_is_unwrapped() {
        let patch_text =
            "```diff\n*** Begin Patch\n*** Add File: a.txt\n+hi\n*** End Patch\n```";
        let actions = text_to_patch(patch_text).expect("fenced patch should parse");
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].type_, ActionType::Add);
    }

    /// Fence stripping composes with the implicit envelope.
    #[test]
    fn test_markdown_fenced_bare_directive_patch() {
        let patch_text = "```\n*** Update File: a.txt\n@@\n-a\n+b\n```";
        let actions = text_to_patch(patch_text).expect("fenced bare patch should parse");
        assert_eq!(actions[0].type_, ActionType::Update);
    }

    /// A present Begin without End usually means the generation was cut
    /// off — auto-closing could apply half a patch, so it must stay loud.
    #[test]
    fn test_begin_without_end_still_fails() {
        let patch_text = "*** Begin Patch\n*** Update File: a.txt\n@@\n ctx\n+new";
        assert!(text_to_patch(patch_text).is_err());
    }

    #[test]
    fn test_end_without_begin_still_fails() {
        let patch_text = "*** Update File: a.txt\n@@\n ctx\n+new\n*** End Patch";
        assert!(text_to_patch(patch_text).is_err());
    }

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