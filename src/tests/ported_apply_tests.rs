//! Ported tests from the original `actions` module to ensure compatibility.

use crate::vfs::Vfs;
use crate::{apply, ZenpatchError};

// Helper to create a VFS from a single file's content.
fn vfs_from_str(path: &str, content: &str) -> Vfs {
    let mut vfs = Vfs::new();
    vfs.insert(path.to_string(), content.to_string());
    vfs
}

#[test]
fn test_update_file_multiple_chunks() {
    let patch = "*** Begin Patch\n*** Update File: multi.txt\n@@\n foo\n-bar\n+BAR\n@@\n baz\n-qux\n+QUX\n*** End Patch";
    let vfs = vfs_from_str("multi.txt", "foo\nbar\nbaz\nqux");
    let result_vfs = apply(patch, &vfs).unwrap();
    assert_eq!(result_vfs.get("multi.txt").unwrap(), "foo\nBAR\nbaz\nQUX");
}

#[test]
fn test_patch_fails_on_unicode_near_miss() {
    // Initial content uses GREEK CAPITAL LETTER ALPHA (U+0391), not LATIN 'A'
    let vfs = vfs_from_str("test.txt", "Line Alpha: Α\nLine Next");
    // Patch context mistakenly uses LATIN 'A' (U+0041) instead of Greek Alpha
    let patch = "*** Begin Patch\n*** Update File: test.txt\n@@\n Line Alpha: A\n-Line Next\n+Modified Line Next\n*** End Patch";

    let result = apply(patch, &vfs);

    // Patch should fail due to exact context mismatch. It will retry with lenient, which should also fail.
    assert!(result.is_err(), "Patch should have failed due to Unicode character mismatch");
    match result.unwrap_err() {
        ZenpatchError::PatchConflict(_) => (), // This is expected
        e => panic!("Expected PatchConflict, got {:?}", e),
    }
}

#[test]
fn test_patch_idempotent_deletion_fails_on_second_apply() {
    let vfs = vfs_from_str("test.txt", "Line 1\nLineToDelete\nLine 3");
    let patch = "*** Begin Patch\n*** Update File: test.txt\n@@\n Line 1\n-LineToDelete\n Line 3\n*** End Patch";

    // First application
    let result_vfs1 = apply(patch, &vfs).expect("First patch application failed");
    assert_eq!(result_vfs1.get("test.txt").unwrap(), "Line 1\nLine 3");

    // Second application should fail because the context "-LineToDelete" is gone
    let result2 = apply(patch, &result_vfs1);
    assert!(result2.is_err(), "Second patch application should have failed (context not found)");
    assert!(matches!(result2.unwrap_err(), ZenpatchError::PatchConflict(_)));
}

#[test]
fn test_patch_repeated_context_close_proximity() {
    let vfs = vfs_from_str("test.txt", "Marker\nTarget\nMarker\nOther Target\nMarker");
    let patch = "*** Begin Patch\n*** Update File: test.txt\n@@\n Marker\n-Target\n+Modified Target\n Marker\n*** End Patch";

    let result_vfs = apply(patch, &vfs).unwrap();
    assert_eq!(result_vfs.get("test.txt").unwrap(), "Marker\nModified Target\nMarker\nOther Target\nMarker");
}

#[test]
fn test_patch_context_looks_like_patch_syntax() {
    let vfs = vfs_from_str("test.txt", "Line 1\n- Line 2 (looks like delete)\n+ Line 3 (looks like add)\n  Line 4 (looks like context)\nLine 5");
    let patch = "*** Begin Patch\n*** Update File: test.txt\n@@\n + Line 3 (looks like add)\n   Line 4 (looks like context)\n-Line 5\n+Modified Line 5\n*** End Patch";

    let result_vfs = apply(patch, &vfs).unwrap();
    assert_eq!(result_vfs.get("test.txt").unwrap(), "Line 1\n- Line 2 (looks like delete)\n+ Line 3 (looks like add)\n  Line 4 (looks like context)\nModified Line 5");
}

