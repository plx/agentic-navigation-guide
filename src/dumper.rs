//! Directory dumping functionality for generating navigation guides

use crate::entry_type::{
    classify_path, EntryClassification, SupportedEntryKind, UnsupportedEntryKind,
};
use crate::errors::{AppError, Result};
use crate::path_codec::{
    contains_forbidden_control, has_windows_drive_prefix, render_os_component,
    render_utf8_component, serialize_component,
};
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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
    fn collect_entries(&self) -> Result<Vec<CollectedEntry>> {
        let mut classify = classify_path;
        self.collect_entries_with(&mut classify)
    }

    fn collect_entries_with<F>(&self, classify: &mut F) -> Result<Vec<CollectedEntry>>
    where
        F: FnMut(&Path) -> io::Result<EntryClassification>,
    {
        // Preserve the staged #43 behavior for a file selected as the root:
        // it currently produces an empty generated body rather than a walk
        // error. Root validation remains with that focused issue.
        if !fs::metadata(&self.root_path)?.is_dir() {
            return Ok(Vec::new());
        }

        let maximum_entry_depth = self.max_depth.map(|depth| depth + 1);
        let mut entries = Vec::new();
        self.collect_directory(
            &self.root_path,
            Path::new(""),
            0,
            maximum_entry_depth,
            classify,
            &mut entries,
        )?;
        Ok(entries)
    }

    fn collect_directory<F>(
        &self,
        directory: &Path,
        relative_directory: &Path,
        directory_depth: usize,
        maximum_entry_depth: Option<usize>,
        classify: &mut F,
        entries: &mut Vec<CollectedEntry>,
    ) -> Result<()>
    where
        F: FnMut(&Path) -> io::Result<EntryClassification>,
    {
        let mut children = fs::read_dir(directory)?.collect::<io::Result<Vec<_>>>()?;
        children.sort_by_key(fs::DirEntry::file_name);

        for entry in children {
            let relative_path = relative_directory.join(entry.file_name());
            if self.is_excluded(&relative_path) {
                continue;
            }

            let depth = directory_depth + 1;
            Self::validate_included_name(&entry.file_name(), depth == 1)?;

            let kind = match classify(&entry.path()) {
                Ok(Ok(kind)) => kind,
                Ok(Err(kind)) => {
                    return Err(Self::unsupported_entry_error(&relative_path, kind));
                }
                Err(error) => {
                    return Err(Self::classification_error(&relative_path, &error));
                }
            };
            entries.push(CollectedEntry {
                relative_path: relative_path.clone(),
                kind,
            });

            if kind == SupportedEntryKind::Directory
                && maximum_entry_depth.map_or(true, |maximum| depth < maximum)
            {
                self.collect_directory(
                    &entry.path(),
                    &relative_path,
                    depth,
                    maximum_entry_depth,
                    classify,
                    entries,
                )?;
            }
        }

        Ok(())
    }

    fn is_excluded(&self, relative_path: &Path) -> bool {
        let Some(globs) = &self.exclude_globs else {
            return false;
        };

        if globs.is_match(relative_path) {
            return true;
        }

        let mut current_path = PathBuf::new();
        for component in relative_path.components() {
            current_path.push(component);
            if globs.is_match(&current_path) {
                return true;
            }
        }

        false
    }

    fn unsupported_entry_error(relative_path: &Path, kind: UnsupportedEntryKind) -> AppError {
        AppError::Other(format!(
            "unsupported included filesystem entry {}: {kind}",
            Self::render_relative_path(relative_path)
        ))
    }

    fn classification_error(relative_path: &Path, error: &io::Error) -> AppError {
        AppError::Other(format!(
            "could not classify included filesystem entry {} without following it ({:?})",
            Self::render_relative_path(relative_path),
            error.kind()
        ))
    }

    fn render_relative_path(relative_path: &Path) -> String {
        relative_path
            .components()
            .map(|component| render_os_component(component.as_os_str()))
            .collect::<Vec<_>>()
            .join("/")
    }

    /// Build a tree structure from flat entries
    fn build_tree(&self, entries: Vec<CollectedEntry>) -> Result<TreeNode> {
        let mut root = TreeNode {
            name: String::new(),
            serialized_name: String::new(),
            is_dir: true,
            children: Vec::new(),
        };

        for entry in entries {
            self.ensure_utf8_relative_path(&entry.relative_path)?;
            self.insert_into_tree(
                &mut root,
                &entry.relative_path,
                entry.kind == SupportedEntryKind::Directory,
            )?;
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

struct CollectedEntry {
    relative_path: PathBuf,
    kind: SupportedEntryKind,
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
        let diagnostic = dumper
            .dump()
            .expect_err("non-UTF-8 names must be rejected")
            .to_string();

        assert!(
            diagnostic.contains("\"\\x62\\x61\\x64\\x2D\\xFF\\x2D\\x6E\\x61\\x6D\\x65\""),
            "diagnostic did not preserve every raw byte: {diagnostic}"
        );
        assert!(
            !diagnostic.contains('\u{fffd}'),
            "diagnostic used a lossy replacement character: {diagnostic}"
        );
    }

    #[test]
    fn issue_42_injected_unknown_entry_type_fails_closed() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("unknown"), "").unwrap();
        let dumper = Dumper::new(temp_dir.path());
        let mut classifier = |_path: &Path| Ok(Err(UnsupportedEntryKind::Unknown));

        let error = match dumper.collect_entries_with(&mut classifier) {
            Ok(_) => panic!("an unknown included entry type must abort collection"),
            Err(error) => error,
        };
        let diagnostic = error.to_string();

        assert!(diagnostic.contains("\"unknown\""), "{diagnostic}");
        assert!(
            diagnostic.contains("unknown filesystem entry type"),
            "{diagnostic}"
        );
        assert!(
            !diagnostic.contains(&temp_dir.path().display().to_string()),
            "diagnostic disclosed the physical root: {diagnostic}"
        );
    }

    #[test]
    fn issue_42_transient_classification_failure_aborts_collection() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("vanished.txt"), "").unwrap();
        let dumper = Dumper::new(temp_dir.path());
        let mut classifier = |_path: &Path| {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "ISSUE42_CLASSIFIER_INTERNAL_SENTINEL",
            ))
        };

        let error = match dumper.collect_entries_with(&mut classifier) {
            Ok(_) => panic!("a transient classification failure must abort collection"),
            Err(error) => error,
        };
        let diagnostic = error.to_string();

        assert!(diagnostic.contains("\"vanished.txt\""), "{diagnostic}");
        assert!(diagnostic.contains("NotFound"), "{diagnostic}");
        assert!(
            !diagnostic.contains("ISSUE42_CLASSIFIER_INTERNAL_SENTINEL"),
            "diagnostic disclosed an untrusted classifier detail: {diagnostic}"
        );
        assert!(
            !diagnostic.contains(&temp_dir.path().display().to_string()),
            "diagnostic disclosed the physical root: {diagnostic}"
        );
    }
}
