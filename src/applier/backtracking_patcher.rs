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
use std::cell::Cell;
use std::collections::HashSet;

thread_local! {
    /// Counts how many recursive backtrack calls have been made in this run.
    static NODE_COUNT: Cell<usize> = Cell::new(0);
}

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

    let (fixed_path, mut state) = find_fixed_mappings(original_lines, chunks, mode);
    let mut current_path = fixed_path;

    NODE_COUNT.with(|cnt| cnt.set(0));
    backtrack_with_mode(&original_lines.to_vec(), chunks, &mut state, &mut current_path, mode);

    if state.solution_count == 0 {
        return Err(ZenpatchError::PatchConflict(
            "No valid patch application sequence found - please fix the patch include more context".to_string(),
        ));
    }
    if state.solution_count > 1 {
        return Err(ZenpatchError::AmbiguousPatch(
            "Patch application is ambiguous - please include more context before or after insertions or deletions".to_string()
        ));
    }

    let solution = state.solution_path.clone().expect("solution_path must be set");
    let mut ordered = solution.clone();
    ordered.sort_by_key(|&(_, pos)| pos);
    let mut result = original_lines.to_vec();
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
    Ok(result)
}

/// Finds fixed mappings based on uniquely identifying context lines in both patch and file.
fn find_fixed_mappings(
    original_lines: &[String],
    chunks: &[Chunk],
    mode: WhitespaceMode,
) -> (Vec<(usize, usize)>, BacktrackingState) {
    let mut result_path = Vec::new();
    let mut state = BacktrackingState::new();
    let mut used_indices = HashSet::new();

    for (chunk_idx, chunk) in chunks.iter().enumerate() {
        let positions = find_match_positions(&original_lines.to_vec(), chunk, mode);
        let mut valid_positions = vec![];

        for &pos in &positions {
            // Check deletion match
            let mut pre_len = 0;
            for (lt, _) in chunk.lines.iter() {
                if *lt == LineType::Context {
                    pre_len += 1;
                } else {
                    break;
                }
            }

            let mut adj_pre = pre_len;
            if pre_len > 0 && !chunk.del_lines.is_empty() {
                if let (LineType::Context, ctx) = &chunk.lines[pre_len - 1] {
                    if let Some((LineType::Deletion, del)) = chunk.lines.get(pre_len) {
                        if match_line(ctx, del, mode) {
                            adj_pre = adj_pre.saturating_sub(1);
                        }
                    }
                }
            }

            let mut content_match = true;
            for (j, del_line) in chunk.del_lines.iter().enumerate() {
                let idx = pos + adj_pre + j;
                if idx >= original_lines.len() || !match_line(&original_lines[idx], del_line, mode) {
                    content_match = false;
                    break;
                }
            }

            if content_match {
                valid_positions.push(pos);
            }
        }

        // Only allow fixed mapping if there is exactly one valid position and it does not overlap
        if valid_positions.len() == 1 {
            let pos = valid_positions[0];
            let affected = get_affected_indices(chunk, pos, mode);
            if affected.iter().all(|idx| !used_indices.contains(idx)) {
                state.applied_chunks.insert(chunk_idx);
                for idx in &affected {
                    state.modified_indices.insert(*idx);
                    used_indices.insert(*idx);
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

fn find_match_positions(
    lines: &Vec<String>,
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
        return positions;
    }

    let clen = pre.len();
    if lines.len() < clen {
        return positions;
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
        for &(ref lt, ref content) in chunk.lines.iter().rev() {
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

    positions
}

fn get_affected_indices(chunk: &Chunk, pos: usize, mode: WhitespaceMode) -> Vec<usize> {
    let mut indices: Vec<usize> = Vec::new();
    let mut pre_len = 0;
    for (lt, _) in chunk.lines.iter() {
        if *lt == LineType::Context {
            pre_len += 1;
        } else {
            break;
        }
    }

    let mut adj_pre = pre_len;
    if pre_len > 0 && !chunk.del_lines.is_empty() {
        if let (LineType::Context, ctx) = &chunk.lines[pre_len - 1] {
            if let Some((LineType::Deletion, del)) = chunk.lines.get(pre_len) {
                if match_line(ctx, del, mode) {
                    adj_pre = adj_pre.saturating_sub(1);
                }
            }
        }
    }

    for idx in pos + adj_pre..pos + adj_pre + chunk.del_lines.len() {
        indices.push(idx);
    }
    indices
}

fn apply_chunk(lines: &Vec<String>, chunk: &Chunk, pos: usize, mode: WhitespaceMode) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    let mut pre_len = 0;
    for (lt, _) in chunk.lines.iter() {
        if *lt == LineType::Context {
            pre_len += 1;
        } else {
            break;
        }
    }

    let mut adj_pre = pre_len;
    if pre_len > 0 && !chunk.del_lines.is_empty() {
        if let (LineType::Context, ctx) = &chunk.lines[pre_len - 1] {
            if let Some((LineType::Deletion, del)) = chunk.lines.get(pre_len) {
                if match_line(ctx, del, mode) {
                    adj_pre = adj_pre.saturating_sub(1);
                }
            }
        }
    }

    let start_copy = (pos + adj_pre).min(lines.len());
    result.extend_from_slice(&lines[..start_copy]);
    result.extend(chunk.ins_lines.iter().cloned());

    let end_del = (pos + adj_pre + chunk.del_lines.len()).min(lines.len());
    result.extend_from_slice(&lines[end_del..]);
    result
}

fn backtrack_with_mode(
    lines: &Vec<String>,
    chunks: &[Chunk],
    state: &mut BacktrackingState,
    current_path: &mut Vec<(usize, usize)>,
    mode: WhitespaceMode,
) {
    let over = NODE_COUNT.with(|c| {
        let n = c.get().saturating_add(1);
        c.set(n);
        n > MAX_BACKTRACK_NODES
    });
    if over || state.solution_count > 1 {
        state.solution_count = 2;
        return;
    }

    if current_path.len() == chunks.len() {
        let mut candidate = lines.clone();
        let mut delta: isize = 0;
        let mut mapping = current_path.clone();
        mapping.sort_by_key(|&(_, pos)| pos);
        for (chunk_idx, orig_pos) in mapping.iter() {
            let chunk = &chunks[*chunk_idx];
            let pos = if delta >= 0 {
                (*orig_pos as isize + delta) as usize
            } else {
                orig_pos.saturating_sub((-delta) as usize)
            };
            candidate = apply_chunk(&candidate, chunk, pos, mode);
            delta += chunk.ins_lines.len() as isize - chunk.del_lines.len() as isize;
        }

        if state.solution_count == 0 {
            state.solution_count = 1;
            state.first_solution_result = Some(candidate.clone());
            state.solution_path = Some(current_path.clone());
            return;
        }

        if let Some(first) = &state.first_solution_result {
            if *first == candidate {
                return;
            }
        }

        state.solution_count = 2;
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

        let positions = find_match_positions(lines, chunk, mode);
        for pos in positions {
            let mut pre_len = 0;
            for (lt, _) in chunk.lines.iter() {
                if *lt == LineType::Context {
                    pre_len += 1;
                } else {
                    break;
                }
            }
            let mut adj_pre = pre_len;
            if pre_len > 0 && !chunk.del_lines.is_empty() {
                if let (LineType::Context, ctx) = &chunk.lines[pre_len - 1] {
                    if let Some((LineType::Deletion, del)) = chunk.lines.get(pre_len) {
                        if match_line(ctx, del, mode) {
                            adj_pre = adj_pre.saturating_sub(1);
                        }
                    }
                }
            }

            let mut content_match = true;
            for (j, del_line) in chunk.del_lines.iter().enumerate() {
                let idx = pos + adj_pre + j;
                if idx >= lines.len() || !match_line(&lines[idx], del_line, mode) {
                    content_match = false;
                    break;
                }
            }
            if !content_match {
                continue;
            }

            let affected = get_affected_indices(chunk, pos, mode);
            if affected.iter().any(|idx| state.modified_indices.contains(idx)) {
                continue;
            }

            let mut next_state = state.clone();
            next_state.applied_chunks.insert(i);
            for idx in affected.iter().cloned() {
                next_state.modified_indices.insert(idx);
            }

            let mut next_path = current_path.clone();
            next_path.push((i, pos));
            backtrack_with_mode(lines, chunks, &mut next_state, &mut next_path, mode);

            state.solution_count = next_state.solution_count;
            if state.solution_count == 1 {
                state.first_solution_result = next_state.first_solution_result.clone();
                state.solution_path = next_state.solution_path.clone();
            }
            if state.solution_count > 1 {
                return;
            }
        }
    }
}
