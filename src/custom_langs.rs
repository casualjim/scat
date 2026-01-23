//! Custom language support for languages not in syntastica-parsers-git.
//! Currently adds support for Terraform and HCL.

use once_cell::sync::OnceCell;
use std::borrow::Cow;
use syntastica::{
  language_set::{FileType, HighlightConfiguration, LanguageSet, SupportedLanguage},
  theme::THEME_KEYS,
};
use tree_sitter_language::LanguageFn;

/// Custom languages that we provide ourselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomLang {
  Hcl,
  Terraform,
}

impl AsRef<str> for CustomLang {
  fn as_ref(&self) -> &str {
    match self {
      Self::Hcl => "hcl",
      Self::Terraform => "terraform",
    }
  }
}

impl<'set, T> SupportedLanguage<'set, T> for CustomLang {
  fn name(&self) -> Cow<'_, str> {
    self.as_ref().into()
  }

  fn for_name(name: impl AsRef<str>, _set: &'set T) -> syntastica::Result<Self> {
    match name.as_ref() {
      "hcl" => Ok(CustomLang::Hcl),
      "terraform" | "tf" => Ok(CustomLang::Terraform),
      name => Err(syntastica::Error::UnsupportedLanguage(name.to_string())),
    }
  }

  fn for_file_type(file_type: FileType, _set: &'set T) -> Option<Self> {
    match file_type {
      FileType::Hcl => Some(CustomLang::Hcl),
      FileType::Terraform => Some(CustomLang::Terraform),
      _ => None,
    }
  }
}

/// Custom language set with HCL and Terraform support.
#[derive(Default)]
pub struct CustomLanguageSet {
  hcl_lang: OnceCell<HighlightConfiguration>,
  terraform_lang: OnceCell<HighlightConfiguration>,
}

impl CustomLanguageSet {
  pub fn new() -> Self {
    Self::default()
  }
}

impl LanguageSet<'_> for CustomLanguageSet {
  type Language = CustomLang;

  fn get_language(&self, language: Self::Language) -> syntastica::Result<&HighlightConfiguration> {
    match language {
      CustomLang::Hcl => init_lang(
        language.as_ref(),
        &self.hcl_lang,
        tree_sitter_hcl::LANGUAGE,
        HCL_HIGHLIGHT_QUERY,
      ),
      CustomLang::Terraform => init_lang(
        language.as_ref(),
        &self.terraform_lang,
        tree_sitter_hcl::LANGUAGE,
        TERRAFORM_HIGHLIGHT_QUERY,
      ),
    }
  }
}

/// Helper function for initializing a language configuration.
fn init_lang<'a>(
  name: &str,
  cell: &'a OnceCell<HighlightConfiguration>,
  get_lang: LanguageFn,
  queries: &str,
) -> syntastica::Result<&'a HighlightConfiguration> {
  cell.get_or_try_init(|| {
    let mut conf = HighlightConfiguration::new(
      get_lang.into(),
      name,
      // Preprocess queries for syntastica compatibility
      &syntastica_query_preprocessor::process_highlights("", true, queries),
      "",
      "",
    )?;
    // Configure with syntastica's theme keys
    conf.configure(THEME_KEYS);
    Ok(conf)
  })
}

// Highlight queries from nvim-treesitter:
// https://github.com/nvim-treesitter/nvim-treesitter/tree/master/queries/hcl

const HCL_HIGHLIGHT_QUERY: &str = r#"; highlights.scm
[
  "!"
  "\*"
  "/"
  "%"
  "\+"
  "-"
  ">"
  ">="
  "<"
  "<="
  "=="
  "!="
  "&&"
  "||"
] @operator

[
  "{"
  "}"
  "["
  "]"
  "("
  ")"
] @punctuation.bracket

[
  "."
  ".*"
  ","
  "[*]"
] @punctuation.delimiter

[
  (ellipsis)
  "\?"
  "=>"
] @punctuation.special

[
  ":"
  "="
] @none

[
  "for"
  "endfor"
  "in"
] @keyword.repeat

[
  "if"
  "else"
  "endif"
] @keyword.conditional

[
  (quoted_template_start) ; "
  (quoted_template_end) ; "
  (template_literal) ; non-interpolation/directive content
] @string

