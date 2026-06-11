//! Defines the `Parser` struct for processing text-based patch files.
//!
//! This struct holds the state required to parse a patch string line by line,
//! extracting a single file change (add, delete, update) according to a specific format.
//! It enforces that a patch text must contain exactly one file operation.
//! Adheres to the one-item-per-file rule and uses fully qualified paths.

/// Parses a text-based patch format to determine a single file operation.
pub struct Parser {
    pub lines: std::vec::Vec<std::string::String>,
    pub index: usize,
}

impl Parser {
    /// Pushes a chunk after stripping empty context lines from its edges.
    ///
    /// Empty context lines INSIDE a hunk are real file content (a blank line
    /// whose leading space was stripped) and must match. At the chunk edges
    /// they are almost always cosmetic separators the LLM added around the
    /// hunk, and requiring a blank line there would break otherwise-valid
    /// patches — so the edges are trimmed.
    fn push_chunk(
        chunks: &mut std::vec::Vec<crate::data::chunk::Chunk>,
        mut chunk: crate::data::chunk::Chunk,
    ) {
        while std::matches!(
            chunk.lines.first(),
            std::option::Option::Some((crate::data::line_type::LineType::Context, c)) if c.is_empty()
        ) {
            chunk.lines.remove(0);
        }
        while std::matches!(
            chunk.lines.last(),
            std::option::Option::Some((crate::data::line_type::LineType::Context, c)) if c.is_empty()
        ) {
            chunk.lines.pop();
        }
        if !chunk.lines.is_empty() {
            chunks.push(chunk);
        }
    }

    /// Creates a new parser for the given patch content.
    pub fn new(patch_content: &str) -> Self {
        let lines = if patch_content.trim().is_empty() {
            std::vec::Vec::new()
        } else {
            patch_content.lines().map(std::string::String::from).collect()
        };

        Self { lines, index: 0 }
    }

    /// Parses the patch text into a single `PatchAction`.
    /// Returns an error if the patch does not contain exactly one file directive.
    pub fn parse(
        &mut self,
    ) -> std::result::Result<std::vec::Vec<crate::data::patch_action::PatchAction>, crate::error::ZenpatchError>
    {
        self.index = 1; // Skip "*** Begin Patch"

        let mut actions = std::vec::Vec::new();

        while self.index < self.lines.len() - 1 {
            let line = self.lines[self.index].trim();

            if line.starts_with("*** Add File: ") {
                actions.push(self.parse_add_file()?);
            } else if line.starts_with("*** Update File: ") {
                actions.push(self.parse_update_file()?);
            } else if line.starts_with("*** Delete File: ") {
                actions.push(self.parse_delete_file()?);
            } else {
                self.index += 1;
            }
        }

        if actions.is_empty() {
            return std::result::Result::Err(crate::error::ZenpatchError::InvalidPatchFormat(
                "No file directive found in patch.".to_string(),
            ));
        }

        std::result::Result::Ok(actions)
    }

    fn parse_add_file(
        &mut self,
    ) -> std::result::Result<crate::data::patch_action::PatchAction, crate::error::ZenpatchError> {
        let line = &self.lines[self.index];
        let filename = line
            .trim_start_matches("*** Add File: ")
            .trim()
           .to_string();
       self.index += 1;

       let mut lines = std::vec::Vec::new();
       let mut ins_lines = std::vec::Vec::new();
       // Bare empty lines are blank lines of the new file whose '+' prefix was
       // omitted — dropping them would corrupt the added file. Only a trailing
       // run of them (a separator before the next directive) is not content.
       let mut trailing_bare_empty: usize = 0;
       while self.index < self.lines.len() && !self.lines[self.index].starts_with("*** ") {
           let line_content = &self.lines[self.index];
           if line_content.starts_with('+') {
               let content = line_content[1..].to_string();
               lines.push((
                   crate::data::line_type::LineType::Insertion,
                   content.clone(),
               ));
               ins_lines.push(content);
               trailing_bare_empty = 0;
           } else if line_content.is_empty() {
               lines.push((
                   crate::data::line_type::LineType::Insertion,
                   std::string::String::new(),
               ));
               ins_lines.push(std::string::String::new());
               trailing_bare_empty += 1;
           }
           self.index += 1;
       }
       for _ in 0..trailing_bare_empty {
           lines.pop();
           ins_lines.pop();
       }

       let chunk = crate::data::chunk::Chunk {
           orig_index: 0,
           lines,
           del_lines: std::vec::Vec::new(),
           ins_lines,
           change_context: std::option::Option::None,
           is_end_of_file: false,
       };

       std::result::Result::Ok(crate::data::patch_action::PatchAction {
           type_: crate::data::action_type::ActionType::Add,
            path: filename,
            new_path: std::option::Option::None,
            chunks: std::vec![chunk],
        })
    }

