//! Provides a function to retrieve the LLM-facing patch format documentation.
//!
//! This file contains a single function, `get_llm_instructions`, which
//! returns a static string slice containing the contents of `llms.txt`.
//! This is useful for providing instructions to AI agents on how to
//! construct valid patches for this library.

pub fn get_llm_instructions() -> &'static str {
    std::include_str!("../llms.txt")
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_instructions_are_not_empty() {
        let instructions = super::get_llm_instructions();
        std::assert!(!instructions.is_empty(), "Instructions string should not be empty.");
        std::assert!(instructions.contains("Zenpatch Patch Format for LLMs"), "Instructions should contain the title.");
    }
}