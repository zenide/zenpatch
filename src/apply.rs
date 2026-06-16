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
                // Errors are tagged with the file path so multi-file patches report WHICH file failed.
                let applied_lines = match result {
                    Err(crate::error::ZenpatchError::PatchConflict(_))
                    | Err(crate::error::ZenpatchError::AmbiguousPatch(_)) => {
                        crate::applier::backtracking_patcher::apply_patch_backtracking_mode(
                            &original_lines,
                            &action.chunks,
                            crate::applier::whitespace_mode::WhitespaceMode::Lenient,
                        )
                        .map_err(|e| e.with_path(&action.path))?
                    }
                    Ok(lines) => lines,
                    Err(e) => return Err(e.with_path(&action.path)),
                };
                // Re-join with the file's own dominant line ending and restore its
                // trailing newline: a one-line patch must not rewrite every line
                // ending in a CRLF file or strip the final newline.
                let crlf_count = original_content.matches("\r\n").count();
                let lf_only_count = original_content.matches('\n').count() - crlf_count;
                let eol = if crlf_count > lf_only_count { "\r\n" } else { "\n" };
                let mut updated_content = applied_lines.join(eol);
                if original_content.ends_with('\n') && !updated_content.is_empty() {
                    updated_content.push_str(eol);
                }

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
                        format!(
                            "in {}: content to delete does not match the file's content",
                            action.path
                        ),
                    ));
                }
            }
        }
    }

    std::result::Result::Ok(new_vfs)
}

/// Re-joins patched lines with the file's dominant EOL and restores its trailing
/// newline (so a one-line patch doesn't rewrite every ending or drop the final \n).
fn rejoin(original_content: &str, applied_lines: &[std::string::String]) -> std::string::String {
    let crlf_count = original_content.matches("\r\n").count();
    let lf_only_count = original_content.matches('\n').count() - crlf_count;
    let eol = if crlf_count > lf_only_count { "\r\n" } else { "\n" };
    let mut updated = applied_lines.join(eol);
    if original_content.ends_with('\n') && !updated.is_empty() {
        updated.push_str(eol);
    }
    updated
}

/// Applies a single Update chunk to `lines`, trying strict then lenient whitespace.
fn apply_one_chunk(
    lines: &[std::string::String],
    chunk: &crate::data::chunk::Chunk,
) -> std::result::Result<std::vec::Vec<std::string::String>, crate::error::ZenpatchError> {
    let single = std::slice::from_ref(chunk);
    match crate::applier::backtracking_patcher::apply_patch_backtracking_mode(
        lines,
        single,
        crate::applier::whitespace_mode::WhitespaceMode::Strict,
    ) {
        std::result::Result::Err(crate::error::ZenpatchError::PatchConflict(_))
        | std::result::Result::Err(crate::error::ZenpatchError::AmbiguousPatch(_)) => {
            crate::applier::backtracking_patcher::apply_patch_backtracking_mode(
                lines,
                single,
                crate::applier::whitespace_mode::WhitespaceMode::Lenient,
            )
        }
        other => other,
    }
}

/// The outcome of a best-effort (partial) patch application.
#[derive(Debug, Clone, Default)]
pub struct PartialReport {
    /// Number of Update hunks that applied (across all files).
    pub applied_hunks: std::primitive::usize,
    /// One human-readable message per hunk/action that was SKIPPED because it
    /// did not apply. An empty list means the whole patch applied cleanly.
    pub skipped: std::vec::Vec<std::string::String>,
}

