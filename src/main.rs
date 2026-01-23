mod custom_langs;
mod decorations;
mod git;
mod unprintable;

use std::fmt::Write as _;
use std::fs;
use std::io::{self, Cursor, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

use clap::{CommandFactory, Parser, ValueEnum};
use dark_light::Mode as DarkLightMode;
use decorations::DecorationConfig;
use eyre::{Result, eyre};
use palate::detectors;
use syntastica::Processor;
use syntastica::language_set::{EitherLang, SupportedLanguage, Union};
use syntastica::renderer::{Renderer, TerminalRenderer};
use syntastica::theme::ResolvedTheme;
use syntastica_parsers_git::{LANGUAGE_NAMES, Lang, LanguageSetImpl};

use custom_langs::{CustomLang, CustomLanguageSet};

const MAX_CONTENT_SIZE_BYTES: usize = 51200;

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum ColorWhen {
  Auto,
  Never,
  Always,
}

#[derive(Parser, Debug)]
#[command(
  name = "scat",
  version,
  about = "cat with syntax highlighting",
  long_about = "A modern replacement for cat with syntax highlighting powered by tree-sitter.\n\
                Automatically detects file types and applies appropriate syntax highlighting.\n\
                Supports 100+ programming languages and multiple color themes.",
  after_help = "EXAMPLES:\n    \
    scat main.rs                    Display a file with syntax highlighting\n    \
    scat -n config.toml             Show file with line numbers\n    \
    scat main.rs#L10-L20            Show only selected lines\n    \
    scat --language rust file.txt   Force Rust syntax highlighting\n    \
    scat --theme dracula main.js    Use Dracula color theme\n    \
    cat file.rs | scat              Read from stdin\n    \
    scat *.py                       Display multiple files\n\n\
    For available themes, see: https://docs.rs/syntastica-themes/latest/syntastica_themes/\n\n\
    To generate shell completions:\n    \
    scat --completions bash > ~/.local/share/bash-completion/completions/scat"
)]
struct Cli {
  #[arg(
    long = "completions",
    alias = "completion",
    value_enum,
    help = "Generate shell completions for the specified shell",
    long_help = "Generate shell completion script for the specified shell.\n\
                 Output the completion script to stdout, which you can then save to the\n\
                 appropriate location for your shell.\n\n\
                 Examples:\n  \
                 scat --completions bash > ~/.local/share/bash-completion/completions/scat\n  \
                 scat --completions zsh > ~/.zsh/completion/_scat\n  \
                 scat --completions fish > ~/.config/fish/completions/scat.fish"
  )]
  completions: Option<clap_complete::Shell>,

  #[arg(
    long,
    short = 'l',
    value_name = "LANG",
    help = "Force a specific programming language",
    long_help = "Override automatic language detection and force a specific language.\n\
                 Useful when the file extension doesn't match the content or for files\n\
                 without extensions.\n\n\
                 Examples:\n  \
                 scat --language rust config.txt\n  \
                 scat --language json response.log\n\n\
                 For a complete list of supported languages, see:\n\
                 https://docs.rs/syntastica-parsers-git/latest/syntastica_parsers_git/"
  )]
  language: Option<String>,

  #[arg(
    long,
    value_name = "THEME",
    default_value = "auto",
    help = "Color theme to use for syntax highlighting",
    long_help = "Specify a color theme for syntax highlighting.\n\n\
                 Use 'auto' (default) to automatically detect light/dark mode:\n  \
                 - Light mode: catppuccin-latte\n  \
                 - Dark mode: catppuccin-mocha\n\n\
                 Popular themes include:\n  \
                 dracula, nord, one-dark, one-light, gruvbox-dark, gruvbox-light,\n  \
                 solarized-dark, solarized-light, tokyo-night, catppuccin-mocha,\n  \
                 catppuccin-latte, catppuccin-frappe, catppuccin-macchiato\n\n\
                 For a complete list of available themes, see:\n\
                 https://docs.rs/syntastica-themes/latest/syntastica_themes/"
  )]
  theme: String,

  #[arg(
    long,
    short = 'n',
    value_name = "RANGE",
    help = "Show only selected lines (e.g. 10-20, 10:20, 10,20, 10)",
    long_help = "Show only selected lines from the file.\n\
                 Accepted formats: start-end, start:end, start,end, or a single line number.\n\
                 Examples:\n  \
                 scat --lines 10-20 main.rs\n  \
                 scat --lines 10:20 main.rs\n  \
                 scat --lines 10,20 main.rs\n  \
                 scat --lines 10 main.rs"
  )]
  lines: Option<String>,

  #[arg(
    long,
    value_enum,
    default_value = "auto",
    help = "Specify when to use colored output"
  )]
  color: ColorWhen,

  #[arg(long, help = "Disable colored output")]
  no_color: bool,

  #[arg(long, help = "List supported themes")]
  list_themes: bool,

  #[arg(
    long,
    short = 's',
    help = "Squeeze consecutive empty lines into a single empty line"
  )]
  squeeze_blank: bool,

  #[arg(
    long,
    value_name = "squeeze-limit",
    help = "Set the maximum number of consecutive empty lines"
  )]
  squeeze_limit: Option<usize>,

  #[arg(
    long,
    value_name = "components",
    help = "Configure which style components to display (numbers, changes)"
  )]
  style: Option<String>,

  #[arg(long, short = 'u', help = "No-op, output is always unbuffered")]
  unbuffered: bool,

  #[arg(
    long,
    short = 'A',
    help = "Show unprintable characters (tabs as →, carriage returns as ↵, etc.)"
  )]
  show_all: bool,

  #[arg(
    long,
    help = "Generate man page",
    long_help = "Generate a manual page in roff format and print to stdout.\n\
                 You can save this to a file and install it in your man path.\n\n\
                 Example:\n  \
                 scat --man-page > scat.1\n  \
                 sudo cp scat.1 /usr/local/share/man/man1/"
  )]
  man_page: bool,

  #[arg(
    value_name = "FILE",
    help = "Files to display (use '-' or omit for stdin)",
    long_help = "One or more files to display with syntax highlighting.\n\
                 If no files are specified, or if '-' is given, reads from stdin.\n\n\
                 Examples:\n  \
                 scat main.rs lib.rs\n  \
                 scat main.rs#L10-L20\n  \
                 cat file.rs | scat\n  \
                 echo 'code' | scat --language rust"
  )]
  files: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug)]
