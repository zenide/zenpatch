//! Implements backtracking-based patch application for a sequence of chunks.
//!
//! Uses exhaustive search to find a unique, non-overlapping application sequence
//! for all chunks, applying deletions and insertions in turn. Fails on ambiguity
//! or conflict. Conforms to rust coding guidelines (one item per file).

use crate::applier::state::BacktrackingState;
use crate::applier::whitespace_mode::WhitespaceMode;
use crate::data::chunk::Chunk;
use crate::data::line_type::LineType;
use crate::error::ZenpatchError;

/// Maximum allowed backtracking nodes before giving up as "ambiguous".
const MAX_BACKTRACK_NODES: usize = 100_000;

fn super_normalise(s: &str) -> String {
    s.trim()
        .chars()
        .map(|c| match c {
            // Various dash / hyphen code-points → ASCII '-'
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}'
            | '\u{2212}' => '-',
            // Fancy single quotes → '\''
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => '\'',
            // Fancy double quotes → '"'
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => '"',
            // Non-breaking space and other odd spaces → normal space
            '\u{00A0}' | '\u{2002}' | '\u{2003}' | '\u{2004}' | '\u{2005}' | '\u{2006}'
            | '\u{2007}' | '\u{2008}' | '\u{2009}' | '\u{200A}' | '\u{202F}' | '\u{205F}'
            | '\u{3000}' => ' ',
            other => other,
        })
        .collect::<String>()
}

fn normalize(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Compares two lines according to whitespace mode: exact or trimmed.
fn match_line(a: &str, b: &str, mode: WhitespaceMode) -> bool {
    match mode {
        WhitespaceMode::Strict => a == b,
        WhitespaceMode::Lenient => {
            normalize(a) == normalize(b)
        },
        WhitespaceMode::SuperLenient => {
            super_normalise(&normalize(a)) == super_normalise(&normalize(b))
        }
    }
}

/// Builds a precise message when no application sequence exists, so the caller (and any LLM
/// reading the error) can fix the patch instead of guessing. The dominant failure is a context
/// (` `) or deletion (`-`) line that does not exist in the file at all — almost always a line
/// the patch author invented or mistyped — so we name the FIRST such line verbatim. If every
/// such line does exist individually but not as a consecutive block, the patch has an ordering /
/// extra-line problem, which we say instead.
fn diagnose_conflict(original_lines: &[String], chunks: &[Chunk], mode: WhitespaceMode) -> String {
    for chunk in chunks {
        for (line_type, content) in &chunk.lines {
            if matches!(line_type, LineType::Context | LineType::Deletion) {
                let exists = original_lines.iter().any(|l| match_line(l, content, mode));
                if !exists {
                    return format!(
                        "this context/deleted line does not exist in the file (it was likely \
                         invented, mistyped, or has wrong whitespace — copy lines verbatim from \
                         the file): \"{}\"",
                        content.trim_end()
                    );
                }
            }
        }
    }
    "the patch's context/deleted lines all exist in the file but not as one consecutive block — \
     a line is out of order, duplicated, or there is an extra line inserted between context lines; \
     re-copy a contiguous run of real lines around the change"
        .to_string()
}

/// Applies patch chunks using strict or lenient whitespace matching.
/// Wrapper that defaults to strict mode.
pub fn apply_patch_backtracking(
    original_lines: &[String],
    chunks: &[Chunk],
) -> Result<Vec<String>, ZenpatchError> {
    apply_patch_backtracking_mode(original_lines, chunks, WhitespaceMode::Strict)
}

/// Core backtracking patcher with configurable whitespace mode.
pub fn apply_patch_backtracking_mode(
    original_lines: &[String],
    chunks: &[Chunk],
    mode: WhitespaceMode,
) -> Result<Vec<String>, ZenpatchError> {
    if original_lines.is_empty() && chunks.iter().all(|c| c.del_lines.is_empty()) {
        let result: Vec<String> = chunks.iter()
            .flat_map(|c| c.ins_lines.iter().cloned())
            .collect();
        return Ok(result);
    }

    // The original file never changes during the search, so each chunk's
    // candidate positions (context match + deletion content check) are
    // computed exactly once here instead of at every search node.
    let valid_positions: Vec<Vec<usize>> = chunks
        .iter()
        .map(|chunk| valid_positions_for_chunk(original_lines, chunk, mode))
        .collect();

    // Content class per chunk: identical chunks share a class, so solution
    // keys are invariant under permutations of interchangeable chunks.
    let chunk_classes: Vec<usize> = chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            chunks[..i]
                .iter()
                .position(|earlier| earlier == chunk)
                .unwrap_or(i)
        })
        .collect();

    // Hunks almost always appear in file order, and for interchangeable
    // hunks the patch encodes WHICH occurrence each one targets only
    // through that order. So: try first with positions required to be
    // non-decreasing in hunk order. This both prunes the search and
    // legitimately resolves repeated-pattern patches that are ambiguous
    // without it. Ordered solutions are a subset of unordered ones, so an
    // ambiguity verdict here is final; only "no solution at all" falls
    // back to the unordered search (out-of-order hunks).
    let (mut current_path, mut state) = find_fixed_mappings(chunks, &valid_positions, mode);
    let ordered_ctx = SearchCtx {
        lines: original_lines,
        chunks,
        valid_positions: &valid_positions,
        chunk_classes: &chunk_classes,
        mode,
        ordered: true,
    };
    backtrack_with_mode(&ordered_ctx, &mut state, &mut current_path);

    if state.solution_count == 0 {
        let (path, st) = find_fixed_mappings(chunks, &valid_positions, mode);
        current_path = path;
        state = st;
        let unordered_ctx = SearchCtx { ordered: false, ..ordered_ctx };
        backtrack_with_mode(&unordered_ctx, &mut state, &mut current_path);
    }

    if state.solution_count == 0 {
        return Err(ZenpatchError::PatchConflict(diagnose_conflict(
            original_lines,
            chunks,
            mode,
        )));
    }
    if state.solution_count > 1 {
        return Err(ZenpatchError::AmbiguousPatch(
            "Patch application is ambiguous - please include more context before or after insertions or deletions".to_string()
        ));
    }

    Ok(state
        .first_solution_result
        .expect("first_solution_result must be set"))
}

