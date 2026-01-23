//! Decoration rendering for line numbers, git changes, and grid separators.
//! Provides styled output similar to bat's decorations.

use syntastica::renderer::{Renderer, TerminalRenderer};
use syntastica::style::{Color, Style};
use syntastica::theme::ResolvedTheme;

use crate::git::LineChange;

/// Configuration for which decorations to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub struct DecorationConfig {
  /// Show line numbers
  pub show_numbers: bool,
  /// Show git change indicators
  pub show_changes: bool,
}

impl DecorationConfig {
  /// Returns true if any decorations are enabled.
  pub fn has_decorations(&self) -> bool {
    self.show_numbers || self.show_changes
  }
}

/// Get a dim style from the theme for line numbers and decorations.
/// Returns the first available theme style or creates a fallback.
fn get_dim_style_or_create(theme: &ResolvedTheme) -> Style {
  theme
    .find_style("comment")
    .or_else(|| theme.find_style("punctuation"))
    .or_else(|| theme.find_style("ui.text"))
    .unwrap_or_else(|| Style::new(Color::new(100, 100, 100), None, false, false, false, false))
}

/// Get git change style with appropriate colors.
fn get_git_change_style(line_change: LineChange) -> Style {
  match line_change {
    LineChange::Removed => Style::new(Color::new(255, 100, 100), None, false, false, false, false), // Red
    LineChange::Modified => Style::new(Color::new(255, 200, 100), None, false, false, false, false), // Yellow
    LineChange::Added => Style::new(Color::new(150, 255, 150), None, false, false, false, false), // Green
  }
}

/// Render a single line with all decorations.
///
/// Layout: {line_number}{space}{git_symbol}{space}{border}{content}
/// The space before git_symbol only appears when git decorations are enabled.
///
/// # Arguments
/// * `content` - The highlighted line content as (text, style_key) pairs
/// * `line_no` - The line number (1-based)
/// * `config` - Decoration configuration
/// * `line_change` - Optional git change for this line
/// * `renderer` - The terminal renderer
/// * `theme` - The color theme
/// * `line_number_width` - Width of line number column
pub fn render_decorated_line(
  content: &[(String, Option<String>)],
  line_no: usize,
  config: &DecorationConfig,
  line_change: Option<LineChange>,
  renderer: &mut TerminalRenderer,
  theme: &ResolvedTheme,
  line_number_width: usize,
) -> String {
  let mut output = String::new();
  let dim_style = get_dim_style_or_create(theme);

  // Line numbers (right-aligned) - use dim style
  if config.show_numbers {
    let prefix = format!("{line_no:>width$}", width = line_number_width);
    let escaped = renderer.escape(&prefix);
    output.push_str(&renderer.styled(&escaped, dim_style));
  }

  // Git symbol (1 character) - comes after line number with a space
  if config.show_changes {
    // Add space before git symbol
    let space = " ";
    let escaped = renderer.escape(space);
    output.push_str(&renderer.styled(&escaped, dim_style));

    let (symbol, style) = match line_change {
      Some(LineChange::Added) => ('+', get_git_change_style(LineChange::Added)),
      Some(LineChange::Modified) => ('~', get_git_change_style(LineChange::Modified)),
      Some(LineChange::Removed) => ('-', get_git_change_style(LineChange::Removed)),
      None => (' ', dim_style),
    };

    let symbol_str = format!("{symbol}");
    let escaped = renderer.escape(&symbol_str);
    output.push_str(&renderer.styled(&escaped, style));
  }

  // Single space separator - use dim style
  if config.show_numbers || config.show_changes {
    let space = " ";
    let escaped = renderer.escape(space);
    output.push_str(&renderer.styled(&escaped, dim_style));
  }

  // Grid separator - shown when there are any decorations
  if config.has_decorations() {
    let grid = "│ ";
    let escaped = renderer.escape(grid);
    output.push_str(&renderer.styled(&escaped, dim_style));
  }

  // Content
  for (text, style_key) in content {
    let escaped = renderer.escape(text);
    match style_key.as_ref().and_then(|key| theme.find_style(key)) {
      Some(style) => output.push_str(&renderer.styled(&escaped, style)),
      None => output.push_str(&renderer.unstyled(&escaped)),
    }
  }

  output
}