struct LineRange {
  start: usize,
  end: usize,
}

#[derive(Clone, Debug)]
struct FileSpec {
  path: PathBuf,
  line_range: Option<LineRange>,
}

fn main() -> Result<()> {
  let cli = Cli::parse();
  if let Some(shell) = cli.completions {
    write_completions(shell)?;
    return Ok(());
  }
  if cli.man_page {
    write_man_page()?;
    return Ok(());
  }
  if cli.list_themes {
    for theme in syntastica_themes::THEMES {
      println!("{theme}");
    }
    return Ok(());
  }
  let mut use_color = io::stdout().is_terminal();
  // Check --no-color flag and NO_COLOR environment variable (https://no-color.org/)
  if cli.no_color || std::env::var("NO_COLOR").is_ok() {
    use_color = false;
  }
  match cli.color {
    ColorWhen::Auto => {}
    ColorWhen::Never => use_color = false,
    ColorWhen::Always => use_color = true,
  }
  // Use Union to combine custom languages (HCL/Terraform) with syntastica-parsers-git
  let custom_set = CustomLanguageSet::new();
  let parser_set = LanguageSetImpl::new();
  let language_set = Union::new(custom_set, parser_set);
  let theme = resolve_theme(&cli.theme);
  let decoration_config = resolve_decoration_config(&cli);
  let squeeze_limit = cli.squeeze_limit.unwrap_or(1);
  let squeeze_blank = cli.squeeze_blank || cli.squeeze_limit.is_some();
  let language_override = match cli.language.as_deref() {
    Some(name) => Some(
      resolve_language_union(name, &language_set)
        .ok_or_else(|| eyre!("Unsupported language: {name}"))?,
    ),
    None => None,
  };

  let files = if cli.files.is_empty() {
    vec![PathBuf::from("-")]
  } else {
    cli.files
  };

  let global_line_range = match cli.lines.as_deref() {
    Some(raw) => Some(parse_line_range_arg(raw)?),
    None => None,
  };

  let mut had_error = false;
  let mut file_specs = Vec::with_capacity(files.len());
  for path in files {
    match parse_file_spec(path, global_line_range) {
      Ok(spec) => file_specs.push(spec),
      Err(err) => {
        eprintln!("scat: {err}");
        had_error = true;
      }
    }
  }

  let mut processor = Processor::new(&language_set);
  let mut renderer = TerminalRenderer::new(None);
  let mut stdout = io::stdout().lock();
  let mut stdin = io::stdin();
  let mut stdin_consumed = false;
  let mut wrote_output = false;
  let multiple_files = file_specs.len() > 1;

  for spec in file_specs {
    // Show file header between files when headers are enabled
    if decoration_config.show_headers && multiple_files {
      if wrote_output {
        writeln!(stdout)?;
      }
      let display_name = display_name_for_spec(&spec);
      // Get terminal width, default to 80 if unavailable
      let term_width = crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80);
      // Create a prominent header that spans the terminal width
      let border = "─".repeat(term_width);
      writeln!(stdout, "{border}")?;
      // Center the filename in the header
      let padding = (term_width.saturating_sub(display_name.len())) / 2;
      writeln!(stdout, "{}{}{}", " ".repeat(padding), display_name, " ".repeat(term_width - display_name.len() - padding))?;
      writeln!(stdout, "{border}")?;
    }

    if spec.path == Path::new("-") {
      if stdin_consumed {
        continue;
      }
      stdin_consumed = true;
      let mut buf = Vec::new();
      if let Err(err) = stdin.read_to_end(&mut buf) {
        eprintln!("scat: -: {err}");
        had_error = true;
        continue;
      }
      emit_bytes(
        &mut stdout,
        buf,
        None,
        spec.line_range,
        language_override.as_ref().map(clone_either_lang),
        decoration_config,
        use_color,
        squeeze_blank,
        squeeze_limit,
        cli.show_all,
        &language_set,
        &mut processor,
        &mut renderer,
        &theme,
      )?;
      wrote_output = true;
      continue;
    }

    match fs::read(&spec.path) {
      Ok(buf) => {
        emit_bytes(
          &mut stdout,
          buf,
          Some(&spec.path),
          spec.line_range,
          language_override.as_ref().map(clone_either_lang),
          decoration_config,
          use_color,
          squeeze_blank,
          squeeze_limit,
          cli.show_all,
          &language_set,
          &mut processor,
          &mut renderer,
          &theme,
        )?;
        wrote_output = true;
      }
      Err(err) => {
        eprintln!("scat: {}: {err}", spec.path.display());
        had_error = true;
      }
    }
  }

  stdout.flush()?;
  if had_error {
    std::process::exit(1);
  }
  Ok(())
}

