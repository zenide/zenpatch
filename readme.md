# Zenpatch

A robust library for applying text-based patches, designed for AI coding agents. It operates on an in-memory Virtual File System (VFS).

Patching is a crucial component of AI-driven coding. AI-generated patches are often imperfect and require lenient rules for application, especially concerning whitespace and special characters. Inspired in part by `https://github.com/openai/codex`'s approach to whitespace, Zenpatch offers a novel solution.

To the best of our knowledge, Zenpatch is a unique implementation that uses backtracking to apply patches. This approach has several advantages:
*   **Simplicity:** It avoids the need for complex, hand-written heuristics for patch application.
*   **Precision:** It is precise and will not apply an ambiguous patch. Ambiguity is detected by counting the number of possible solutions; a correct patch must have exactly one unique solution.

## Usage

The primary function is `zenpatch::apply`, which takes a patch string and a `Vfs` (a `HashMap<String, String>`), and returns the patched `Vfs`.

### Example

```rust
use zenpatch::{apply, Vfs, ZenpatchError};

fn main() -> Result<(), ZenpatchError> {
    // Initialize a VFS with some files.
    let mut vfs = Vfs::new();
    vfs.insert("file_to_update.txt".to_string(), "line 1\nold content\nline 3".to_string());
    vfs.insert("file_to_delete.txt".to_string(), "this file will be deleted".to_string());

    // A single patch can contain multiple actions (add, update, delete).
    let patch_content = r#"
*** Begin Patch
*** Update File: file_to_update.txt
@@
 line 1
-old content
+new content
 line 3
*** Add File: new_file.txt
+hello world
*** Delete File: file_to_delete.txt
-this file will be deleted
*** End Patch
"#;

    // Apply the patch to the VFS.
    let patched_vfs = apply(patch_content, &vfs)?;

    // Verify the results.
    assert_eq!(patched_vfs.get("file_to_update.txt").unwrap(), "line 1\nnew content\nline 3");
    assert_eq!(patched_vfs.get("new_file.txt").unwrap(), "hello world");
    assert!(patched_vfs.get("file_to_delete.txt").is_none());

    println!("Patch applied successfully!");
    Ok(())
}
```

## Patch Format

For detailed instructions on the text-based patch format, especially for use in AI coding agents, please refer to the `llms.txt` file in this crate. The content of this file is also available programmatically via the `zenpatch::get_llm_instructions()` function.