/// Length of the chunk's leading context run, adjusted for the
/// duplicated-line case where the last context line equals the first
/// deleted line (the two refer to the same file line).
fn adjusted_pre_len(chunk: &Chunk, mode: WhitespaceMode) -> usize {
    let mut pre_len = 0;
    for (lt, _) in chunk.lines.iter() {
        if *lt == LineType::Context {
            pre_len += 1;
        } else {
            break;
        }
    }
    if pre_len > 0 && !chunk.del_lines.is_empty() {
        let last_ctx = &chunk.lines[pre_len - 1].1;
        if let Some((LineType::Deletion, del)) = chunk.lines.get(pre_len) {
            if match_line(last_ctx, del, mode) {
                return pre_len - 1;
            }
        }
    }
    pre_len
}

/// Candidate positions for a chunk: context matches whose deletion block
/// also matches the file content at that offset.
fn valid_positions_for_chunk(
    lines: &[String],
    chunk: &Chunk,
    mode: WhitespaceMode,
) -> Vec<usize> {
    let adj_pre = adjusted_pre_len(chunk, mode);
    find_match_positions(lines, chunk, mode)
        .into_iter()
        .filter(|&pos| {
            chunk.del_lines.iter().enumerate().all(|(j, del_line)| {
                let idx = pos + adj_pre + j;
                idx < lines.len() && match_line(&lines[idx], del_line, mode)
            })
        })
        .collect()
}

/// The original-file index range consumed (deleted) by a chunk matched at `pos`.
fn affected_range(chunk: &Chunk, pos: usize, mode: WhitespaceMode) -> std::ops::Range<usize> {
    let start = pos + adjusted_pre_len(chunk, mode);
    start..start + chunk.del_lines.len()
}

/// Applies a complete (chunk index, original position) mapping to the
/// original lines, in ascending position order with a running offset.
fn materialize_solution(
    lines: &[String],
    chunks: &[Chunk],
    mapping: &[(usize, usize)],
    mode: WhitespaceMode,
) -> Vec<String> {
    let mut ordered: Vec<(usize, usize)> = mapping.to_vec();
    ordered.sort_by_key(|&(_, pos)| pos);
    let mut result = lines.to_vec();
    let mut delta: isize = 0;
    for (chunk_idx, orig_pos) in ordered {
        let chunk = &chunks[chunk_idx];
        let pos = if delta >= 0 {
            (orig_pos as isize + delta) as usize
        } else {
            orig_pos.saturating_sub((-delta) as usize)
        };
        result = apply_chunk(&result, chunk, pos, mode);
        delta += chunk.ins_lines.len() as isize - chunk.del_lines.len() as isize;
    }
    result
}