fn write_completions(shell: clap_complete::Shell) -> Result<()> {
  let mut cmd = Cli::command();
  clap_complete::generate(shell, &mut cmd, "scat", &mut io::stdout());
  Ok(())
}

fn write_man_page() -> Result<()> {
  let cmd = Cli::command();
  let man = clap_mangen::Man::new(cmd);
  man.render(&mut io::stdout())?;
  Ok(())
}

fn clone_either_lang(lang: &EitherLang<CustomLang, Lang>) -> EitherLang<CustomLang, Lang> {
  match lang {
    EitherLang::Left(custom) => EitherLang::Left(*custom),
    EitherLang::Right(parser) => EitherLang::Right(*parser),
  }
}

#[allow(clippy::too_many_arguments)]
fn emit_bytes(
  stdout: &mut impl Write,
  bytes: Vec<u8>,
  path: Option<&Path>,
  line_range: Option<LineRange>,
  language_override: Option<EitherLang<CustomLang, Lang>>,
  decoration_config: DecorationConfig,
  use_color: bool,
  squeeze_blank: bool,
  squeeze_limit: usize,
  show_all: bool,
  language_set: &Union<CustomLanguageSet, LanguageSetImpl>,
  processor: &mut Processor<Union<CustomLanguageSet, LanguageSetImpl>>,
  renderer: &mut TerminalRenderer,
  theme: &ResolvedTheme,
) -> Result<bool> {
  let bytes = if let Some(range) = line_range {
    slice_bytes_by_line_range(&bytes, range)
  } else {
    bytes
  };
  let bytes = if squeeze_blank {
    squeeze_blank_lines_bytes(&bytes, squeeze_limit)
  } else {
    bytes
  };
  let line_number_start = line_range.map(|range| range.start).unwrap_or(1);
  let ended_with_newline = bytes.last() == Some(&b'\n') || bytes.is_empty();

  // Handle show_all flag for non-color, non-decoration case
  if !use_color && !decoration_config.has_decorations() {
    if show_all {
      if let Ok(text) = String::from_utf8(bytes.clone()) {
        let transformed = unprintable::show_unprintable(&text, unprintable::get_char_style());
        stdout.write_all(transformed.as_bytes())?;
      } else {
        // Invalid UTF-8, write as-is
        stdout.write_all(&bytes)?;
      }
    } else {
      stdout.write_all(&bytes)?;
    }
    return Ok(ended_with_newline);
  }

  // Fetch git changes if needed (only for actual file paths, not stdin)
  let git_changes = if decoration_config.show_changes {
    // Only check git for real file paths (not stdin "-")
    if let Some(p) = path {
      if p != Path::new("-") {
        // Convert to absolute path for git detection
        let abs_path = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        git::get_git_line_changes(&abs_path).unwrap_or_default()
      } else {
        Vec::new()
      }
    } else {
      Vec::new()
    }
  } else {
    Vec::new()
  };

  if use_color {
    match String::from_utf8(bytes) {
      Ok(text) => {
        let language = language_override.or_else(|| detect_language(path, &text, language_set));
        let rendered = render_text(
          &text,
          language,
          decoration_config,
          line_number_start,
          &git_changes,
          processor,
          renderer,
          theme,
          show_all,
        );
        stdout.write_all(rendered.as_bytes())?;
        return Ok(ended_with_newline);
      }
      Err(err) => {
        let bytes = err.into_bytes();
        if decoration_config.show_numbers {
          write_numbered_bytes(stdout, &bytes, line_number_start)?;
        } else if show_all {
          // Try to convert what we can, handling invalid UTF-8
          let text = String::from_utf8_lossy(&bytes);
          let transformed = unprintable::show_unprintable(&text, unprintable::get_char_style());
          stdout.write_all(transformed.as_bytes())?;
        } else {
          stdout.write_all(&bytes)?;
        }
        return Ok(ended_with_newline);
      }
    }
  }

  if decoration_config.show_numbers {
    if show_all {
      // Use number_plain_text when show_all is enabled
      if let Ok(text) = String::from_utf8(bytes.clone()) {
        let numbered = number_plain_text(&text, line_number_start, show_all);
        stdout.write_all(numbered.as_bytes())?;
      } else {
        write_numbered_bytes(stdout, &bytes, line_number_start)?;
      }
    } else {
      write_numbered_bytes(stdout, &bytes, line_number_start)?;
    }
  } else if show_all {
    // Handle show_all for non-color case with decorations
    if let Ok(text) = String::from_utf8(bytes.clone()) {
      let transformed = unprintable::show_unprintable(&text, unprintable::get_char_style());
      stdout.write_all(transformed.as_bytes())?;
    } else {
      stdout.write_all(&bytes)?;
    }
  } else {
    stdout.write_all(&bytes)?;
  }
  Ok(ended_with_newline)
}

