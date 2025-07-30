//! Error types for the agentic navigation guide

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias for the library
pub type Result<T> = std::result::Result<T, AppError>;

/// Top-level application error type
#[derive(Debug, Error)]
pub enum AppError {
    /// Syntax error in the navigation guide
    #[error("syntax error: {0}")]
    Syntax(#[from] SyntaxError),

    /// Semantic error when verifying against filesystem
    #[error("semantic error: {0}")]
    Semantic(#[from] SemanticError),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Pattern parsing error
    #[error("invalid glob pattern: {0}")]
    GlobPattern(#[from] globset::Error),

    /// WalkDir error
    #[error("filesystem walk error: {0}")]
    WalkDir(#[from] walkdir::Error),

    /// Other errors
    #[error("{0}")]
    Other(String),
}

/// Syntax errors in navigation guide format
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SyntaxError {
    /// Missing opening sentinel marker
    #[error("line {line}: missing opening <agentic-navigation-guide> marker")]
    MissingOpeningMarker { line: usize },

    /// Missing closing sentinel marker
    #[error("line {line}: missing closing </agentic-navigation-guide> marker")]
    MissingClosingMarker { line: usize },

    /// Multiple guide blocks found
    #[error("line {line}: multiple <agentic-navigation-guide> blocks found")]
    MultipleGuideBlocks { line: usize },

    /// Empty guide block
    #[error("empty navigation guide block")]
    EmptyGuideBlock,

    /// Invalid list format
    #[error("line {line}: invalid list format - must start with '-'")]
    InvalidListFormat { line: usize },

    /// Directory missing trailing slash
    #[error("line {line}: directory '{path}' must end with '/'")]
    DirectoryMissingSlash { line: usize, path: String },

    /// Invalid special directory
    #[error("line {line}: invalid special directory '{path}' (. and .. are not allowed)")]
    InvalidSpecialDirectory { line: usize, path: String },

    /// Inconsistent indentation
    #[error("line {line}: inconsistent indentation - expected {expected} spaces, found {found}")]
    InconsistentIndentation {
        line: usize,
        expected: usize,
        found: usize,
    },

    /// Invalid indentation level
    #[error("line {line}: invalid indentation level - must be a multiple of the indent size")]
    InvalidIndentationLevel { line: usize },

    /// Blank line in guide block
    #[error("line {line}: blank lines are not allowed within the guide block")]
    BlankLineInGuide { line: usize },

    /// Invalid path format
    #[error("line {line}: invalid path format '{path}'")]
    InvalidPathFormat { line: usize, path: String },

    /// Invalid comment format
    #[error("line {line}: invalid comment format - comments must be separated by '#'")]
    InvalidCommentFormat { line: usize },
}

/// Semantic errors when verifying against filesystem
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SemanticError {
    /// File or directory not found
    #[error("line {line}: {item_type} '{path}' not found at {full_path}")]
    ItemNotFound {
        line: usize,
        item_type: String,
        path: String,
        full_path: PathBuf,
    },

    /// Type mismatch (file vs directory)
    #[error("line {line}: expected {expected} but found {found} at '{path}'")]
    TypeMismatch {
        line: usize,
        expected: String,
        found: String,
        path: String,
    },

    /// Invalid nesting - item not actually child of parent
    #[error("line {line}: '{child}' is not a child of '{parent}'")]
    InvalidNesting {
        line: usize,
        child: String,
        parent: String,
    },

    /// Symlink target mismatch
    #[error("line {line}: symlink '{path}' points to '{actual}' but guide specifies '{expected}'")]
    SymlinkTargetMismatch {
        line: usize,
        path: String,
        expected: String,
        actual: String,
    },

    /// Permission denied accessing item
    #[error("line {line}: permission denied accessing '{path}'")]
    PermissionDenied { line: usize, path: String },
}

impl SyntaxError {
    /// Get the line number associated with this error, if any
    pub fn line_number(&self) -> Option<usize> {
        match self {
            Self::MissingOpeningMarker { line }
            | Self::MissingClosingMarker { line }
            | Self::MultipleGuideBlocks { line }
            | Self::InvalidListFormat { line }
            | Self::DirectoryMissingSlash { line, .. }
            | Self::InvalidSpecialDirectory { line, .. }
            | Self::InconsistentIndentation { line, .. }
            | Self::InvalidIndentationLevel { line }
            | Self::BlankLineInGuide { line }
            | Self::InvalidPathFormat { line, .. }
            | Self::InvalidCommentFormat { line } => Some(*line),
            Self::EmptyGuideBlock => None,
        }
    }
}

impl SemanticError {
    /// Get the line number associated with this error
    pub fn line_number(&self) -> usize {
        match self {
            Self::ItemNotFound { line, .. }
            | Self::TypeMismatch { line, .. }
            | Self::InvalidNesting { line, .. }
            | Self::SymlinkTargetMismatch { line, .. }
            | Self::PermissionDenied { line, .. } => *line,
        }
    }
}

/// Format errors for display with optional context
pub struct ErrorFormatter;

impl ErrorFormatter {
    /// Format an error with line context if available
    pub fn format_with_context(error: &AppError, file_content: Option<&str>) -> String {
        match error {
            AppError::Syntax(e) => {
                if let Some(line_num) = e.line_number() {
                    Self::format_with_line_context(e.to_string(), line_num, file_content)
                } else {
                    e.to_string()
                }
            }
            AppError::Semantic(e) => {
                Self::format_with_line_context(e.to_string(), e.line_number(), file_content)
            }
            _ => error.to_string(),
        }
    }

    fn format_with_line_context(
        error_msg: String,
        line_num: usize,
        file_content: Option<&str>,
    ) -> String {
        if let Some(content) = file_content {
            if let Some(line) = content.lines().nth(line_num.saturating_sub(1)) {
                format!("{}\n    {}", error_msg, line.trim())
            } else {
                error_msg
            }
        } else {
            error_msg
        }
    }
}