#[test]
fn test_patch_delete_and_re_add_different_content() {
    let vfs = vfs_from_str("test.txt", "Line A\nLine B\nLine C");
    // Chunk 1 deletes Line B, Chunk 2 adds Line B New in the same spot
    let patch = "*** Begin Patch\n*** Update File: test.txt\n@@\n Line A\n-Line B\n Line C\n@@\n Line A\n+Line B New\n Line C\n*** End Patch";

    let result_vfs = apply(patch, &vfs).unwrap();
    assert_eq!(result_vfs.get("test.txt").unwrap(), "Line A\nLine B New\nLine C");
}

#[test]
fn test_patch_no_op_change() {
    let vfs = vfs_from_str("test.txt", "Line 1\nLine 2\nLine 3");
    // Patch deletes and re-adds the same line - should result in no effective change.
    let patch = "*** Begin Patch\n*** Update File: test.txt\n@@\n Line 1\n-Line 2\n+Line 2\n Line 3\n*** End Patch";

    let result_vfs = apply(patch, &vfs).unwrap();
    // The file might be rewritten, but content should be identical.
    assert_eq!(result_vfs.get("test.txt").unwrap(), "Line 1\nLine 2\nLine 3");
}

#[test]
fn test_patch_windows_style_newlines() {
    let vfs = vfs_from_str("test.txt", "Line 1\r\nLine 2\r\nLine 3"); // Windows-style CRLF
    // Patch with Unix-style newlines
    let patch = "*** Begin Patch\n*** Update File: test.txt\n@@\n Line 1\n-Line 2\n+Modified Line 2\n Line 3\n*** End Patch";

    let result_vfs = apply(patch, &vfs).unwrap();
    // The `apply` function joins with `\n`, so it normalizes newlines.
    assert_eq!(result_vfs.get("test.txt").unwrap(), "Line 1\nModified Line 2\nLine 3");
}

// ── Whitespace fallback tests (Strict → Lenient) ──

#[test]
fn test_whitespace_fallback_extra_leading_spaces() {
    // File has extra leading spaces; patch context doesn't.
    // Strict fails, lenient should succeed.
    let vfs = vfs_from_str("ws.txt", "  Line 1\n  Line 2\n  Line 3");
    let patch = "*** Begin Patch\n*** Update File: ws.txt\n@@\n Line 1\n-Line 2\n+Modified Line 2\n Line 3\n*** End Patch";
    let result_vfs = apply(patch, &vfs).unwrap();
    assert_eq!(result_vfs.get("ws.txt").unwrap(), "  Line 1\nModified Line 2\n  Line 3");
}

#[test]
fn test_whitespace_fallback_tabs_vs_spaces() {
    // File uses tabs, patch uses spaces — strict fails, lenient succeeds.
    let vfs = vfs_from_str("tabs.txt", "\tLine 1\n\tLine 2\n\tLine 3");
    let patch = "*** Begin Patch\n*** Update File: tabs.txt\n@@\n Line 1\n-Line 2\n+Modified Line 2\n Line 3\n*** End Patch";
    let result_vfs = apply(patch, &vfs).unwrap();
    assert_eq!(result_vfs.get("tabs.txt").unwrap(), "\tLine 1\nModified Line 2\n\tLine 3");
}

#[test]
fn test_whitespace_fallback_mixed_indentation() {
    // File has inconsistent whitespace (some lines tabs, some spaces).
    let vfs = vfs_from_str("mixed.txt", "    alpha\n\tbeta\n    gamma");
    let patch = "*** Begin Patch\n*** Update File: mixed.txt\n@@\n alpha\n-beta\n+BETA\n gamma\n*** End Patch";
    let result_vfs = apply(patch, &vfs).unwrap();
    assert_eq!(result_vfs.get("mixed.txt").unwrap(), "    alpha\nBETA\n    gamma");
}

// ── Multi-file interaction tests ──

