//! Tests for backtracking patcher on large, synthetic files.

use crate::vfs::Vfs;
use crate::apply;

// Helper to create a VFS from a single file's content.
fn vfs_from_str(path: &str, content: &str) -> Vfs {
    let mut vfs = Vfs::new();
    vfs.insert(path.to_string(), content.to_string());
    vfs
}

/// Generates a large file content with a given number of lines.
fn generate_large_file(lines: usize) -> String {
    (0..lines).map(|i| format!("Line {}", i)).collect::<Vec<_>>().join("\n")
}

#[test]
fn test_large_file_scattered_changes() {
    let file_content = generate_large_file(5000);
    let vfs = vfs_from_str("large.txt", &file_content);

    let patch = r#"*** Begin Patch
*** Update File: large.txt
@@
 Line 9
-Line 10
+Line 10 - Modified
 Line 11
@@
 Line 2499
-Line 2500
+Line 2500 - Modified
 Line 2501
@@
 Line 4989
-Line 4990
+Line 4990 - Modified
 Line 4991
*** End Patch"#;

    let result_vfs = apply(patch, &vfs).unwrap();
    let updated_content = result_vfs.get("large.txt").unwrap();

    assert!(updated_content.contains("Line 10 - Modified"));
    assert!(updated_content.contains("Line 2500 - Modified"));
    assert!(updated_content.contains("Line 4990 - Modified"));
    assert!(!updated_content.contains("\nLine 10\n"));
    assert!(!updated_content.contains("\nLine 2500\n"));
    assert!(!updated_content.contains("\nLine 4990\n"));

    let original_line_count = file_content.lines().count();
    let updated_line_count = updated_content.lines().count();
    assert_eq!(original_line_count, updated_line_count);
}

#[test]
fn test_large_file_block_modification() {
    let file_content = generate_large_file(5000);
    let vfs = vfs_from_str("large.txt", &file_content);

    let patch = r#"*** Begin Patch
*** Update File: large.txt
@@
 Line 2499
-Line 2500
-Line 2501
-Line 2502
+Line 2500 - Block Modified
+Line 2501 - Block Modified
+Line 2502 - Block Modified
 Line 2503
*** End Patch"#;

    let result_vfs = apply(patch, &vfs).unwrap();
    let updated_content = result_vfs.get("large.txt").unwrap();

    assert!(updated_content.contains("Line 2500 - Block Modified"));
    assert!(!updated_content.contains("\nLine 2500\n"));
    let original_line_count = file_content.lines().count();
    let updated_line_count = updated_content.lines().count();
    assert_eq!(original_line_count, updated_line_count);
}

#[test]
fn test_large_file_large_insertion() {
    let file_content = generate_large_file(2000);
    let vfs = vfs_from_str("large.txt", &file_content);

    let insertion_block = (0..100).map(|i| format!("+Inserted Line {}", i)).collect::<Vec<_>>().join("\n");

    let patch = format!(r#"*** Begin Patch
*** Update File: large.txt
@@
 Line 999
 Line 1000
{}
 Line 1001
*** End Patch"#, insertion_block);

    let result_vfs = apply(&patch, &vfs).unwrap();
    let updated_content = result_vfs.get("large.txt").unwrap();

    assert!(updated_content.contains("Inserted Line 0"));
    assert!(updated_content.contains("Inserted Line 99"));
    
    let original_line_count = file_content.lines().count();
    let updated_line_count = updated_content.lines().count();
    assert_eq!(updated_line_count, original_line_count + 100);
}

#[test]
fn test_large_file_large_deletion() {
    let file_content = generate_large_file(2000);
    let vfs = vfs_from_str("large.txt", &file_content);

    let deletion_block = (1000..1100).map(|i| format!("-Line {}", i)).collect::<Vec<_>>().join("\n");

    let patch = format!(r#"*** Begin Patch
*** Update File: large.txt
@@
 Line 999
{}
 Line 1100
*** End Patch"#, deletion_block);

    let result_vfs = apply(&patch, &vfs).unwrap();
    let updated_content = result_vfs.get("large.txt").unwrap();

    assert!(!updated_content.contains("Line 1000"));
    assert!(!updated_content.contains("Line 1099"));
    assert!(updated_content.contains("Line 999"));
    assert!(updated_content.contains("Line 1100"));

    let original_line_count = file_content.lines().count();
    let updated_line_count = updated_content.lines().count();
    assert_eq!(updated_line_count, original_line_count - 100);
}

#[test]
fn test_large_file_with_many_repeating_lines() {
    let mut lines = Vec::new();
    for i in 0..1000 {
        lines.push(format!("--- Start Block {} ---", i));
        lines.push("Repeat A".to_string());
        lines.push("Repeat B".to_string());
        lines.push("Repeat C".to_string());
        lines.push(format!("--- End Block {} ---", i));
    }
    let file_content = lines.join("\n");
    let vfs = vfs_from_str("large_repeating.txt", &file_content);

    // Target a specific block in the middle to ensure context is respected
    let patch = r#"*** Begin Patch
*** Update File: large_repeating.txt
@@
 --- Start Block 500 ---
 Repeat A
-Repeat B
+Repeat B - Modified
 Repeat C
 --- End Block 500 ---
*** End Patch"#;

    let result_vfs = apply(patch, &vfs).unwrap();
    let updated_content = result_vfs.get("large_repeating.txt").unwrap();

    assert!(updated_content.contains("Repeat B - Modified"));

    // Check that other blocks were NOT modified
    assert!(updated_content.contains(r#"--- Start Block 499 ---
Repeat A
Repeat B
Repeat C
--- End Block 499 ---"#));
    assert!(updated_content.contains(r#"--- Start Block 501 ---
Repeat A
Repeat B
Repeat C
--- End Block 501 ---"#));

    // Count occurrences of "Repeat B" vs "Repeat B - Modified"
    let modified_count = updated_content.matches("Repeat B - Modified").count();
    // Count lines that are exactly "Repeat B" (not containing "Repeat B - Modified")
    let unmodified_count = updated_content.lines()
        .filter(|line| *line == "Repeat B")
        .count();
    assert_eq!(modified_count, 1);
    assert_eq!(unmodified_count, 999);
}