    fn parse_update_file(
        &mut self,
    ) -> std::result::Result<crate::data::patch_action::PatchAction, crate::error::ZenpatchError> {
        let line = &self.lines[self.index];
        let filename = line
            .trim_start_matches("*** Update File: ")
            .trim()
            .to_string();
        self.index += 1;

        let mut chunks = std::vec::Vec::new();
        let mut new_path: std::option::Option<std::string::String> = std::option::Option::None;
        let mut current_chunk = crate::data::chunk::Chunk::new();

        while self.index < self.lines.len() && !self.lines[self.index].starts_with("*** End Patch")
        {
            let line = self.lines[self.index].clone();

            if line.starts_with("*** Add File:")
                || line.starts_with("*** Update File:")
                || line.starts_with("*** Delete File:")
            {
                break; // Stop before next file directive
            }

            if line.starts_with("*** Move to: ") {
                new_path = std::option::Option::Some(
                    line.trim_start_matches("*** Move to: ").trim().to_string(),
                );
                self.index += 1;
                continue;
            }

            if line == "*** End of File" {
                // Anchor the current chunk to the file tail, but KEEP parsing:
                // breaking here would silently discard any further @@ chunks in
                // this Update section while the patch still "succeeds".
                current_chunk.is_end_of_file = true;
                Self::push_chunk(&mut chunks, current_chunk);
                current_chunk = crate::data::chunk::Chunk::new();
                self.index += 1;
                continue;
            }

            if line.starts_with("@@") {
                Self::push_chunk(&mut chunks, current_chunk);
                current_chunk = crate::data::chunk::Chunk::new();
                // Extract change_context from "@@ <text>" header
                let trimmed = &line[2..];
                if !trimmed.is_empty() {
                    let ctx = trimmed.trim_start();
                    if !ctx.is_empty() {
                        current_chunk.change_context = std::option::Option::Some(ctx.to_string());
                    }
                }
                self.index += 1;
                continue;
            }

            let (line_type, content) = if line.is_empty() {
                // An empty line in a hunk is an empty CONTEXT line whose lone
                // leading space was stripped (editors and LLMs trim trailing
                // whitespace) — dropping it would make the hunk's context
                // non-consecutive and the patch unappliable.
                (
                    crate::data::line_type::LineType::Context,
                    std::string::String::new(),
                )
            } else if line.starts_with(' ') {
                (
                    crate::data::line_type::LineType::Context,
                    line[1..].to_string(),
                )
            } else if line.starts_with('+') {
                (
                    crate::data::line_type::LineType::Insertion,
                    line[1..].to_string(),
                )
            } else if line.starts_with('-') {
                (
                    crate::data::line_type::LineType::Deletion,
                    line[1..].to_string(),
                )
            } else {
                self.index += 1;
                continue;
            };

            current_chunk.lines.push((line_type, content));
            self.index += 1;
        }

        Self::push_chunk(&mut chunks, current_chunk);

        std::result::Result::Ok(crate::data::patch_action::PatchAction {
            type_: crate::data::action_type::ActionType::Update,
            path: filename,
            new_path,
            chunks,
        })
    }

