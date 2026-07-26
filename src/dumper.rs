//! Directory dumping functionality for generating navigation guides

use crate::errors::{AppError, Result};
use crate::path_codec::{
    contains_forbidden_control, has_windows_drive_prefix, render_os_component,
    render_utf8_component, serialize_component,
};
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

/// Dumper for creating navigation guides from directory structures
pub struct Dumper {
    /// Root path to dump from
    root_path: PathBuf,
    /// Maximum depth to traverse
    max_depth: Option<usize>,
    /// Glob patterns to exclude
    exclude_globs: Option<GlobSet>,
    /// Number of spaces for indentation
    indent_size: usize,
}

impl Dumper {
    /// Create a new dumper for the given root path
    pub fn new(root_path: &Path) -> Self {
        Self {
            root_path: root_path.to_path_buf(),
            max_depth: None,
            exclude_globs: None,
            indent_size: 2,
        }
    }

    /// Set the maximum depth to traverse
    pub fn with_max_depth(mut self, max_depth: Option<usize>) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Set exclude patterns
    pub fn with_exclude_patterns(mut self, patterns: &[String]) -> Result<Self> {
        if patterns.is_empty() {
            self.exclude_globs = None;
        } else {
            let mut builder = GlobSetBuilder::new();
            for pattern in patterns {
                builder.add(Glob::new(pattern)?);
            }
            self.exclude_globs = Some(builder.build()?);
        }
        Ok(self)
    }

    /// Set the indent size
    pub fn with_indent_size(mut self, indent_size: usize) -> Self {
        self.indent_size = indent_size;
        self
    }

    /// Dump the directory structure as a navigation guide
    pub fn dump(&self) -> Result<String> {
        let mut output = String::new();

        // Get directory entries
        let entries = self.collect_entries()?;

        // Build the tree structure
        let mut tree = self.build_tree(entries)?;
        Self::prepare_tree(&mut tree, 0)?;

        // Format as markdown
        self.format_tree(&tree, &mut output, 0);

        Ok(output)
    }

    /// Dump with XML wrapper tags
    pub fn dump_with_wrapper(&self) -> Result<String> {
        let content = self.dump()?;
        Ok(format!(
            "<agentic-navigation-guide>\n{content}</agentic-navigation-guide>"
        ))
    }

    /// Collect all directory entries respecting depth and exclusion rules
    fn collect_entries(&self) -> Result<Vec<DirEntry>> {
        let mut walker = WalkDir::new(&self.root_path)
            .min_depth(1) // Skip the root itself
            .sort_by_file_name();

        if let Some(max_depth) = self.max_depth {
            walker = walker.max_depth(max_depth + 1); // +1 because we skip root
        }

        let exclude_globs = self.exclude_globs.clone();
        let root_path = self.root_path.clone();

        let walker = walker.into_iter().filter_entry(move |entry| {
            // Check exclusion patterns
            if let Some(ref globs) = exclude_globs {
                let path = entry.path();
                if let Ok(relative_path) = path.strip_prefix(&root_path) {
                    // Check the full relative path
                    if globs.is_match(relative_path) {
                        return false;
                    }

                    // For directories, check if any parent component matches
                    // This prevents descending into excluded directories
                    let mut current_path = PathBuf::new();
                    for component in relative_path.components() {
                        current_path.push(component);
                        if globs.is_match(&current_path) {
                            return false;
                        }
                    }
                }
            }
            true
        });

        let mut entries = Vec::new();

        for entry in walker {
            let entry = entry?;
            Self::validate_included_name(entry.file_name(), entry.depth() == 1)?;
            entries.push(entry);
        }

        Ok(entries)
    }

    /// Build a tree structure from flat entries
    fn build_tree(&self, entries: Vec<DirEntry>) -> Result<TreeNode> {
        let mut root = TreeNode {
            name: String::new(),
            serialized_name: String::new(),
            is_dir: true,
            children: Vec::new(),
        };

        for entry in entries {
            let path = entry.path();
            let relative_path = path
                .strip_prefix(&self.root_path)
                .unwrap_or(path)
                .to_path_buf();

            self.ensure_utf8_relative_path(&relative_path)?;
            self.insert_into_tree(&mut root, &relative_path, entry.file_type().is_dir())?;
        }

        Ok(root)
    }