fn detect_language(
  path: Option<&Path>,
  content: &str,
  language_set: &Union<CustomLanguageSet, LanguageSetImpl>,
) -> Option<EitherLang<CustomLang, Lang>> {
  let name = detect_language_name(path, content)?;
  resolve_language_union(name.to_ascii_lowercase(), language_set)
}

fn resolve_language_union(
  name: impl AsRef<str>,
  language_set: &Union<CustomLanguageSet, LanguageSetImpl>,
) -> Option<EitherLang<CustomLang, Lang>> {
  let name = name.as_ref().trim();
  let normalized = name.to_ascii_lowercase();

  // First check if it's a custom language (HCL or Terraform)
  if let Ok(custom_lang) =
    <CustomLang as SupportedLanguage<'_, _>>::for_name(&normalized, language_set)
  {
    return Some(EitherLang::Left(custom_lang));
  }

  // Then try the syntastica parsers with aliases
  let name = match normalized.as_str() {
    "xml" | "xhtml" | "svg" | "plist" => "html",
    _ => normalized.as_str(),
  };

  // Try as a normal language
  if let Ok(lang) = <Lang as SupportedLanguage<'_, _>>::for_name(name, language_set) {
    return Some(EitherLang::Right(lang));
  }

  // Try as an injection language
  if let Some(lang) = <Lang as SupportedLanguage<'_, _>>::for_injection(name, language_set) {
    return Some(EitherLang::Right(lang));
  }

  // Try with canonical names
  if let Some(canonical) = LANGUAGE_NAMES
    .iter()
    .copied()
    .find(|candidate| candidate.eq_ignore_ascii_case(name))
    && let Ok(lang) = <Lang as SupportedLanguage<'_, _>>::for_name(canonical, language_set)
  {
    return Some(EitherLang::Right(lang));
  }

  None
}