[
  (heredoc_identifier) ; END
  (heredoc_start) ; << or <<-
] @punctuation.delimiter

[
  (template_interpolation_start) ; ${
  (template_interpolation_end) ; }
  (template_directive_start) ; %{
  (template_directive_end) ; }
  (strip_marker) ; ~
] @punctuation.special

(numeric_lit) @number

(bool_lit) @boolean

(null_lit) @constant

(comment) @comment @spell

(identifier) @variable

(body
  (block
    (identifier) @keyword))

(body
  (block
    (body
      (block
        (identifier) @type))))

(function_call
  (identifier) @function)

(attribute
  (identifier) @variable.member)

; { key: val }
;
; highlight identifier keys as though they were block attributes
(object_elem
  key: (expression
    (variable_expr
      (identifier) @variable.member)))

; var.foo, data.bar
;
; first element in get_attr is a variable.builtin or a reference to a variable.builtin
(expression
  (variable_expr
    (identifier) @variable.builtin)
  (get_attr
    (identifier) @variable.member))
"#;

// Highlight queries from nvim-treesitter:
// https://github.com/nvim-treesitter/nvim-treesitter/tree/master/queries/terraform

const TERRAFORM_HIGHLIGHT_QUERY: &str = r#"; highlights.scm
[
  "!"
  "\*"
  "/"
  "%"
  "\+"
  "-"
  ">"
  ">="
  "<"
  "<="
  "=="
  "!="
  "&&"
  "||"
] @operator

[
  "{"
  "}"
  "["
  "]"
  "("
  ")"
] @punctuation.bracket

[
  "."
  ".*"
  ","
  "[*]"
] @punctuation.delimiter

[
  (ellipsis)
  "\?"
  "=>"
] @punctuation.special

[
  ":"
  "="
] @none

[
  "for"
  "endfor"
  "in"
] @keyword.repeat

[
  "if"
  "else"
  "endif"
] @keyword.conditional

[
  (quoted_template_start) ; "
  (quoted_template_end) ; "
  (template_literal) ; non-interpolation/directive content
] @string

[
  (heredoc_identifier) ; END
  (heredoc_start) ; << or <<-
] @punctuation.delimiter

[
  (template_interpolation_start) ; ${
  (template_interpolation_end) ; }
  (template_directive_start) ; %{
  (template_directive_end) ; }
  (strip_marker) ; ~
] @punctuation.special

(numeric_lit) @number

(bool_lit) @boolean

(null_lit) @constant

(comment) @comment @spell

(identifier) @variable

(body
  (block
    (identifier) @keyword))

(body
  (block
    (body
      (block
        (identifier) @type))))

(function_call
  (identifier) @function)

(attribute
  (identifier) @variable.member)

; { key: val }
;
; highlight identifier keys as though they were block attributes
(object_elem
  key: (expression
    (variable_expr
      (identifier) @variable.member)))

; var.foo, data.bar
;
; first element in get_attr is a variable.builtin or a reference to a variable.builtin
(expression
  (variable_expr
    (identifier) @variable.builtin)
  (get_attr
    (identifier) @variable.member))

; Terraform specific references
;
;
; local/module/data/var/output
(expression
  (variable_expr
    (identifier) @variable.builtin
    (#any-of? @variable.builtin "data" "var" "local" "module" "output"))
  (get_attr
    (identifier) @variable.member))

; path.root/cwd/module
(expression
  (variable_expr
    (identifier) @type.builtin
    (#eq? @type.builtin "path"))
  (get_attr
    (identifier) @variable.builtin
    (#any-of? @variable.builtin "root" "cwd" "module")))

; terraform.workspace
(expression
  (variable_expr
    (identifier) @type.builtin
    (#eq? @type.builtin "terraform"))
  (get_attr
    (identifier) @variable.builtin
    (#any-of? @variable.builtin "workspace")))

; Terraform specific keywords
; FIXME: ideally only for identifiers under a `variable` block to minimize false positives
((identifier) @type.builtin
  (#any-of? @type.builtin "bool" "string" "number" "object" "tuple" "list" "map" "set" "any"))

(object_elem
  val: (expression
    (variable_expr
      (identifier) @type.builtin
      (#any-of? @type.builtin "bool" "string" "number" "object" "tuple" "list" "map" "set" "any"))))
"#;
