# Zenpatch

A robust library for applying text-based patches, designed for AI coding agents.

## Usage

The primary function is `zenpatch::apply`, which takes a patch string and the original content string, and returns the patched content.

### Example

```rust
use zenpatch::{apply, ZenpatchError};

fn main() -> Result<(), ZenpatchError> {
    let original_content = "line 1\nline 2\nline 3\n";
    let patch_content = r#"
*** Begin Patch
*** Update File: example.txt
@@
 line 1
-line 2
+line two
 line 3
*** End Patch
"#;

    let patched_content = apply(patch_content, original_content)?;

    assert_eq!(patched_content, "line 1\nline two\nline 3\n");
    println!("Patch applied successfully!");
    Ok(())
}
```