fn detect_language_name(path: Option<&Path>, content: &str) -> Option<&'static str> {
  let mut extension: Option<String> = None;
  let mut candidates = Vec::new();

  if let Some(path) = path
    && let Some(filename) = path.file_name().and_then(|name| name.to_str())
  {
    if let Some(candidate) = detectors::get_language_from_filename(filename) {
      return Some(candidate);
    }

    extension = detectors::get_extension(filename).map(str::to_string);
    candidates = extension
      .as_deref()
      .map(detectors::get_languages_from_extension)
      .unwrap_or_default();
    if candidates.len() == 1 {
      return Some(candidates[0]);
    }
  }

  let shebang_candidates =
    detectors::get_languages_from_shebang(Cursor::new(content)).unwrap_or_default();
  candidates = filter_candidates(candidates, shebang_candidates);
  if candidates.len() == 1 {
    return Some(candidates[0]);
  }

  let content = truncate_to_char_boundary(content, MAX_CONTENT_SIZE_BYTES);
  candidates = if candidates.len() > 1 {
    if let Some(extension) = extension.as_deref() {
      let heuristic_candidates =
        detectors::get_languages_from_heuristics(extension, &candidates, content);
      filter_candidates(candidates, heuristic_candidates)
    } else {
      candidates
    }
  } else {
    candidates
  };

  match candidates.len() {
    0 => None,
    1 => Some(candidates[0]),
    _ => Some(detectors::classify(content, &candidates)),
  }
}

fn filter_candidates(
  previous_candidates: Vec<&'static str>,
  new_candidates: Vec<&'static str>,
) -> Vec<&'static str> {
  if previous_candidates.is_empty() {
    return new_candidates;
  }

  if new_candidates.is_empty() {
    return previous_candidates;
  }

  let filtered_candidates: Vec<&'static str> = previous_candidates
    .iter()
    .filter(|candidate| new_candidates.contains(candidate))
    .copied()
    .collect();

  if filtered_candidates.is_empty() {
    previous_candidates
  } else {
    filtered_candidates
  }
}

fn truncate_to_char_boundary(s: &str, mut max: usize) -> &str {
  if max >= s.len() {
    return s;
  }

  while !s.is_char_boundary(max) {
    max -= 1;
  }

  &s[..max]
}

#[allow(clippy::too_many_arguments)]
fn render_text(
  text: &str,
  language: Option<EitherLang<CustomLang, Lang>>,
  decoration_config: DecorationConfig,
  line_number_start: usize,
  git_changes: &[Option<git::LineChange>],
  processor: &mut Processor<Union<CustomLanguageSet, LanguageSetImpl>>,
  renderer: &mut TerminalRenderer,
  theme: &ResolvedTheme,
  show_all: bool,
) -> String {
  let Some(language) = language else {
    return if decoration_config.show_numbers {
      number_plain_text(text, line_number_start, show_all)
    } else if show_all {
      unprintable::show_unprintable(text, unprintable::get_char_style())
    } else {
      text.to_string()
    };
  };

  match processor.process(text, language) {
    Ok(highlights) => {
      if decoration_config.has_decorations() {
        render_highlights_with_decorations(
          &highlights,
          decoration_config,
          line_number_start,
          git_changes,
          renderer,
          theme,
          show_all,
        )
      } else if show_all {
        // Render with show_all transformation applied before terminal escapes
        render_highlights_show_all(&highlights, renderer, theme)
      } else {
        syntastica::render(&highlights, renderer, theme.clone())
      }
    }
    Err(_) => {
      if decoration_config.show_numbers {
        number_plain_text(text, line_number_start, show_all)
      } else if show_all {
        unprintable::show_unprintable(text, unprintable::get_char_style())
      } else {
        text.to_string()
      }
    }
  }
}