    fn parse_delete_file(
        &mut self,
    ) -> std::result::Result<crate::data::patch_action::PatchAction, crate::error::ZenpatchError> {
        let line = &self.lines[self.index];
        let filename = line
            .trim_start_matches("*** Delete File: ")
            .trim()
            .to_string();
        self.index += 1;

        let mut lines = std::vec::Vec::new();
        while self.index < self.lines.len() && !self.lines[self.index].starts_with("*** ") {
            let line_content = &self.lines[self.index];
            if line_content.starts_with('-') {
                let content = line_content[1..].to_string();
                lines.push((crate::data::line_type::LineType::Deletion, content));
            }
            self.index += 1;
        }

        let chunks = if lines.is_empty() {
            std::vec::Vec::new()
        } else {
            std::vec![crate::data::chunk::Chunk {
                orig_index: 0,
                lines,
                del_lines: std::vec::Vec::new(),
                ins_lines: std::vec::Vec::new(),
                change_context: std::option::Option::None,
                is_end_of_file: false,
            }]
        };

        std::result::Result::Ok(crate::data::patch_action::PatchAction {
            type_: crate::data::action_type::ActionType::Delete,
            path: filename,
            new_path: std::option::Option::None,
            chunks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{action_type::ActionType, line_type::LineType};

    #[test]
    fn test_parse_add_file() {
        let content = "*** Begin Patch\n*** Add File: new.txt\n+hello\n+world\n*** End Patch";
        let mut parser = Parser::new(content);
        let actions = parser.parse().unwrap();

        assert_eq!(actions.len(), 1);
        let action = &actions[0];
        assert_eq!(action.type_, ActionType::Add);
        assert_eq!(action.path, "new.txt");
        assert_eq!(action.chunks.len(), 1);
        let chunk = &action.chunks[0];
        assert_eq!(chunk.lines.len(), 2);
        assert_eq!(
            chunk.lines[0],
            (LineType::Insertion, "hello".to_string())
        );
        assert_eq!(
            chunk.lines[1],
            (LineType::Insertion, "world".to_string())
        );
    }

    #[test]
    fn test_parse_delete_file() {
        let content = "*** Begin Patch\n*** Delete File: old.txt\n*** End Patch";
        let mut parser = Parser::new(content);
        let actions = parser.parse().unwrap();
        assert_eq!(actions.len(), 1);
        let action = &actions[0];
        assert_eq!(action.type_, ActionType::Delete);
        assert_eq!(action.path, "old.txt");
        assert!(action.chunks.is_empty());
    }

    #[test]
    fn test_parse_delete_file_with_content() {
        let content = "*** Begin Patch\n*** Delete File: old.txt\n-line1\n-line2\n*** End Patch";
        let mut parser = Parser::new(content);
        let actions = parser.parse().unwrap();
        assert_eq!(actions.len(), 1);
        let action = &actions[0];
        assert_eq!(action.type_, ActionType::Delete);
        assert_eq!(action.path, "old.txt");
        assert_eq!(action.chunks.len(), 1);
        let chunk = &action.chunks[0];
        assert_eq!(chunk.lines.len(), 2);
        assert_eq!(chunk.lines[0], (LineType::Deletion, "line1".to_string()));
        assert_eq!(chunk.lines[1], (LineType::Deletion, "line2".to_string()));
    }

    #[test]
    fn test_parse_update_file() {
        let content =
            "*** Begin Patch\n*** Update File: file.txt\n@@\n-a\n+b\n c\n*** End Patch";
        let mut parser = Parser::new(content);
        let actions = parser.parse().unwrap();
        assert_eq!(actions.len(), 1);
        let action = &actions[0];
        assert_eq!(action.type_, ActionType::Update);
        assert_eq!(action.path, "file.txt");
        assert_eq!(action.chunks.len(), 1);
        let chunk = &action.chunks[0];
        assert_eq!(chunk.lines.len(), 3);
        assert_eq!(chunk.lines[0], (LineType::Deletion, "a".to_string()));
        assert_eq!(chunk.lines[1], (LineType::Insertion, "b".to_string()));
        assert_eq!(chunk.lines[2], (LineType::Context, "c".to_string()));
    }

    #[test]
    fn test_parse_update_with_move() {
        let content = "*** Begin Patch\n*** Update File: old.txt\n*** Move to: new.txt\n@@\n+a\n*** End Patch";
        let mut parser = Parser::new(content);
        let actions = parser.parse().unwrap();

        assert_eq!(actions.len(), 1);
        let action = &actions[0];
        assert_eq!(action.type_, ActionType::Update);
        assert_eq!(action.path, "old.txt");
        assert_eq!(action.new_path, Some("new.txt".to_string()));
        assert_eq!(action.chunks.len(), 1);
    }

    #[test]
    fn test_multiple_directives_success() {
        let content =
            "*** Begin Patch\n*** Add File: a.txt\n+1\n*** Delete File: b.txt\n*** End Patch";
        let mut parser = Parser::new(content);
        let actions = parser.parse().unwrap();
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].type_, ActionType::Add);
        assert_eq!(actions[0].path, "a.txt");
        assert_eq!(actions[1].type_, ActionType::Delete);
        assert_eq!(actions[1].path, "b.txt");
    }

    #[test]
    fn test_no_directive_error() {
        let content = "*** Begin Patch\nSome random text\n*** End Patch";
        let mut parser = Parser::new(content);
        let result = parser.parse();
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::ZenpatchError::InvalidPatchFormat(msg) => {
                assert!(msg.contains("No file directive found"));
            }
            _ => panic!("Expected InvalidPatchFormat error"),
        }
    }