/// Pre-commits every chunk that has exactly one valid, non-overlapping
/// position — these need no search at all.
fn find_fixed_mappings(
    chunks: &[Chunk],
    valid_positions: &[Vec<usize>],
    mode: WhitespaceMode,
) -> (Vec<(usize, usize)>, BacktrackingState) {
    let mut result_path = Vec::new();
    let mut state = BacktrackingState::new();

    for (chunk_idx, chunk) in chunks.iter().enumerate() {
        if let [pos] = valid_positions[chunk_idx][..] {
            let affected = affected_range(chunk, pos, mode);
            if affected.clone().all(|idx| !state.modified_indices.contains(&idx)) {
                state.applied_chunks.insert(chunk_idx);
                for idx in affected {
                    state.modified_indices.insert(idx);
                }
                result_path.push((chunk_idx, pos));
            }
        }
    }

    (result_path, state)
}


fn get_pre_context_lines(chunk: &Chunk) -> Vec<String> {
    let mut ctx: Vec<String> = Vec::new();
    for (line_type, content) in chunk.lines.iter() {
        if *line_type == LineType::Context {
            ctx.push(content.clone());
        } else {
            break;
        }
    }
    ctx
}

fn apply_chunk_constraints(
    positions: Vec<usize>,
    lines: &[String],
    chunk: &Chunk,
    mode: WhitespaceMode,
) -> Vec<usize> {
    let mut filtered = positions;

    // Filter by change_context: only keep positions strictly after the line matching the context
    if let Some(ref ctx) = chunk.change_context {
        let anchor = lines.iter().position(|l| match_line(l, ctx, mode));
        if let Some(anchor_idx) = anchor {
            filtered.retain(|&pos| pos > anchor_idx);
        } else {
            // Context string not found anywhere in the file → no valid positions
            return Vec::new();
        }
    }

    // Filter by is_end_of_file: the matched region must reach the end of the file
    if chunk.is_end_of_file {
        let pre_len = get_pre_context_lines(chunk).len();
        let span = pre_len + chunk.del_lines.len();
        // For pure insertions with context, the context + insertion should land at the end
        let effective_span = if span == 0 { 0 } else { span };
        filtered.retain(|&pos| pos + effective_span >= lines.len());
    }

    filtered
}

fn find_match_positions(
    lines: &[String],
    chunk: &Chunk,
    mode: WhitespaceMode,
) -> Vec<usize> {
    let pre = get_pre_context_lines(chunk);
    let mut positions: Vec<usize> = Vec::new();
    if pre.is_empty() {
        // No leading context: pure insertion or deletion
        if chunk.del_lines.is_empty() {
            // Pure insertion: use original index as insertion point
            positions.push(chunk.orig_index.min(lines.len()));
        } else {
            // Pure deletion: scan for all matching deletion sequences
            let del_len = chunk.del_lines.len();
            if del_len > 0 && lines.len() >= del_len {
                for i in 0..=lines.len() - del_len {
                    let mut ok = true;
                    for (j, del_line) in chunk.del_lines.iter().enumerate() {
                        if !match_line(&lines[i + j], del_line, mode) {
                            ok = false;
                            break;
                        }
                    }
                    if ok {
                        positions.push(i);
                    }
                }
            }
        }
        return apply_chunk_constraints(positions, lines, chunk, mode);
    }

    let clen = pre.len();
    if lines.len() < clen {
        return apply_chunk_constraints(positions, lines, chunk, mode);
    }

    let max_start = lines.len() - clen;
    for i in 0..=max_start {
        if pre.iter().enumerate().all(|(j, ctx)| match_line(&lines[i + j], ctx, mode)) {
            positions.push(i);
        }
    }
    // collect trailing context (post-context) for potential disambiguation
    let post_context: Vec<String> = {
        let mut ctx: Vec<String> = Vec::new();
        for (lt, content) in chunk.lines.iter().rev() {
            if *lt == LineType::Context {
                if !content.trim().is_empty() {
                    ctx.push(content.clone());
                }
            } else {
                break;
            }
        }
        ctx.reverse();
        ctx
    };

    // For pure insertions (no deletions), attempt to disambiguate using post-context
    if chunk.del_lines.is_empty() && !chunk.ins_lines.is_empty() && !post_context.is_empty() {
        // use the first post-context line as an anchor
        let anchor = &post_context[0];
        let pre_full_len = get_pre_context_lines(chunk).len();
        let mut filtered: Vec<usize> = Vec::new();
        for &pos in &positions {
            // search within a small window after pre-context for the anchor line
            let start = pos + pre_full_len;
            let end = std::cmp::min(lines.len(), start + pre_full_len + 10);
            if (start..end).any(|i| match_line(&lines[i], anchor, mode)) {
                filtered.push(pos);
            }
        }
        positions = filtered;
    }
    // fallback to anchor on last pre-context line if still no positions in lenient mode and no post-context
    if post_context.is_empty() && positions.is_empty() && matches!(mode, WhitespaceMode::Lenient) && !pre.is_empty() {
        let anchor_idx = pre.len() - 1;
        let anchor_line = &pre[anchor_idx];
        for (i, orig_line) in lines.iter().enumerate() {
            if match_line(orig_line, anchor_line, WhitespaceMode::Lenient) {
                positions.push(i.saturating_sub(anchor_idx));
            }
        }
    }

    apply_chunk_constraints(positions, lines, chunk, mode)
}

