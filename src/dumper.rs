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

const MIN_INDENT_SIZE: usize = 1;
const MAX_INDENT_SIZE: usize = 16;
const MAX_LOGICAL_DEPTH: usize = 256;

#[derive(Clone, Copy)]
struct TraversalLimit {
    maximum_entry_depth: usize,
    reject_beyond_limit: bool,
}

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
        if entries.is_empty() {
            return Err(AppError::Other(format!(
                "generation root {} has no included, representable entries after depth and exclusion rules",
                self.render_root()
            )));
        }

        // Build the tree structure
        let mut tree = self.build_tree(entries)?;
        Self::prepare_tree(&mut tree, 0)?;

        // Format as markdown
        self.format_tree(&tree, &mut output, 0)?;
        debug_assert!(!output.is_empty());

        Ok(output)
    }

    /// Dump with XML wrapper tags
    pub fn dump_with_wrapper(&self) -> Result<String> {
        let content = self.dump()?;
        Ok(format!(
            "<agentic-navigation-guide>\n{content}</agentic-navigation-guide>"
        ))
    }

    fn validate_configuration(&self) -> Result<TraversalLimit> {
        if !(MIN_INDENT_SIZE..=MAX_INDENT_SIZE).contains(&self.indent_size) {
            return Err(AppError::Other(format!(
                "indent size {} is outside the supported range {} through {}",
                self.indent_size, MIN_INDENT_SIZE, MAX_INDENT_SIZE
            )));
        }

        let (maximum_logical_depth, reject_beyond_limit) = match self.max_depth {
            Some(depth) if depth <= MAX_LOGICAL_DEPTH => (depth, false),
            Some(depth) => {
                return Err(AppError::Other(format!(
                    "maximum depth {depth} is outside the supported range 0 through {MAX_LOGICAL_DEPTH}"
                )));
            }
            None => (MAX_LOGICAL_DEPTH, true),
        };
        let maximum_entry_depth = maximum_logical_depth.checked_add(1).ok_or_else(|| {
            AppError::Other("maximum generation depth could not be represented".to_string())
        })?;

        Ok(TraversalLimit {
            maximum_entry_depth,
            reject_beyond_limit,
        })
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
        let traversal_limit = self.validate_configuration()?;
        self.collect_entries_with_limit(traversal_limit, classify)
    }

    fn collect_entries_with_limit<F>(
        &self,
        traversal_limit: TraversalLimit,
        classify: &mut F,
    ) -> Result<Vec<CollectedEntry>>
    where
        F: FnMut(&Path) -> io::Result<EntryClassification>,
    {
        let canonical_root = fs::canonicalize(&self.root_path)
            .map_err(|error| self.root_access_error("resolve the generation root", error.kind()))?;
        let metadata = fs::metadata(&canonical_root)
            .map_err(|error| self.root_access_error("inspect the generation root", error.kind()))?;
        if !metadata.is_dir() {
            return Err(AppError::Other(format!(
                "generation root {} must resolve to a directory",
                self.render_root()
            )));
        }

        let mut entries = Vec::new();
        self.collect_directory(
            &canonical_root,
            Path::new(""),
            0,
            traversal_limit,
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
        traversal_limit: TraversalLimit,
        classify: &mut F,
        entries: &mut Vec<CollectedEntry>,
    ) -> Result<()>
    where
        F: FnMut(&Path) -> io::Result<EntryClassification>,
    {
        let read_dir = fs::read_dir(directory)
            .map_err(|error| self.enumeration_error(relative_directory, error.kind()))?;
        let mut children = read_dir
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<io::Result<Vec<_>>>()
            .map_err(|error| self.enumeration_error(relative_directory, error.kind()))?;
        children.sort();

        for name in children {
            let relative_path = relative_directory.join(&name);
            if self.is_excluded(&relative_path) {
                continue;
            }

            let depth = directory_depth.checked_add(1).ok_or_else(|| {
                AppError::Other("generation entry depth could not be represented".to_string())
            })?;
            if depth > traversal_limit.maximum_entry_depth {
                return Err(AppError::Other(format!(
                    "generation without an explicit depth encountered an included entry beyond maximum logical depth {MAX_LOGICAL_DEPTH}"
                )));
            }
            Self::validate_included_name(&name, depth == 1)?;

            let physical_path = directory.join(&name);
            let kind = match classify(&physical_path) {
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
                && (depth < traversal_limit.maximum_entry_depth
                    || traversal_limit.reject_beyond_limit)
            {
                self.collect_directory(
                    &physical_path,
                    &relative_path,
                    depth,
                    traversal_limit,
                    classify,
                    entries,
                )?;
            }
        }

        Ok(())
    }

    fn root_access_error(&self, operation: &str, kind: io::ErrorKind) -> AppError {
        AppError::Other(format!(
            "could not {operation} {} ({kind:?})",
            self.render_root()
        ))
    }

    fn enumeration_error(&self, relative_directory: &Path, kind: io::ErrorKind) -> AppError {
        if relative_directory.as_os_str().is_empty() {
            return self.root_access_error("read the generation root directory", kind);
        }

        AppError::Other(format!(
            "could not enumerate included directory {} ({kind:?})",
            Self::render_relative_path(relative_directory)
        ))
    }

    fn render_root(&self) -> String {
        render_os_component(self.root_path.as_os_str())
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
            let child_depth = depth.checked_add(1).ok_or_else(|| {
                AppError::Other("generation tree depth could not be represented".to_string())
            })?;
            Self::prepare_tree(child, child_depth)?;
        }

        Ok(())
    }

    /// Format the tree as markdown
    fn format_tree(&self, node: &TreeNode, output: &mut String, depth: usize) -> Result<()> {
        for child in &node.children {
            let indent_width = depth.checked_mul(self.indent_size).ok_or_else(|| {
                AppError::Other("generation indentation width could not be represented".to_string())
            })?;
            let indent = " ".repeat(indent_width);
            let name = if child.is_dir {
                format!("{}/", child.serialized_name)
            } else {
                child.serialized_name.clone()
            };

            output.push_str(&format!("{indent}- {name}\n"));

            if !child.children.is_empty() {
                let child_depth = depth.checked_add(1).ok_or_else(|| {
                    AppError::Other("generation tree depth could not be represented".to_string())
                })?;
                self.format_tree(child, output, child_depth)?;
            }
        }

        Ok(())
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
    use crate::{Parser, Validator, Verifier};
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::ffi::OsStrExt;
    use std::panic::{catch_unwind, AssertUnwindSafe};
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

    #[test]
    fn issue_43_invalid_roots_and_empty_generation_reject() {
        let empty = TempDir::new().unwrap();
        assert!(
            Dumper::new(empty.path()).dump().is_err(),
            "an empty directory generated an empty guide body"
        );

        let excluded = TempDir::new().unwrap();
        fs::write(excluded.path().join("only.txt"), "").unwrap();
        let patterns = vec!["only.txt".to_string()];
        assert!(
            Dumper::new(excluded.path())
                .with_exclude_patterns(&patterns)
                .unwrap()
                .dump()
                .is_err(),
            "a fully excluded directory generated an empty guide body"
        );

        let parent = TempDir::new().unwrap();
        let file_root = parent.path().join("root.txt");
        fs::write(&file_root, "").unwrap();
        assert!(
            Dumper::new(&file_root).dump().is_err(),
            "a regular-file root generated an empty guide body"
        );

        let missing_root = parent.path().join("missing");
        assert!(
            Dumper::new(&missing_root).dump().is_err(),
            "a missing root was accepted"
        );

        let directory_only = TempDir::new().unwrap();
        fs::create_dir(directory_only.path().join("empty-child")).unwrap();
        assert_eq!(
            Dumper::new(directory_only.path()).dump().unwrap(),
            "- empty-child/\n",
            "an included empty directory is a representable nonempty entry"
        );
    }

    #[cfg(unix)]
    #[test]
    fn issue_43_unreadable_root_rejects_when_the_platform_enforces_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let root = TempDir::new().unwrap();
        fs::write(root.path().join("file.txt"), "").unwrap();
        let original = fs::metadata(root.path()).unwrap().permissions();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o000)).unwrap();

        if fs::read_dir(root.path()).is_ok() {
            fs::set_permissions(root.path(), original).unwrap();
            return;
        }

        let observed = Dumper::new(root.path()).dump();
        fs::set_permissions(root.path(), original).unwrap();
        assert!(observed.is_err(), "an unreadable root was accepted");
    }

    #[test]
    fn issue_43_numeric_bounds_are_enforced_without_panics() {
        let root = TempDir::new().unwrap();
        fs::create_dir(root.path().join("nested")).unwrap();
        fs::write(root.path().join("nested/file.txt"), "").unwrap();

        for indent in [0, 17, usize::MAX] {
            let observed = catch_unwind(AssertUnwindSafe(|| {
                Dumper::new(root.path()).with_indent_size(indent).dump()
            }));
            assert!(
                matches!(observed, Ok(Err(_))),
                "indent {indent} did not return a bounded rejection: {observed:?}"
            );
        }

        for depth in [257, usize::MAX] {
            let observed = catch_unwind(AssertUnwindSafe(|| {
                Dumper::new(root.path()).with_max_depth(Some(depth)).dump()
            }));
            assert!(
                matches!(observed, Ok(Err(_))),
                "depth {depth} did not return a bounded rejection: {observed:?}"
            );
        }
    }

    #[test]
    fn issue_43_valid_numeric_boundaries_generate_checkable_guides() {
        let root = TempDir::new().unwrap();
        fs::create_dir(root.path().join("nested")).unwrap();
        fs::write(root.path().join("nested/file.txt"), "").unwrap();

        for indent in [1, 16] {
            let source = Dumper::new(root.path())
                .with_indent_size(indent)
                .dump_with_wrapper()
                .unwrap_or_else(|error| panic!("valid indent {indent} failed: {error}"));
            let guide = Parser::new()
                .parse(&source)
                .unwrap_or_else(|error| panic!("indent {indent} was not parseable: {error}"));
            Validator::new()
                .validate_syntax(&guide)
                .unwrap_or_else(|error| panic!("indent {indent} was not valid: {error}"));
            Verifier::new(root.path())
                .verify(&guide)
                .unwrap_or_else(|error| panic!("indent {indent} was not checkable: {error}"));
        }

        for depth in [0, 2, 256] {
            let source = Dumper::new(root.path())
                .with_max_depth(Some(depth))
                .dump_with_wrapper()
                .unwrap_or_else(|error| panic!("valid depth {depth} failed: {error}"));
            let guide = Parser::new()
                .parse(&source)
                .unwrap_or_else(|error| panic!("depth {depth} was not parseable: {error}"));
            Validator::new()
                .validate_syntax(&guide)
                .unwrap_or_else(|error| panic!("depth {depth} was not valid: {error}"));
        }

        let depth_zero = Dumper::new(root.path())
            .with_max_depth(Some(0))
            .dump()
            .unwrap();
        assert_eq!(depth_zero, "- nested/\n");
    }

    #[cfg(unix)]
    #[test]
    fn issue_43_explicit_depth_does_not_inspect_deeper_entries() {
        use std::os::unix::fs::symlink;

        let root = TempDir::new().unwrap();
        fs::create_dir(root.path().join("visible")).unwrap();
        symlink(
            root.path().join("missing-target"),
            root.path().join("visible/outside-listing"),
        )
        .unwrap();

        assert_eq!(
            Dumper::new(root.path())
                .with_max_depth(Some(0))
                .dump()
                .expect("entries below an explicit cutoff are outside the listing"),
            "- visible/\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn issue_43_omitted_depth_rejects_instead_of_silently_truncating() {
        const MAX_LOGICAL_DEPTH: usize = 256;

        let root = TempDir::new().unwrap();
        let mut directory = root.path().to_path_buf();
        for _ in 0..=(MAX_LOGICAL_DEPTH + 1) {
            directory.push("d");
            fs::create_dir(&directory).unwrap();
        }

        assert!(
            Dumper::new(root.path()).dump().is_err(),
            "omitted depth silently emitted a guide beyond logical depth 256"
        );

        let source = Dumper::new(root.path())
            .with_max_depth(Some(MAX_LOGICAL_DEPTH))
            .dump_with_wrapper()
            .expect("an explicit depth of 256 may produce a partial listing");
        let guide = Parser::new()
            .parse(&source)
            .expect("the explicit depth-256 listing must parse");
        Validator::new()
            .validate_syntax(&guide)
            .expect("the explicit depth-256 listing must validate");
    }

    #[cfg(unix)]
    #[test]
    fn issue_43_caller_selected_root_alias_is_a_generation_anchor() {
        use std::os::unix::fs::symlink;

        let target = TempDir::new().unwrap();
        fs::create_dir(target.path().join("nested")).unwrap();
        fs::write(target.path().join("nested/file.txt"), "").unwrap();
        let alias_parent = TempDir::new().unwrap();
        let alias = alias_parent.path().join("selected-root");
        symlink(target.path(), &alias).unwrap();

        let source = Dumper::new(&alias)
            .dump_with_wrapper()
            .expect("a caller-selected root alias must be accepted");
        let guide = Parser::new()
            .parse(&source)
            .expect("root-alias generation must remain parseable");
        Validator::new()
            .validate_syntax(&guide)
            .expect("root-alias generation must remain valid");
        Verifier::new(&alias)
            .verify(&guide)
            .expect("root-alias generation must remain checkable");
    }
}
