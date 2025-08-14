//! Ported tests from the original `actions` module to ensure compatibility.

use crate::{apply, ZenpatchError};

#[test]
fn test_update_file_multiple_chunks() {
    let initial_content = "foo\nbar\nbaz\nqux";
    let patch = "*** Begin Patch\n*** Update File: multi.txt\n@@\n foo\n-bar\n+BAR\n@@\n baz\n-qux\n+QUX\n*** End Patch";
    let result = apply(patch, initial_content);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "foo\nBAR\nbaz\nQUX");
}

#[test]
fn test_patch_fails_on_unicode_near_miss() {
    // Initial content uses GREEK CAPITAL LETTER ALPHA (U+0391), not LATIN 'A'
    let initial_content = "Line Alpha: Α\nLine Next";
    // Patch context mistakenly uses LATIN 'A' (U+0041) instead of Greek Alpha
    let patch = "*** Begin Patch\n*** Update File: test.txt\n@@\n Line Alpha: A\n-Line Next\n+Modified Line Next\n*** End Patch";

    let result = apply(patch, initial_content);

    // Patch should fail due to exact context mismatch. It will retry with lenient, which should also fail.
    assert!(result.is_err(), "Patch should have failed due to Unicode character mismatch");
    match result.unwrap_err() {
        ZenpatchError::PatchConflict(_) => (), // This is expected
        e => panic!("Expected PatchConflict, got {:?}", e),
    }
}

#[test]
fn test_patch_idempotent_deletion_fails_on_second_apply() {
    let initial_content = "Line 1\nLineToDelete\nLine 3";
    let patch = "*** Begin Patch\n*** Update File: test.txt\n@@\n Line 1\n-LineToDelete\n Line 3\n*** End Patch";

    // First application
    let result1 = apply(patch, initial_content);
    assert!(result1.is_ok(), "First patch application failed: {:?}", result1.err());
    let content_after_first = result1.unwrap();
    assert_eq!(content_after_first, "Line 1\nLine 3");

    // Second application should fail because the context "-LineToDelete" is gone
    let result2 = apply(patch, &content_after_first);
    assert!(result2.is_err(), "Second patch application should have failed (context not found)");
    assert!(matches!(result2.unwrap_err(), ZenpatchError::PatchConflict(_)));
}

#[test]
fn test_patch_repeated_context_close_proximity() {
    let initial_content = "Marker\nTarget\nMarker\nOther Target\nMarker";
    let patch = "*** Begin Patch\n*** Update File: test.txt\n@@\n Marker\n-Target\n+Modified Target\n Marker\n*** End Patch";

    let result = apply(patch, initial_content);
    assert!(result.is_ok(), "Patch failed: {:?}", result.err());
    assert_eq!(result.unwrap(), "Marker\nModified Target\nMarker\nOther Target\nMarker");
}

#[test]
fn test_patch_context_looks_like_patch_syntax() {
    let initial_content = "Line 1\n- Line 2 (looks like delete)\n+ Line 3 (looks like add)\n  Line 4 (looks like context)\nLine 5";
    let patch = "*** Begin Patch\n*** Update File: test.txt\n@@\n + Line 3 (looks like add)\n   Line 4 (looks like context)\n-Line 5\n+Modified Line 5\n*** End Patch";

    let result = apply(patch, initial_content);
    assert!(result.is_ok(), "Patch failed: {:?}", result.err());
    assert_eq!(result.unwrap(), "Line 1\n- Line 2 (looks like delete)\n+ Line 3 (looks like add)\n  Line 4 (looks like context)\nModified Line 5");
}

#[test]
fn test_patch_delete_and_re_add_different_content() {
    let initial_content = "Line A\nLine B\nLine C";
    // Chunk 1 deletes Line B, Chunk 2 adds Line B New in the same spot
    let patch = "*** Begin Patch\n*** Update File: test.txt\n@@\n Line A\n-Line B\n Line C\n@@\n Line A\n+Line B New\n Line C\n*** End Patch";

    let result = apply(patch, initial_content);
    assert!(result.is_ok(), "Patch failed: {:?}", result.err());
    assert_eq!(result.unwrap(), "Line A\nLine B New\nLine C");
}

#[test]
fn test_patch_no_op_change() {
    let initial_content = "Line 1\nLine 2\nLine 3";
    // Patch deletes and re-adds the same line - should result in no effective change.
    let patch = "*** Begin Patch\n*** Update File: test.txt\n@@\n Line 1\n-Line 2\n+Line 2\n Line 3\n*** End Patch";

    let result = apply(patch, initial_content);
    assert!(result.is_ok(), "Patch failed: {:?}", result.err());
    // The file might be rewritten, but content should be identical.
    assert_eq!(result.unwrap(), initial_content);
}

#[test]
fn test_patch_windows_style_newlines() {
    let initial_content = "Line 1\r\nLine 2\r\nLine 3"; // Windows-style CRLF
    // Patch with Unix-style newlines
    let patch = "*** Begin Patch\n*** Update File: test.txt\n@@\n Line 1\n-Line 2\n+Modified Line 2\n Line 3\n*** End Patch";

    let result = apply(patch, initial_content);
    assert!(result.is_ok(), "Patch failed: {:?}", result.err());
    // The `apply` function joins with `\n`, so it normalizes newlines.
    assert_eq!(result.unwrap(), "Line 1\nModified Line 2\nLine 3");
}