fn apply_chunk(lines: &[String], chunk: &Chunk, pos: usize, mode: WhitespaceMode) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    let adj_pre = adjusted_pre_len(chunk, mode);

    let start_copy = (pos + adj_pre).min(lines.len());
    result.extend_from_slice(&lines[..start_copy]);
    result.extend(chunk.ins_lines.iter().cloned());

    let end_del = (pos + adj_pre + chunk.del_lines.len()).min(lines.len());
    result.extend_from_slice(&lines[end_del..]);
    result
}

/// Immutable inputs of the backtracking search, fixed for its whole duration.
struct SearchCtx<'a> {
    lines: &'a [String],
    chunks: &'a [Chunk],
    /// Pre-computed candidate positions per chunk (same indexing as `chunks`).
    valid_positions: &'a [Vec<usize>],
    /// Content class per chunk: index of the first chunk with equal content.
    chunk_classes: &'a [usize],
    mode: WhitespaceMode,
    /// When set, chunk positions must be non-decreasing in chunk order
    /// (hunks appear in file order).
    ordered: bool,
}

fn backtrack_with_mode(
    ctx: &SearchCtx<'_>,
    state: &mut BacktrackingState,
    current_path: &mut Vec<(usize, usize)>,
) {
    let SearchCtx { lines, chunks, valid_positions, chunk_classes, mode, ordered } = *ctx;
    state.nodes_visited += 1;
    if state.nodes_visited > MAX_BACKTRACK_NODES || state.solution_count > 1 {
        state.solution_count = 2;
        return;
    }

    if current_path.len() == chunks.len() {
        // Different application orders can produce the same file; only a
        // DIFFERENT resulting file counts as a second (ambiguous) solution.
        // Mappings with equal (position, chunk content) keys are identical
        // by construction — dedup them without materializing the file.
        let mut key: Vec<(usize, usize)> = current_path
            .iter()
            .map(|&(chunk_idx, pos)| (pos, chunk_classes[chunk_idx]))
            .collect();
        key.sort_unstable();
        if state.first_solution_key.as_ref() == Some(&key) {
            return;
        }
        let candidate = materialize_solution(lines, chunks, current_path, mode);
        match &state.first_solution_result {
            None => {
                state.solution_count = 1;
                state.first_solution_result = Some(candidate);
                state.solution_path = Some(current_path.clone());
                state.first_solution_key = Some(key);
            }
            Some(first) if *first == candidate => {}
            Some(_) => state.solution_count = 2,
        }
        return;
    }

    let min_orig = chunks.iter().enumerate()
        .filter(|(j, _)| !state.applied_chunks.contains(j))
        .map(|(_, c)| c.orig_index)
        .min();

    for (i, chunk) in chunks.iter().enumerate() {
        if state.applied_chunks.contains(&i) {
            continue;
        }
        if let Some(min_o) = min_orig {
            if chunk.orig_index != min_o {
                continue;
            }
        }

        for &pos in &valid_positions[i] {
            // File-order constraint, checked against every placement so far
            // (including pre-committed fixed mappings on either side).
            if ordered
                && current_path
                    .iter()
                    .any(|&(j, pj)| (j < i && pj > pos) || (j > i && pj < pos))
            {
                continue;
            }

            let affected = affected_range(chunk, pos, mode);
            if affected.clone().any(|idx| state.modified_indices.contains(&idx)) {
                continue;
            }

            // Do/undo on the ONE shared state instead of cloning it per node.
            state.applied_chunks.insert(i);
            for idx in affected.clone() {
                state.modified_indices.insert(idx);
            }
            current_path.push((i, pos));

            backtrack_with_mode(ctx, state, current_path);

            current_path.pop();
            for idx in affected {
                state.modified_indices.remove(&idx);
            }
            state.applied_chunks.remove(&i);

            if state.solution_count > 1 {
                return;
            }
        }

        // Branch ONLY on the first eligible chunk. A complete solution
        // assigns every chunk a position regardless of assignment order,
        // and feasibility (overlap) is order-independent — trying other
        // chunks first would only re-enumerate the same mappings in a
        // different order (a factorial blow-up that hit the node cap and
        // misreported unique solutions as ambiguous). For the same reason,
        // if this chunk has no feasible position now it never will on this
        // path, so the path is dead.
        return;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::chunk::Chunk;
    use crate::data::line_type::LineType;

    // ── match_line tests ──

    #[test]
    fn test_match_line_strict_exact() {
        assert!(match_line("hello world", "hello world", WhitespaceMode::Strict));
    }

    #[test]
    fn test_match_line_strict_whitespace_differs() {
        assert!(!match_line("hello  world", "hello world", WhitespaceMode::Strict));
        assert!(!match_line("  hello", "hello", WhitespaceMode::Strict));
    }

    #[test]
    fn test_match_line_lenient_collapses_whitespace() {
        assert!(match_line("hello  world", "hello world", WhitespaceMode::Lenient));
        assert!(match_line("  hello  ", "hello", WhitespaceMode::Lenient));
        assert!(match_line("\thello\tworld", "hello world", WhitespaceMode::Lenient));
    }

    #[test]
    fn test_match_line_lenient_different_content() {
        assert!(!match_line("hello", "world", WhitespaceMode::Lenient));
    }

    #[test]
    fn test_match_line_super_lenient_fancy_quotes() {
        assert!(match_line(
            "\u{201C}hello\u{201D}",
            "\"hello\"",
            WhitespaceMode::SuperLenient
        ));
        assert!(match_line(
            "\u{2018}it\u{2019}s\u{2019}",
            "'it's'",
            WhitespaceMode::SuperLenient
        ));
    }

    #[test]
    fn test_match_line_super_lenient_dashes() {
        assert!(match_line("a\u{2014}b", "a-b", WhitespaceMode::SuperLenient));
        assert!(match_line("a\u{2013}b", "a-b", WhitespaceMode::SuperLenient));
        assert!(match_line("a\u{2212}b", "a-b", WhitespaceMode::SuperLenient));
    }

    #[test]
    fn test_match_line_super_lenient_special_spaces() {
        assert!(match_line(
            "hello\u{00A0}world",
            "hello world",
            WhitespaceMode::SuperLenient
        ));
        assert!(match_line(
            "hello\u{2003}world",
            "hello world",
            WhitespaceMode::SuperLenient
        ));
    }

    // ── normalize / super_normalise tests ──

    #[test]
    fn test_normalize_collapses_whitespace() {
        assert_eq!(normalize("  hello   world  "), "hello world");
        assert_eq!(normalize("a"), "a");
        assert_eq!(normalize(""), "");
        assert_eq!(normalize("  \t\n  "), "");
    }

    #[test]
    fn test_super_normalise_fancy_characters() {
        assert_eq!(super_normalise("\u{201C}hi\u{201D}"), "\"hi\"");
        assert_eq!(super_normalise("\u{2018}hi\u{2019}"), "'hi'");
        assert_eq!(super_normalise("a\u{2014}b"), "a-b");
        assert_eq!(super_normalise("\u{00A0}hi\u{00A0}"), "hi");
    }

    #[test]
    fn test_super_normalise_trims() {
        assert_eq!(super_normalise("  hello  "), "hello");
    }

    // ── apply_patch_backtracking direct tests ──

    fn make_chunk(
        context_before: &[&str],
        deletions: &[&str],
        insertions: &[&str],
        context_after: &[&str],
        orig_index: usize,
    ) -> Chunk {
        let mut lines = Vec::new();
        for c in context_before {
            lines.push((LineType::Context, c.to_string()));
        }
        for d in deletions {
            lines.push((LineType::Deletion, d.to_string()));
        }
        for i in insertions {
            lines.push((LineType::Insertion, i.to_string()));
        }
        for c in context_after {
            lines.push((LineType::Context, c.to_string()));
        }
        Chunk {
            orig_index,
            lines,
            del_lines: deletions.iter().map(|s| s.to_string()).collect(),
            ins_lines: insertions.iter().map(|s| s.to_string()).collect(),
            change_context: None,
            is_end_of_file: false,
        }
    }

    #[test]
    fn test_single_chunk_replacement() {
        let original: Vec<String> = vec!["aaa", "bbb", "ccc"]
            .into_iter().map(String::from).collect();
        let chunk = make_chunk(&["aaa"], &["bbb"], &["BBB"], &["ccc"], 0);
        let result = apply_patch_backtracking(&original, &[chunk]).unwrap();
        assert_eq!(result, vec!["aaa", "BBB", "ccc"]);
    }

    #[test]
    fn test_pure_insertion_with_context() {
        let original: Vec<String> = vec!["aaa", "ccc"]
            .into_iter().map(String::from).collect();
        let chunk = make_chunk(&["aaa"], &[], &["bbb"], &["ccc"], 0);
        let result = apply_patch_backtracking(&original, &[chunk]).unwrap();
        assert_eq!(result, vec!["aaa", "bbb", "ccc"]);
    }

    #[test]
    fn test_pure_deletion() {
        let original: Vec<String> = vec!["aaa", "bbb", "ccc"]
            .into_iter().map(String::from).collect();
        let chunk = make_chunk(&["aaa"], &["bbb"], &[], &["ccc"], 0);
        let result = apply_patch_backtracking(&original, &[chunk]).unwrap();
        assert_eq!(result, vec!["aaa", "ccc"]);
    }

    #[test]
    fn test_empty_file_pure_insertion() {
        let original: Vec<String> = vec![];
        let chunk = make_chunk(&[], &[], &["hello", "world"], &[], 0);
        let result = apply_patch_backtracking(&original, &[chunk]).unwrap();
        assert_eq!(result, vec!["hello", "world"]);
    }

    #[test]
    fn test_conflict_context_not_found() {
        let original: Vec<String> = vec!["aaa", "bbb"]
            .into_iter().map(String::from).collect();
        let chunk = make_chunk(&["zzz"], &["bbb"], &["BBB"], &[], 0);
        let result = apply_patch_backtracking(&original, &[chunk]);
        assert!(matches!(result, Err(ZenpatchError::PatchConflict(_))));
    }

    #[test]
    fn test_ambiguous_patch_repeated_context() {
        let original: Vec<String> = vec!["aaa", "bbb", "aaa", "bbb"]
            .into_iter().map(String::from).collect();
        let chunk = make_chunk(&["aaa"], &["bbb"], &["BBB"], &[], 0);
        let result = apply_patch_backtracking(&original, &[chunk]);
        assert!(matches!(result, Err(ZenpatchError::AmbiguousPatch(_))));
    }

    #[test]
    fn test_multiple_chunks_non_overlapping() {
        let original: Vec<String> = vec!["aaa", "bbb", "ccc", "ddd", "eee"]
            .into_iter().map(String::from).collect();
        let chunk1 = make_chunk(&["aaa"], &["bbb"], &["BBB"], &[], 0);
        let chunk2 = make_chunk(&["ddd"], &["eee"], &["EEE"], &[], 3);
        let result = apply_patch_backtracking(&original, &[chunk1, chunk2]).unwrap();
        assert_eq!(result, vec!["aaa", "BBB", "ccc", "ddd", "EEE"]);
    }

    #[test]
    fn test_lenient_mode_whitespace_difference() {
        let original: Vec<String> = vec!["  aaa", "bbb", "ccc"]
            .into_iter().map(String::from).collect();
        let chunk = make_chunk(&["aaa"], &["bbb"], &["BBB"], &["ccc"], 0);
        let result = apply_patch_backtracking_mode(
            &original, &[chunk], WhitespaceMode::Lenient,
        ).unwrap();
        assert_eq!(result, vec!["  aaa", "BBB", "ccc"]);
    }

    #[test]
    fn test_super_lenient_mode_fancy_quotes() {
        let original: Vec<String> = vec!["say \"hello\"", "next"]
            .into_iter().map(String::from).collect();
        let chunk = make_chunk(
            &["say \u{201C}hello\u{201D}"],
            &["next"],
            &["NEXT"],
            &[],
            0,
        );
        let result = apply_patch_backtracking_mode(
            &original, &[chunk], WhitespaceMode::SuperLenient,
        ).unwrap();
        assert_eq!(result, vec!["say \"hello\"", "NEXT"]);
    }

    #[test]
    fn test_multiple_insertions_empty_file() {
        let original: Vec<String> = vec![];
        let chunk1 = make_chunk(&[], &[], &["line1"], &[], 0);
        let chunk2 = make_chunk(&[], &[], &["line2"], &[], 0);
        let result = apply_patch_backtracking(&original, &[chunk1, chunk2]).unwrap();
        assert_eq!(result, vec!["line1", "line2"]);
    }

    #[test]
    fn test_deletion_at_file_start() {
        let original: Vec<String> = vec!["aaa", "bbb", "ccc"]
            .into_iter().map(String::from).collect();
        let chunk = make_chunk(&[], &["aaa"], &[], &["bbb"], 0);
        let result = apply_patch_backtracking(&original, &[chunk]).unwrap();
        assert_eq!(result, vec!["bbb", "ccc"]);
    }

    #[test]
    fn test_deletion_at_file_end() {
        let original: Vec<String> = vec!["aaa", "bbb", "ccc"]
            .into_iter().map(String::from).collect();
        let chunk = make_chunk(&["bbb"], &["ccc"], &[], &[], 1);
        let result = apply_patch_backtracking(&original, &[chunk]).unwrap();
        assert_eq!(result, vec!["aaa", "bbb"]);
    }

    #[test]
    fn test_replace_entire_file() {
        let original: Vec<String> = vec!["old"]
            .into_iter().map(String::from).collect();
        let chunk = make_chunk(&[], &["old"], &["new"], &[], 0);
        let result = apply_patch_backtracking(&original, &[chunk]).unwrap();
        assert_eq!(result, vec!["new"]);
    }

    /// Several IDENTICAL chunks matching several positions force the search
    /// through every assignment permutation; all orders produce the same
    /// file, so this must resolve as ONE solution, not ambiguity.
    #[test]
    fn test_identical_chunks_permutations_dedup_to_single_solution() {
        let mut original: Vec<String> = Vec::new();
        for i in 0..5 {
            original.push(format!("keep {i}"));
            original.push("dup".to_string());
        }
        let chunks: Vec<Chunk> = (0..5)
            .map(|_| make_chunk(&[], &["dup"], &[], &[], 0))
            .collect();
        let result = apply_patch_backtracking(&original, &chunks).unwrap();
        assert_eq!(
            result,
            vec!["keep 0", "keep 1", "keep 2", "keep 3", "keep 4"]
        );
    }

    /// Regression: 7 interchangeable chunks used to explode the search into
    /// chunk-order × position permutations, hit the node cap, and misreport
    /// the unique solution as AmbiguousPatch. With order-free enumeration
    /// and key-based dedup this must apply, and quickly.
    #[test]
    fn test_many_identical_chunks_resolve_instead_of_false_ambiguity() {
        let mut original: Vec<String> = Vec::new();
        for i in 0..2000 {
            original.push(format!("keep {i}"));
            if i % 300 == 0 {
                original.push("dup".to_string());
            }
        }
        let chunks: Vec<Chunk> = (0..7)
            .map(|_| make_chunk(&[], &["dup"], &[], &[], 0))
            .collect();
        let result = apply_patch_backtracking(&original, &chunks).unwrap();
        assert_eq!(result.len(), 2000);
        assert!(result.iter().all(|l| l != "dup"));
    }

    // ── ordered-first (file-order) tests ──

    /// Two hunks targeting two identical regions: without the file-order
    /// constraint the assignment is ambiguous (X/Y could swap). Patch order
    /// is the only encoding of which occurrence each hunk targets — ordered
    /// search resolves it.
    #[test]
    fn test_file_order_resolves_interchangeable_hunks() {
        let original: Vec<String> = vec!["marker", "target", "marker", "target"]
            .into_iter().map(String::from).collect();
        let chunk1 = make_chunk(&["marker"], &["target"], &["X"], &[], 0);
        let chunk2 = make_chunk(&["marker"], &["target"], &["Y"], &[], 0);
        let result = apply_patch_backtracking(&original, &[chunk1, chunk2]).unwrap();
        assert_eq!(result, vec!["marker", "X", "marker", "Y"]);
    }

    /// Order does NOT fabricate uniqueness: two hunks over three identical
    /// regions have several in-order assignments with different results —
    /// still ambiguous.
    #[test]
    fn test_file_order_does_not_mask_real_ambiguity() {
        let original: Vec<String> = vec!["target", "target", "target"]
            .into_iter().map(String::from).collect();
        let chunk1 = make_chunk(&[], &["target"], &["X"], &[], 0);
        let chunk2 = make_chunk(&[], &["target"], &["Y"], &[], 0);
        let result = apply_patch_backtracking(&original, &[chunk1, chunk2]);
        assert!(matches!(result, Err(ZenpatchError::AmbiguousPatch(_))));
    }

    /// Out-of-order hunks with unique placements still apply via the
    /// unordered fallback.
    #[test]
    fn test_out_of_order_unique_hunks_fall_back_and_apply() {
        let original: Vec<String> = vec!["aaa", "mid", "zzz"]
            .into_iter().map(String::from).collect();
        let chunk1 = make_chunk(&[], &["zzz"], &["ZZZ"], &[], 0);
        let chunk2 = make_chunk(&[], &["aaa"], &["AAA"], &[], 0);
        let result = apply_patch_backtracking(&original, &[chunk1, chunk2]).unwrap();
        assert_eq!(result, vec!["AAA", "mid", "ZZZ"]);
    }

    // ── change_context constraint tests ──

    #[test]
    fn test_change_context_narrows_repeated_pattern() {
        // File has two identical "marker" / "target" blocks.
        // Without change_context both would match → ambiguous.
        // With change_context pointing to "class Bar", only the second matches.
        let original: Vec<String> = vec![
            "class Foo:",
            "  marker",
            "  target",
            "class Bar:",
            "  marker",
            "  target",
        ].into_iter().map(String::from).collect();

        let mut chunk = make_chunk(&["  marker"], &["  target"], &["  REPLACED"], &[], 0);
        chunk.change_context = Some("class Bar:".to_string());

        let result = apply_patch_backtracking(&original, &[chunk]).unwrap();
        assert_eq!(result, vec![
            "class Foo:",
            "  marker",
            "  target",
            "class Bar:",
            "  marker",
            "  REPLACED",
        ]);
    }

    #[test]
    fn test_change_context_not_found_is_conflict() {
        let original: Vec<String> = vec!["aaa", "bbb", "ccc"]
            .into_iter().map(String::from).collect();
        let mut chunk = make_chunk(&["aaa"], &["bbb"], &["BBB"], &[], 0);
        chunk.change_context = Some("nonexistent".to_string());

        let result = apply_patch_backtracking(&original, &[chunk]);
        assert!(matches!(result, Err(ZenpatchError::PatchConflict(_))));
    }

    // ── is_end_of_file constraint tests ──

    #[test]
    fn test_is_end_of_file_constrains_to_end() {
        // File has "marker" / "target" appearing twice.
        // is_end_of_file should force matching only the one at the end.
        let original: Vec<String> = vec![
            "marker",
            "target",
            "middle",
            "marker",
            "target",
        ].into_iter().map(String::from).collect();

        let mut chunk = make_chunk(&["marker"], &["target"], &["REPLACED"], &[], 0);
        chunk.is_end_of_file = true;

        let result = apply_patch_backtracking(&original, &[chunk]).unwrap();
        assert_eq!(result, vec![
            "marker",
            "target",
            "middle",
            "marker",
            "REPLACED",
        ]);
    }

    #[test]
    fn test_is_end_of_file_insertion_at_end() {
        let original: Vec<String> = vec!["first", "last"]
            .into_iter().map(String::from).collect();

        let mut chunk = make_chunk(&["last"], &[], &["appended"], &[], 0);
        chunk.is_end_of_file = true;

        let result = apply_patch_backtracking(&original, &[chunk]).unwrap();
        assert_eq!(result, vec!["first", "last", "appended"]);
    }
}