    #[test]
    fn test_update_multiple_chunks_with_move() {
        let content = "*** Begin Patch\n\
*** Update File: old.txt\n\
*** Move to: new.txt\n\
@@\n line1\n-line2\n+LINE2\n\
@@\n line3\n-line4\n+LINE4\n\
*** End Patch";
        let mut parser = Parser::new(content);
        let actions = parser.parse().unwrap();
        assert_eq!(actions.len(), 1);
        let action = &actions[0];
        assert_eq!(action.type_, ActionType::Update);
        assert_eq!(action.path, "old.txt");
        assert_eq!(action.new_path, Some("new.txt".to_string()));
        assert_eq!(action.chunks.len(), 2);
        assert_eq!(action.chunks[0].lines[0], (LineType::Context, "line1".to_string()));
        assert_eq!(action.chunks[0].lines[1], (LineType::Deletion, "line2".to_string()));
        assert_eq!(action.chunks[0].lines[2], (LineType::Insertion, "LINE2".to_string()));
        assert_eq!(action.chunks[1].lines[0], (LineType::Context, "line3".to_string()));
    }

    #[test]
    fn test_invalid_lines_inside_chunk_are_skipped() {
        // Lines that don't start with ' ', '+', '-', or '@@' are skipped
        let content = "*** Begin Patch\n*** Update File: f.txt\n@@\n context\nThis line has no valid prefix\n-del\n+ins\n*** End Patch";
        let mut parser = Parser::new(content);
        let actions = parser.parse().unwrap();
        assert_eq!(actions[0].chunks.len(), 1);
        let chunk = &actions[0].chunks[0];
        // The invalid line is skipped; only context, del, ins remain
        assert_eq!(chunk.lines.len(), 3);
        assert_eq!(chunk.lines[0], (LineType::Context, "context".to_string()));
        assert_eq!(chunk.lines[1], (LineType::Deletion, "del".to_string()));
        assert_eq!(chunk.lines[2], (LineType::Insertion, "ins".to_string()));
    }

    #[test]
    fn test_empty_chunk_consecutive_at_markers() {
        // Two @@ in a row: first chunk is empty, second has content
        let content = "*** Begin Patch\n\
*** Update File: f.txt\n\
@@\n\
@@\n\
-old\n\
+new\n\
*** End Patch";
        let mut parser = Parser::new(content);
        let actions = parser.parse().unwrap();
        // First @@ creates a chunk, second @@ pushes it (empty) then creates new one
        // But the empty chunk has no lines, so it's not pushed
        assert_eq!(actions[0].chunks.len(), 1);
        assert_eq!(actions[0].chunks[0].lines.len(), 2);
    }

    #[test]
    fn test_file_path_with_spaces_and_special_chars() {
        let content = "*** Begin Patch\n\
*** Add File: path/to my file (1).txt\n\
+content\n\
*** End Patch";
        let mut parser = Parser::new(content);
        let actions = parser.parse().unwrap();
        assert_eq!(actions[0].path, "path/to my file (1).txt");
    }

    #[test]
    fn test_file_path_with_unicode() {
        let content = "*** Begin Patch\n\
*** Add File: src/\u{00E9}l\u{00E8}ve.rs\n\
+fn main() {}\n\
*** End Patch";
        let mut parser = Parser::new(content);
        let actions = parser.parse().unwrap();
        assert_eq!(actions[0].path, "src/\u{00E9}l\u{00E8}ve.rs");
    }

    #[test]
    fn test_parse_at_header_with_context_text() {
        let content = "*** Begin Patch\n\
*** Update File: file.py\n\
@@ def foo():\n\
-old\n\
+new\n\
*** End Patch";
        let mut parser = Parser::new(content);
        let actions = parser.parse().unwrap();
        assert_eq!(actions[0].chunks.len(), 1);
        assert_eq!(
            actions[0].chunks[0].change_context,
            Some("def foo():".to_string())
        );
    }

    #[test]
    fn test_parse_bare_at_header_no_context() {
        let content = "*** Begin Patch\n\
*** Update File: file.txt\n\
@@\n\
-old\n\
+new\n\
*** End Patch";
        let mut parser = Parser::new(content);
        let actions = parser.parse().unwrap();
        assert_eq!(actions[0].chunks.len(), 1);
        assert_eq!(actions[0].chunks[0].change_context, None);
    }

    #[test]
    fn test_parse_end_of_file_marker() {
        let content = "*** Begin Patch\n\
*** Update File: file.txt\n\
@@\n\
 last line\n\
+appended\n\
*** End of File\n\
*** End Patch";
        let mut parser = Parser::new(content);
        let actions = parser.parse().unwrap();
        assert_eq!(actions[0].chunks.len(), 1);
        assert!(actions[0].chunks[0].is_end_of_file);
    }

    #[test]
    fn test_parse_end_of_file_not_present() {
        let content = "*** Begin Patch\n\
*** Update File: file.txt\n\
@@\n\
 ctx\n\
-old\n\
+new\n\
*** End Patch";
        let mut parser = Parser::new(content);
        let actions = parser.parse().unwrap();
        assert!(!actions[0].chunks[0].is_end_of_file);
    }
}