/// Best-effort variant of [`apply`]: applies every hunk it can and SKIPS the ones
/// that don't, instead of rejecting the whole patch when a single hunk is wrong.
///
/// For each Update file, the full set of hunks is first attempted atomically (the
/// normal, highest-fidelity path); only if that fails does it fall back to applying
/// each hunk independently, dropping the ones that conflict. The returned
/// [`PartialReport`] lists what was skipped so the caller can re-prompt for just
/// those. Only an unparseable patch returns `Err`.
pub fn apply_partial(
    patch_text: &str,
    vfs: &crate::vfs::Vfs,
) -> std::result::Result<(crate::vfs::Vfs, PartialReport), crate::error::ZenpatchError> {
    let mut new_vfs = vfs.clone();
    let mut report = PartialReport::default();
    let actions = crate::parser::text_to_patch::text_to_patch(patch_text)?;

    for action in actions {
        match action.type_ {
            crate::data::action_type::ActionType::Update => {
                let original_content = match new_vfs.get(&action.path) {
                    std::option::Option::Some(c) => c.to_string(),
                    std::option::Option::None => {
                        report.skipped.push(format!("{}: file not found", action.path));
                        continue;
                    }
                };
                let original_lines: std::vec::Vec<std::string::String> =
                    original_content.lines().map(std::string::String::from).collect();

                // 1. Try all hunks atomically (best fidelity / disambiguation).
                let atomic = match crate::applier::backtracking_patcher::apply_patch_backtracking_mode(
                    &original_lines,
                    &action.chunks,
                    crate::applier::whitespace_mode::WhitespaceMode::Strict,
                ) {
                    std::result::Result::Err(crate::error::ZenpatchError::PatchConflict(_))
                    | std::result::Result::Err(crate::error::ZenpatchError::AmbiguousPatch(_)) => {
                        crate::applier::backtracking_patcher::apply_patch_backtracking_mode(
                            &original_lines,
                            &action.chunks,
                            crate::applier::whitespace_mode::WhitespaceMode::Lenient,
                        )
                    }
                    other => other,
                };

                let final_lines = match atomic {
                    std::result::Result::Ok(lines) => {
                        report.applied_hunks += action.chunks.len();
                        lines
                    }
                    std::result::Result::Err(_) => {
                        // 2. Fall back to per-hunk best effort.
                        let mut lines = original_lines.clone();
                        for (i, chunk) in action.chunks.iter().enumerate() {
                            match apply_one_chunk(&lines, chunk) {
                                std::result::Result::Ok(updated) => {
                                    lines = updated;
                                    report.applied_hunks += 1;
                                }
                                std::result::Result::Err(e) => {
                                    report.skipped.push(format!(
                                        "{}: hunk {} skipped: {}",
                                        action.path,
                                        i + 1,
                                        e
                                    ));
                                }
                            }
                        }
                        lines
                    }
                };

                if final_lines == original_lines {
                    continue; // nothing applied for this file
                }
                let updated_content = rejoin(&original_content, &final_lines);
                if let Some(new_path) = &action.new_path {
                    new_vfs.remove(&action.path);
                    new_vfs.insert(new_path.clone(), updated_content);
                } else {
                    new_vfs.insert(action.path.clone(), updated_content);
                }
            }
            crate::data::action_type::ActionType::Add => {
                if new_vfs.contains_key(&action.path) {
                    report.skipped.push(format!("{}: add skipped (file exists)", action.path));
                    continue;
                }
                let content: std::vec::Vec<std::string::String> =
                    action.chunks.iter().flat_map(|c| c.ins_lines.clone()).collect();
                new_vfs.insert(action.path.clone(), content.join("\n"));
                report.applied_hunks += 1;
            }
            crate::data::action_type::ActionType::Delete => {
                let original_content = match new_vfs.get(&action.path) {
                    std::option::Option::Some(c) => c.to_string(),
                    std::option::Option::None => {
                        report.skipped.push(format!("{}: delete skipped (not found)", action.path));
                        continue;
                    }
                };
                let content_to_delete: std::vec::Vec<std::string::String> =
                    action.chunks.iter().flat_map(|c| c.del_lines.clone()).collect();
                let original_lines: std::vec::Vec<std::string::String> =
                    original_content.lines().map(std::string::String::from).collect();
                if content_to_delete == original_lines {
                    new_vfs.remove(&action.path);
                    report.applied_hunks += 1;
                } else {
                    report.skipped.push(format!("{}: delete skipped (content mismatch)", action.path));
                }
            }
        }
    }

    std::result::Result::Ok((new_vfs, report))
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
    fn test_apply_partial_keeps_good_hunk_drops_bad() {
        // Two hunks for one file: the first is applyable, the second's context
        // ("ghost") does not exist. apply_partial must land the good one and skip
        // the bad one (where atomic `apply` would reject the whole patch).
        let patch = "*** Begin Patch\n*** Update File: a.txt\n@@\n-a\n+A\n@@\n ghost\n-real\n+REAL\n*** End Patch";
        let vfs = vfs_from_str("a.txt", "a\nb\nreal");
        // atomic apply fails outright
        assert!(super::apply(patch, &vfs).is_err());
        // partial apply lands the good hunk, reports the bad one
        let (out, report) = super::apply_partial(patch, &vfs).unwrap();
        assert_eq!(out.get("a.txt").unwrap(), "A\nb\nreal");
        assert_eq!(report.applied_hunks, 1);
        assert_eq!(report.skipped.len(), 1);
        assert!(report.skipped[0].contains("ghost") || report.skipped[0].contains("hunk 2"));
    }

    #[test]
    fn test_apply_partial_clean_patch_applies_all() {
        let patch = "*** Begin Patch\n*** Update File: a.txt\n@@\n-a\n+A\n*** End Patch";
        let vfs = vfs_from_str("a.txt", "a\nz\n");
        let (out, report) = super::apply_partial(patch, &vfs).unwrap();
        assert_eq!(out.get("a.txt").unwrap(), "A\nz\n");
        assert!(report.skipped.is_empty());
        assert_eq!(report.applied_hunks, 1);
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
                assert!(msg.contains("does not match"));
            }
            _ => panic!("Expected PatchConflict error"),
        }
    }

    #[test]
    fn test_update_preserves_trailing_newline() {
        let patch = "*** Begin Patch\n*** Update File: a.txt\n@@\n-a\n+b\n*** End Patch";
        let vfs = vfs_from_str("a.txt", "a\nz\n");
        let result_vfs = super::apply(patch, &vfs).unwrap();
        assert_eq!(result_vfs.get("a.txt").unwrap(), "b\nz\n");
    }

    #[test]
    fn test_update_keeps_absent_trailing_newline_absent() {
        let patch = "*** Begin Patch\n*** Update File: a.txt\n@@\n-a\n+b\n*** End Patch";
        let vfs = vfs_from_str("a.txt", "a\nz");
        let result_vfs = super::apply(patch, &vfs).unwrap();
        assert_eq!(result_vfs.get("a.txt").unwrap(), "b\nz");
    }

    #[test]
    fn test_update_preserves_crlf_and_trailing_newline() {
        let patch = "*** Begin Patch\n*** Update File: a.txt\n@@\n-a\n+b\n*** End Patch";
        let vfs = vfs_from_str("a.txt", "a\r\nz\r\n");
        let result_vfs = super::apply(patch, &vfs).unwrap();
        assert_eq!(result_vfs.get("a.txt").unwrap(), "b\r\nz\r\n");
    }

    /// A blank context line inside a hunk (its lone ' ' prefix stripped by the
    /// LLM or an editor) must still match a blank line in the file.
    #[test]
    fn test_update_blank_context_line_inside_hunk() {
        let patch = "*** Begin Patch\n*** Update File: f.py\n@@\n fn_a = 1\n\n-fn_b = 2\n+fn_b = 99\n*** End Patch";
        let vfs = vfs_from_str("f.py", "fn_a = 1\n\nfn_b = 2");
        let result_vfs = super::apply(patch, &vfs).unwrap();
        assert_eq!(result_vfs.get("f.py").unwrap(), "fn_a = 1\n\nfn_b = 99");
    }

    /// Cosmetic blank lines at the hunk edges (after @@, before End Patch)
    /// must not be required to exist in the file.
    #[test]
    fn test_update_blank_separator_lines_at_hunk_edges_ignored() {
        let patch = "*** Begin Patch\n*** Update File: f.txt\n@@\n\n a\n-b\n+B\n\n*** End Patch";
        let vfs = vfs_from_str("f.txt", "a\nb");
        let result_vfs = super::apply(patch, &vfs).unwrap();
        assert_eq!(result_vfs.get("f.txt").unwrap(), "a\nB");
    }

    /// Chunks after an '*** End of File' marker must be applied, not
    /// silently discarded while the patch reports success.
    #[test]
    fn test_update_chunk_after_end_of_file_is_not_dropped() {
        let patch = "*** Begin Patch\n*** Update File: f.txt\n@@\n gamma\n-omega\n+OMEGA\n*** End of File\n@@\n-alpha\n+ALPHA\n*** End Patch";
        let vfs = vfs_from_str("f.txt", "alpha\nbeta\ngamma\nomega");
        let result_vfs = super::apply(patch, &vfs).unwrap();
        assert_eq!(
            result_vfs.get("f.txt").unwrap(),
            "ALPHA\nbeta\ngamma\nOMEGA"
        );
    }

    /// Bare blank lines inside Add File content are blank lines of the new
    /// file (the '+' was omitted) — they must be kept, not dropped.
    #[test]
    fn test_add_file_bare_blank_line_is_content() {
        let patch = "*** Begin Patch\n*** Add File: m.py\n+def a():\n+    pass\n\n+def b():\n+    pass\n*** End Patch";
        let vfs = Vfs::new();
        let result_vfs = super::apply(patch, &vfs).unwrap();
        assert_eq!(
            result_vfs.get("m.py").unwrap(),
            "def a():\n    pass\n\ndef b():\n    pass"
        );
    }

    /// A trailing run of bare blank lines before the next directive is a
    /// separator, not content of the added file.
    #[test]
    fn test_add_file_trailing_bare_blank_separator_trimmed() {
        let patch = "*** Begin Patch\n*** Add File: a.txt\n+x\n\n*** Delete File: b.txt\n-y\n*** End Patch";
        let vfs = vfs_from_str("b.txt", "y");
        let result_vfs = super::apply(patch, &vfs).unwrap();
        assert_eq!(result_vfs.get("a.txt").unwrap(), "x");
        assert!(result_vfs.get("b.txt").is_none());
    }

    #[test]
    fn test_update_conflict_names_file_and_offending_line() {
        // The hunk's context line "ghost" is not in the file: the error must name both the
        // file and the offending line so the patch can be fixed without guessing.
        let patch =
            "*** Begin Patch\n*** Update File: src/a.txt\n@@\n ghost\n-real\n+changed\n*** End Patch";
        let vfs = vfs_from_str("src/a.txt", "real\nother");
        match super::apply(patch, &vfs).unwrap_err() {
            crate::error::ZenpatchError::PatchConflict(msg) => {
                assert!(msg.contains("src/a.txt"), "should name the file: {msg}");
                assert!(msg.contains("ghost"), "should quote the offending line: {msg}");
            }
            other => panic!("Expected PatchConflict error, got {other:?}"),
        }
    }
}
