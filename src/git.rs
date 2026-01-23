//! Git status detection for line changes.
//! Provides per-line git modification indicators similar to bat.

use eyre::{Result, eyre};
use std::path::Path;
use std::process::Command;

/// Represents the type of change for a single line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineChange {
  /// Line was added (green +)
  Added,
  /// Line was modified (yellow ~)
  Modified,
  /// Line was removed (red -)
  #[allow(dead_code)]
  Removed,
}

/// Get git line changes for a file path.
///
/// Returns a vector where the index corresponds to the line number (1-based).
/// Lines with no changes will have `None` in the vector.
pub fn get_git_line_changes(path: &Path) -> Result<Vec<Option<LineChange>>> {
  get_git_line_changes_impl(path)
}

fn get_git_line_changes_impl(path: &Path) -> Result<Vec<Option<LineChange>>> {
  // Use git diff --unified=0 to get proper line-by-line changes
  let output = Command::new("git")
    .arg("diff")
    .arg("--unified=0")
    .arg("--no-color")
    .arg("--")
    .arg(path)
    .output()
    .map_err(|e| eyre!("Failed to run git diff: {}", e))?;

  let diff_output = String::from_utf8_lossy(&output.stdout);

  // Parse the unified diff format
  // Format: " @{old_start},{old_count} +{new_start},{new_count} @@"
  // Then lines prefixed with " " (unchanged), "+" (added), "-" (removed)
  parse_unified_diff(&diff_output)
}

/// Parse a unified diff output to extract per-line change information.
fn parse_unified_diff(diff: &str) -> Result<Vec<Option<LineChange>>> {
  use std::collections::HashMap;

  let mut changes: HashMap<usize, LineChange> = HashMap::new();
  let mut lines = diff.lines().peekable();
  let mut current_new_line: usize = 1;

  while let Some(line) = lines.next() {
    if line.is_empty() {
      continue;
    }

    // Check for diff header line: "@@ -o,s +n,t @@"
    if line.starts_with("@@") {
      if let Some(header) = parse_diff_header(line) {
        current_new_line = header.new_start;
      }
      continue;
    }

    // Skip file headers and meta lines
    if line.starts_with("---") || line.starts_with("+++") {
      continue;
    }
    if line.starts_with("\\") {
      continue; // "\ No newline at end of file"
    }

    match line.chars().next() {
      Some(' ') => {
        // Unchanged line - advance line number
        current_new_line += 1;
      }
      Some('-') => {
        // Removed line - check if next line is an addition at same position (modification)
        if let Some(next_line) = lines.peek() {
          if next_line.starts_with('+') {
            // This is a modification: - followed by +
            changes.insert(current_new_line, LineChange::Modified);
            lines.next(); // consume the + line
          } else {
            // Pure removal - don't increment current_new_line since line doesn't exist in new file
          }
        } else {
          // Removal at end of diff
        }
        // Note: for pure removals, we don't insert into changes since those lines don't exist in new file
      }
      Some('+') => {
        // Added line
        changes.entry(current_new_line).or_insert(LineChange::Added);
        current_new_line += 1;
      }
      _ => {
        current_new_line += 1;
      }
    }
  }

  // Convert HashMap to Vec, using 0-based indexing
  if changes.is_empty() {
    return Ok(Vec::new());
  }

  let max_line = *changes.keys().max().unwrap_or(&1);
  let mut result = vec![None; max_line];

  for (line_num, change) in changes {
    if line_num > 0 {
      result[line_num - 1] = Some(change);
    }
  }

  Ok(result)
}

/// Parse a diff header line like "@@ -3,5 +3,6 @@"
struct DiffHeader {
  _old_start: usize,
  new_start: usize,
}

fn parse_diff_header(line: &str) -> Option<DiffHeader> {
  // Format: "@@ -o,s +n,t @@"
  let parts: Vec<&str> = line.split_whitespace().collect();
  if parts.len() < 4 {
    return None;
  }

  // Parse "-o,s" part
  let old_part = parts[1].strip_prefix('-')?;
  let old_parts: Vec<&str> = old_part.split(',').collect();
  if old_parts.len() < 2 {
    return None;
  }
  let old_start: usize = old_parts[0].parse().ok()?;

  // Parse "+n,t" part
  let new_part = parts[2].strip_prefix('+')?;
  let new_parts: Vec<&str> = new_part.split(',').collect();
  if new_parts.len() < 2 {
    return None;
  }
  let new_start: usize = new_parts[0].parse().ok()?;

  Some(DiffHeader {
    _old_start: old_start,
    new_start,
  })
}