    /// Insert a path into the tree structure
    fn insert_into_tree(&self, node: &mut TreeNode, path: &Path, is_dir: bool) -> Result<()> {
        let components: Vec<_> = path.components().collect();

        if components.is_empty() {
            return Ok(());
        }

        if components.len() == 1 {
            // Leaf node
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| AppError::NonUtf8Path {
                    path: self.root_path.join(path),
                })?
                .to_string();
            node.children.push(TreeNode {
                name,
                serialized_name: String::new(),
                is_dir,
                children: Vec::new(),
            });
        } else {
            // Find or create intermediate directory
            let first = components[0]
                .as_os_str()
                .to_str()
                .ok_or_else(|| AppError::NonUtf8Path {
                    path: self.root_path.join(path),
                })?
                .to_string();
            let rest = components[1..].iter().collect::<PathBuf>();

            let child = if let Some(existing) = node
                .children
                .iter_mut()
                .find(|c| c.name == first && c.is_dir)
            {
                existing
            } else {
                node.children.push(TreeNode {
                    name: first.clone(),
                    serialized_name: String::new(),
                    is_dir: true,
                    children: Vec::new(),
                });
                node.children.last_mut().unwrap()
            };

            self.insert_into_tree(child, &rest, is_dir)?;
        }

        Ok(())
    }

    fn ensure_utf8_relative_path(&self, relative_path: &Path) -> Result<()> {
        for (index, component) in relative_path.components().enumerate() {
            Self::validate_included_name(component.as_os_str(), index == 0)?;
        }

        Ok(())
    }

    fn validate_included_name(name: &OsStr, at_root: bool) -> Result<()> {
        let Some(name) = name.to_str() else {
            return Err(AppError::Other(format!(
                "unsupported non-UTF-8 filesystem name {}",
                render_os_component(name)
            )));
        };

        if contains_forbidden_control(name) {
            return Err(AppError::Other(format!(
                "unsupported control-bearing filesystem name {}",
                render_utf8_component(name)
            )));
        }

        if at_root && (name.starts_with('\\') || has_windows_drive_prefix(name)) {
            return Err(AppError::Other(format!(
                "unsupported rooted or drive-prefixed filesystem name {}",
                render_utf8_component(name)
            )));
        }

        Ok(())
    }

    fn prepare_tree(node: &mut TreeNode, depth: usize) -> Result<()> {
        node.children
            .sort_by(|left, right| left.name.as_bytes().cmp(right.name.as_bytes()));

        for child in &mut node.children {
            Self::validate_included_name(OsStr::new(&child.name), depth == 0)?;
            child.serialized_name = serialize_component(&child.name).ok_or_else(|| {
                AppError::Other(format!(
                    "unsupported filesystem name {}",
                    render_utf8_component(&child.name)
                ))
            })?;
            Self::prepare_tree(child, depth + 1)?;
        }

        Ok(())
    }

    /// Format the tree as markdown
    fn format_tree(&self, node: &TreeNode, output: &mut String, depth: usize) {
        for child in &node.children {
            let indent = " ".repeat(depth * self.indent_size);
            let name = if child.is_dir {
                format!("{}/", child.serialized_name)
            } else {
                child.serialized_name.clone()
            };

            output.push_str(&format!("{indent}- {name}\n"));

            if !child.children.is_empty() {
                self.format_tree(child, output, depth + 1);
            }
        }
    }
}

/// Internal tree node structure
struct TreeNode {
    name: String,
    serialized_name: String,
    is_dir: bool,
    children: Vec<TreeNode>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::ffi::OsStrExt;
    use tempfile::TempDir;

    #[test]
    fn test_dump_simple_directory() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create test structure
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "").unwrap();
        fs::write(root.join("Cargo.toml"), "").unwrap();

        let dumper = Dumper::new(root);
        let output = dumper.dump().unwrap();

        assert!(output.contains("- src/"));
        assert!(output.contains("  - main.rs"));
        assert!(output.contains("- Cargo.toml"));
    }

    #[test]
    fn test_dump_with_max_depth() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create nested structure
        fs::create_dir_all(root.join("a/b/c")).unwrap();
        fs::write(root.join("a/b/c/deep.txt"), "").unwrap();

        let dumper = Dumper::new(root).with_max_depth(Some(2));
        let output = dumper.dump().unwrap();

        assert!(output.contains("- a/"));
        assert!(output.contains("  - b/"));
        assert!(!output.contains("deep.txt"));
    }

    #[test]
    fn test_dump_supports_utf8_names() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        fs::create_dir(root.join("数据")).unwrap();
        fs::write(root.join("数据/résumé-🧭.md"), "").unwrap();

        let dumper = Dumper::new(root);
        let output = dumper.dump().unwrap();

        assert!(output.contains("- 数据/"));
        assert!(output.contains("  - résumé-🧭.md"));
    }

    #[cfg(unix)]
    #[test]
    fn test_dump_rejects_non_utf8_names() {
        use std::ffi::OsStr;

        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        let non_utf8_name = OsStr::from_bytes(b"bad-\xFF-name");
        if fs::write(root.join(non_utf8_name), "").is_err() {
            // Some Unix filesystems (notably on macOS) reject invalid UTF-8 names at creation time.
            return;
        }

        let dumper = Dumper::new(root);
        let result = dumper.dump();

        assert!(matches!(
            result,
            Err(crate::errors::AppError::NonUtf8Path { .. })
        ));
    }
}