/// Render highlights with show_all transformation applied before terminal escape sequences are added.
/// This ensures we only transform unprintable characters in the source text, not ANSI escape codes.
fn render_highlights_show_all(
  highlights: &syntastica::Highlights<'_>,
  renderer: &mut TerminalRenderer,
  theme: &ResolvedTheme,
) -> String {
  let char_style = unprintable::get_char_style();
  let mut result = String::new();
  let last_line = highlights.len().saturating_sub(1);

  // Determine the line feed marker to use
  let lf_marker = if matches!(char_style, unprintable::CharStyle::Unicode) {
    "␊"
  } else {
    "$"
  };

  for (index, line) in highlights.iter().enumerate() {
    let line_has_content = line.iter().any(|(text, _)| !text.is_empty());
    for (text, style) in line.iter() {
      // Transform unprintable characters in the text
      let transformed = unprintable::show_unprintable(text, char_style);
      // Apply styling if present
      if let Some(style_name) = style {
        // Look up the Style from the theme using the style name
        if let Some(style_obj) = theme.get(*style_name) {
          result.push_str(&renderer.styled(transformed.as_str(), *style_obj));
        } else {
          result.push_str(&transformed);
        }
      } else {
        result.push_str(&transformed);
      }
    }
    // Add line feed marker at the end of each line that has content
    if line_has_content {
      result.push_str(lf_marker);
    }
    // Add newline between lines, but not after the last line
    if index != last_line {
      result.push_str(&renderer.newline());
    }
  }

  result
}

fn resolve_theme(theme: &str) -> ResolvedTheme {
  let theme_name = theme.trim();
  let theme_key = theme_name.split(':').next().unwrap_or("auto");

  match theme_key {
    "" | "auto" => resolve_auto_theme(),
    "dark" => syntastica_themes::catppuccin::mocha(),
    "light" => syntastica_themes::catppuccin::latte(),
    _ => {
      if let Some(theme) = syntastica_themes::from_str(theme_key) {
        return theme;
      }
      resolve_auto_theme()
    }
  }
}

fn resolve_auto_theme() -> ResolvedTheme {
  match dark_light::detect() {
    Ok(DarkLightMode::Light) => syntastica_themes::catppuccin::latte(),
    Ok(DarkLightMode::Dark) => syntastica_themes::catppuccin::mocha(),
    Ok(DarkLightMode::Unspecified) => syntastica_themes::catppuccin::mocha(),
    Err(_) => syntastica_themes::catppuccin::mocha(),
  }
}

fn render_highlights_with_decorations(
  highlights: &syntastica::Highlights<'_>,
  decoration_config: DecorationConfig,
  line_number_start: usize,
  git_changes: &[Option<git::LineChange>],
  renderer: &mut TerminalRenderer,
  theme: &ResolvedTheme,
  show_all: bool,
) -> String {
  if highlights.is_empty() {
    return String::new();
  }

  // Only show git margin if there are actual changes
  let has_git_changes = git_changes.iter().any(|c| c.is_some());
  let effective_config = if has_git_changes {
    decoration_config
  } else {
    DecorationConfig {
      show_changes: false,
      ..decoration_config
    }
  };

  let last_line_no = line_number_start.saturating_add(highlights.len().saturating_sub(1));
  let width = line_number_width(last_line_no);
  let last_line = highlights.len().saturating_sub(1);
  let mut out = renderer.head().into_owned();

  for (index, line) in highlights.iter().enumerate() {
    let line_no = line_number_start + index;
    let line_change = git_changes.get(index).copied().flatten();

    // Convert highlights to the format expected by the decorations module
    // Apply unprintable character transformation if show_all is enabled
    let line_content: Vec<(String, Option<String>)> = line
      .iter()
      .map(|(text, style)| {
        let transformed_text = if show_all {
          unprintable::show_unprintable(text, unprintable::get_char_style())
        } else {
          text.to_string()
        };
        (transformed_text, style.map(|s| s.to_string()))
      })
      .collect();

    let rendered = decorations::render_decorated_line(
      &line_content,
      line_no,
      &effective_config,
      line_change,
      renderer,
      theme,
      width,
    );

    out.push_str(&rendered);

    // Add line feed marker at the end of each line when show_all is enabled
    // Only add it to lines that have actual content
    if show_all {
      let line_has_content = line.iter().any(|(text, _)| !text.is_empty());
      if line_has_content {
        let lf_marker = if matches!(unprintable::get_char_style(), unprintable::CharStyle::Unicode) {
          "␊"
        } else {
          "$"
        };
        out.push_str(lf_marker);
      }
    }

    if index != last_line {
      out += &renderer.newline();
    }
  }

  out + &renderer.tail()
}

