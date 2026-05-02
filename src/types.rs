//! Core data types for the agentic navigation guide

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents a filesystem item type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilesystemItem {
    /// A regular file
    File {
        /// The file path relative to its parent
        path: String,
        /// Optional comment describing the file
        comment: Option<String>,
    },
    /// A directory
    Directory {
        /// The directory path relative to its parent
        path: String,
        /// Optional comment describing the directory
        comment: Option<String>,
        /// Child items within this directory
        children: Vec<NavigationGuideLine>,
    },
    /// A symbolic link
    Symlink {
        /// The symlink path relative to its parent
        path: String,
        /// Optional comment describing the symlink
        comment: Option<String>,
        /// The target of the symlink (if specified in the guide)
        target: Option<String>,
    },
    /// A placeholder representing one or more unlisted items
    Placeholder {
        /// Optional comment describing what the placeholder represents
        comment: Option<String>,
    },
}

/// Represents a single line in the navigation guide
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationGuideLine {
    /// The line number in the original document
    pub line_number: usize,
    /// The indentation level (number of spaces / indent_size)
    pub indent_level: usize,
    /// The filesystem item this line represents
    pub item: FilesystemItem,
}

impl NavigationGuideLine {
    /// Get the path of the filesystem item
    pub fn path(&self) -> &str {
        match &self.item {
            FilesystemItem::File { path, .. }
            | FilesystemItem::Directory { path, .. }
            | FilesystemItem::Symlink { path, .. } => path,
            FilesystemItem::Placeholder { .. } => "...",
        }
    }

    /// Get the comment of the filesystem item
    pub fn comment(&self) -> Option<&str> {
        match &self.item {
            FilesystemItem::File { comment, .. }
            | FilesystemItem::Directory { comment, .. }
            | FilesystemItem::Symlink { comment, .. }
            | FilesystemItem::Placeholder { comment, .. } => comment.as_deref(),
        }
    }

    /// Check if this item is a directory
    pub fn is_directory(&self) -> bool {
        matches!(self.item, FilesystemItem::Directory { .. })
    }

    /// Check if this item is a placeholder
    pub fn is_placeholder(&self) -> bool {
        matches!(self.item, FilesystemItem::Placeholder { .. })
    }

    /// Get the children of a directory item
    pub fn children(&self) -> Option<&[NavigationGuideLine]> {
        match &self.item {
            FilesystemItem::Directory { children, .. } => Some(children),
            _ => None,
        }
    }
}

/// Represents a complete navigation guide
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationGuide {
    /// The root items in the guide
    pub items: Vec<NavigationGuideLine>,
    /// The original prologue content (before the guide block)
    pub prologue: Option<String>,
    /// The original epilogue content (after the guide block)
    pub epilogue: Option<String>,
    /// Whether this guide should be ignored during verification
    pub ignore: bool,
}

impl NavigationGuide {
    /// Create a new empty navigation guide
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            prologue: None,
            epilogue: None,
            ignore: false,
        }
    }
}

impl Default for NavigationGuide {
    fn default() -> Self {
        Self::new()
    }
}

/// Execution mode for the CLI tool
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ExecutionMode {
    /// Default execution mode
    #[default]
    Default,
    /// Running as a post-tool-use hook
    PostToolUse,
    /// Running as a pre-commit hook
    PreCommitHook,
    /// Running as a GitHub Actions check
    GitHubActions,
}

/// Log level for output verbosity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LogLevel {
    /// Minimal output
    Quiet,
    /// Normal output
    #[default]
    Default,
    /// Verbose output
    Verbose,
}

/// Configuration for the CLI tool
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// The execution mode
    pub execution_mode: ExecutionMode,
    /// The log level
    pub log_level: LogLevel,
    /// The root directory for operations
    pub root_path: Option<PathBuf>,
    /// The path to the navigation guide file
    pub guide_path: Option<PathBuf>,
    /// The original guide path as provided by the user (for error messages)
    pub original_guide_path: Option<String>,
    /// The original root path as provided by the user (for error messages)
    pub original_root_path: Option<String>,
}