#[test]
fn test_patch_updates_two_files() {
    let mut vfs = Vfs::new();
    vfs.insert("a.txt".to_string(), "line1\nline2".to_string());
    vfs.insert("b.txt".to_string(), "foo\nbar".to_string());
    let patch = "*** Begin Patch\n\
*** Update File: a.txt\n@@\n line1\n-line2\n+LINE2\n\
*** Update File: b.txt\n@@\n foo\n-bar\n+BAR\n\
*** End Patch";
    let result_vfs = apply(patch, &vfs).unwrap();
    assert_eq!(result_vfs.get("a.txt").unwrap(), "line1\nLINE2");
    assert_eq!(result_vfs.get("b.txt").unwrap(), "foo\nBAR");
}

#[test]
fn test_patch_adds_and_updates_in_one_patch() {
    let vfs = vfs_from_str("existing.txt", "aaa\nbbb");
    let patch = "*** Begin Patch\n\
*** Add File: new.txt\n+hello\n+world\n\
*** Update File: existing.txt\n@@\n aaa\n-bbb\n+BBB\n\
*** End Patch";
    let result_vfs = apply(patch, &vfs).unwrap();
    assert_eq!(result_vfs.get("new.txt").unwrap(), "hello\nworld");
    assert_eq!(result_vfs.get("existing.txt").unwrap(), "aaa\nBBB");
}

#[test]
fn test_update_file_not_in_vfs_returns_file_not_found() {
    let vfs = Vfs::new();
    let patch = "*** Begin Patch\n*** Update File: missing.txt\n@@\n-old\n+new\n*** End Patch";
    let result = apply(patch, &vfs);
    assert!(matches!(result.unwrap_err(), ZenpatchError::FileNotFound(p) if p == "missing.txt"));
}

#[test]
fn test_duplicate_add_returns_file_exists() {
    let vfs = vfs_from_str("dup.txt", "content");
    let patch = "*** Begin Patch\n*** Add File: dup.txt\n+new content\n*** End Patch";
    let result = apply(patch, &vfs);
    assert!(matches!(result.unwrap_err(), ZenpatchError::FileExists(p) if p == "dup.txt"));
}

// ── @@ header disambiguation tests ──

#[test]
fn test_at_header_disambiguates_repeated_context() {
    // File has two identical blocks under different class headers.
    let content = "class Foo:\n  def run(self):\n    pass\nclass Bar:\n  def run(self):\n    pass";
    let vfs = vfs_from_str("code.py", content);
    let patch = "*** Begin Patch\n*** Update File: code.py\n@@ class Bar:\n def run(self):\n-    pass\n+    return 42\n*** End Patch";
    let result_vfs = apply(patch, &vfs).unwrap();
    assert_eq!(
        result_vfs.get("code.py").unwrap(),
        "class Foo:\n  def run(self):\n    pass\nclass Bar:\n  def run(self):\n    return 42"
    );
}

// ── *** End of File anchoring tests ──

#[test]
fn test_end_of_file_anchors_to_end() {
    // File has "marker" appearing twice; the End of File marker forces the second one.
    let content = "marker\ntarget\nmiddle\nmarker\ntarget";
    let vfs = vfs_from_str("eof.txt", content);
    let patch = "*** Begin Patch\n*** Update File: eof.txt\n@@\n marker\n-target\n+REPLACED\n*** End of File\n*** End Patch";
    let result_vfs = apply(patch, &vfs).unwrap();
    assert_eq!(
        result_vfs.get("eof.txt").unwrap(),
        "marker\ntarget\nmiddle\nmarker\nREPLACED"
    );
}

#[test]
fn test_end_of_file_append() {
    let content = "first\nlast";
    let vfs = vfs_from_str("append.txt", content);
    let patch = "*** Begin Patch\n*** Update File: append.txt\n@@\n last\n+appended line\n*** End of File\n*** End Patch";
    let result_vfs = apply(patch, &vfs).unwrap();
    assert_eq!(
        result_vfs.get("append.txt").unwrap(),
        "first\nlast\nappended line"
    );
}