fn number_plain_text(text: &str, line_number_start: usize, show_all: bool) -> String {
  let line_count = count_lines_bytes(text.as_bytes());
  if line_count == 0 {
    return String::new();
  }

  let last_line_no = line_number_start.saturating_add(line_count.saturating_sub(1));
  let width = line_number_width(last_line_no);
  let mut out = String::new();
  let mut line_no = line_number_start;

  for chunk in text.split_inclusive('\n') {
    let _ = write!(out, "{:>width$}  ", line_no, width = width);
    let content = if show_all {
      unprintable::show_unprintable(chunk, unprintable::get_char_style())
    } else {
      chunk.to_string()
    };
    out.push_str(&content);
    line_no += 1;
  }
  out
}

fn write_numbered_bytes(
  stdout: &mut impl Write,
  bytes: &[u8],
  line_number_start: usize,
) -> Result<()> {
  let line_count = count_lines_bytes(bytes);
  if line_count == 0 {
    return Ok(());
  }

  let last_line_no = line_number_start.saturating_add(line_count.saturating_sub(1));
  let width = line_number_width(last_line_no);
  let mut line_no = line_number_start;
  write_prefix(stdout, line_no, width)?;
  for (index, byte) in bytes.iter().enumerate() {
    stdout.write_all(&[*byte])?;
    if *byte == b'\n' && index + 1 < bytes.len() {
      line_no += 1;
      write_prefix(stdout, line_no, width)?;
    }
  }
  Ok(())
}

fn write_prefix(stdout: &mut impl Write, line_no: usize, width: usize) -> Result<()> {
  write!(stdout, "{:>width$}  ", line_no, width = width)?;
  Ok(())
}

fn count_lines_bytes(bytes: &[u8]) -> usize {
  if bytes.is_empty() {
    return 0;
  }
  let count = bytes.iter().filter(|byte| **byte == b'\n').count();
  if bytes.last() == Some(&b'\n') {
    count
  } else {
    count + 1
  }
}

fn line_number_width(line_count: usize) -> usize {
  let width = line_count.to_string().len();
  if width == 0 { 1 } else { width }
}

fn resolve_decoration_config(cli: &Cli) -> DecorationConfig {
  let mut config = DecorationConfig::default();

  // Parse style components from --style flag
  if let Some(style) = cli.style.as_deref() {
    config = parse_style_components(config, style);
  }

  config
}

/// Parse style components from the --style flag.
/// Supports: "numbers", "changes", "headers"
fn parse_style_components(mut config: DecorationConfig, style: &str) -> DecorationConfig {
  for raw in style.split(',') {
    let token = raw.trim();
    match token {
      "numbers" => config.show_numbers = true,
      "changes" => config.show_changes = true,
      "headers" => config.show_headers = true,
      _ => {}
    }
  }
  config
}

fn display_name_for_spec(spec: &FileSpec) -> String {
  if spec.path == Path::new("-") {
    "-".to_string()
  } else {
    spec.path.to_string_lossy().to_string()
  }
}

