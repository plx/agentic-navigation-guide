//! Directory dumping functionality for generating navigation guides

use crate::errors::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};
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
        let tree = self.build_tree(entries);

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

        let mut entries = Vec::new();

        for entry in walker {
            let entry = entry?;

            // Check exclusion patterns
            if let Some(globs) = &self.exclude_globs {
                let path = entry.path();
                let relative_path = path.strip_prefix(&self.root_path).unwrap_or(path);
                if globs.is_match(relative_path) {
                    continue;
                }
            }

            entries.push(entry);
        }

        Ok(entries)
    }

    /// Build a tree structure from flat entries
    fn build_tree(&self, entries: Vec<DirEntry>) -> TreeNode {
        let mut root = TreeNode {
            name: String::new(),
            is_dir: true,
            children: Vec::new(),
        };

        for entry in entries {
            let path = entry.path();
            let relative_path = path
                .strip_prefix(&self.root_path)
                .unwrap_or(path)
                .to_path_buf();

            self.insert_into_tree(&mut root, &relative_path, entry.file_type().is_dir());
        }

        root
    }

    /// Insert a path into the tree structure
    fn insert_into_tree(&self, node: &mut TreeNode, path: &Path, is_dir: bool) {
        let components: Vec<_> = path.components().collect();

        if components.is_empty() {
            return;
        }

        if components.len() == 1 {
            // Leaf node
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            node.children.push(TreeNode {
                name,
                is_dir,
                children: Vec::new(),
            });
        } else {
            // Find or create intermediate directory
            let first = components[0].as_os_str().to_string_lossy().to_string();
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
                    is_dir: true,
                    children: Vec::new(),
                });
                node.children.last_mut().unwrap()
            };

            self.insert_into_tree(child, &rest, is_dir);
        }
    }

    /// Format the tree as markdown
    fn format_tree(&self, node: &TreeNode, output: &mut String, depth: usize) {
        for child in &node.children {
            let indent = " ".repeat(depth * self.indent_size);
            let name = if child.is_dir {
                format!("{}/", child.name)
            } else {
                child.name.clone()
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
    is_dir: bool,
    children: Vec<TreeNode>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
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
}