fn squeeze_blank_lines_bytes(bytes: &[u8], limit: usize) -> Vec<u8> {
  if bytes.is_empty() {
    return Vec::new();
  }
  let mut out = Vec::with_capacity(bytes.len());
  let mut blank_count = 0usize;
  let mut start = 0usize;
  for (index, byte) in bytes.iter().enumerate() {
    if *byte == b'\n' {
      let line = &bytes[start..=index];
      let mut content_end = index;
      if content_end > start && bytes[content_end - 1] == b'\r' {
        content_end -= 1;
      }
      let is_blank = content_end == start;
      if is_blank {
        blank_count += 1;
        if blank_count <= limit {
          out.extend_from_slice(line);
        }
      } else {
        blank_count = 0;
        out.extend_from_slice(line);
      }
      start = index + 1;
    }
  }
  if start < bytes.len() {
    let line = &bytes[start..];
    let mut content_end = bytes.len();
    if content_end > start && bytes[content_end - 1] == b'\r' {
      content_end -= 1;
    }
    let is_blank = content_end == start;
    if is_blank {
      blank_count += 1;
      if blank_count <= limit {
        out.extend_from_slice(line);
      }
    } else {
      out.extend_from_slice(line);
    }
  }
  out
}

fn parse_file_spec(path: PathBuf, default_range: Option<LineRange>) -> Result<FileSpec> {
  let raw = path.to_string_lossy();
  if let Some((path_part, line_range)) = parse_line_range_suffix(&raw)? {
    let parsed_path = PathBuf::from(path_part);
    return Ok(FileSpec {
      path: parsed_path,
      line_range: Some(line_range),
    });
  }
  Ok(FileSpec {
    path,
    line_range: default_range,
  })
}

fn parse_line_range_suffix(raw: &str) -> Result<Option<(String, LineRange)>> {
  let (path_part, range_part) = match raw.rsplit_once("#L").or_else(|| raw.rsplit_once("#l")) {
    Some(parts) => parts,
    None => return Ok(None),
  };
  if path_part.is_empty() {
    return Err(eyre!("missing file path before line range"));
  }
  if range_part.is_empty() {
    return Err(eyre!("missing line range after #L"));
  }
  let line_range = parse_line_range(range_part).ok_or_else(|| {
    eyre!(
      "invalid line range '#L{range_part}' (expected #L<start>-<end>, #L<start>:<end>, #L<start>,<end>, or #L<start>)"
    )
  })?;
  Ok(Some((path_part.to_string(), line_range)))
}

fn parse_line_range_arg(raw: &str) -> Result<LineRange> {
  parse_line_range(raw).ok_or_else(|| {
    eyre!("invalid line range '{raw}' (expected start-end, start:end, start,end, or start)")
  })
}

fn parse_line_range(raw: &str) -> Option<LineRange> {
  let raw = raw.trim();
  let raw = raw
    .strip_prefix('L')
    .or_else(|| raw.strip_prefix('l'))
    .unwrap_or(raw);
  if raw.is_empty() {
    return None;
  }
  let (start_raw, end_raw) = match split_line_range(raw) {
    Some(parts) => parts,
    None => {
      let line = raw.parse::<usize>().ok()?;
      if line == 0 {
        return None;
      }
      return Some(LineRange {
        start: line,
        end: line,
      });
    }
  };
  if start_raw.is_empty() || end_raw.is_empty() {
    return None;
  }
  let start_raw = start_raw.trim();
  let end_raw = end_raw.trim();
  let start = start_raw.parse::<usize>().ok()?;
  let end_raw = end_raw
    .strip_prefix('L')
    .or_else(|| end_raw.strip_prefix('l'))
    .unwrap_or(end_raw);
  let end = end_raw.parse::<usize>().ok()?;
  if start == 0 || end == 0 || end < start {
    return None;
  }
  Some(LineRange { start, end })
}

fn split_line_range(raw: &str) -> Option<(&str, &str)> {
  for separator in ['-', ':', ','] {
    if let Some(parts) = raw.split_once(separator) {
      return Some(parts);
    }
  }
  None
}

fn slice_bytes_by_line_range(bytes: &[u8], range: LineRange) -> Vec<u8> {
  if bytes.is_empty() {
    return Vec::new();
  }
  let mut out = Vec::new();
  let mut line_no = 1usize;
  let mut start = 0usize;
  for (index, byte) in bytes.iter().enumerate() {
    if *byte == b'\n' {
      let line_end = index + 1;
      if line_no >= range.start && line_no <= range.end {
        out.extend_from_slice(&bytes[start..line_end]);
      }
      line_no += 1;
      if line_no > range.end {
        return out;
      }
      start = line_end;
    }
  }
  if start < bytes.len() && line_no >= range.start && line_no <= range.end {
    out.extend_from_slice(&bytes[start..]);
  }
  out
}
