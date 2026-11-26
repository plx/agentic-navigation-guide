use crate::guide_input;
use crate::proven::settings::{self, PathDefault};
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

pub(super) const GUIDE_PATH: &str = "AGENTIC_NAVIGATION_GUIDE_PATH";
pub(super) const GUIDE_ROOT: &str = "AGENTIC_NAVIGATION_GUIDE_ROOT";
pub(super) const GUIDE_NAME: &str = "AGENTIC_NAVIGATION_GUIDE_NAME";
pub(super) const LOG_MODE: &str = "AGENTIC_NAVIGATION_GUIDE_LOG_MODE";
pub(super) const EXECUTION_MODE: &str = "AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE";
pub(super) const DEFAULT_GUIDE_NAME: &str = "AGENTIC_NAVIGATION_GUIDE.md";

#[derive(Default)]
pub(super) struct EnvironmentDefaults {
    guide_path: Option<OsString>,
    guide_root: Option<OsString>,
    guide_name: Option<OsString>,
    log_mode: Option<OsString>,
    execution_mode: Option<OsString>,
}

impl EnvironmentDefaults {
    pub(super) fn capture() -> Self {
        Self {
            guide_path: env::var_os(GUIDE_PATH),
            guide_root: env::var_os(GUIDE_ROOT),
            guide_name: env::var_os(GUIDE_NAME),
            log_mode: env::var_os(LOG_MODE),
            execution_mode: env::var_os(EXECUTION_MODE),
        }
    }

    pub(super) fn guide_path(&self) -> Result<Option<PathBuf>, EnvironmentError> {
        selected_path(self.guide_path.as_ref(), GUIDE_PATH)
    }

    /// Whether `AGENTIC_NAVIGATION_GUIDE_PATH` is set at all; when it is
    /// relevant, a set value either selects an explicit path or is rejected,
    /// and either way the implicit name is not resolved.
    pub(super) fn has_guide_path(&self) -> bool {
        self.guide_path.is_some()
    }

    pub(super) fn guide_root(&self) -> Result<Option<PathBuf>, EnvironmentError> {
        selected_path(self.guide_root.as_ref(), GUIDE_ROOT)
    }

    pub(super) fn guide_name(&self) -> Result<String, EnvironmentError> {
        let Some(raw) = self.guide_name.as_ref() else {
            return Ok(DEFAULT_GUIDE_NAME.to_string());
        };
        let name = raw.to_str().ok_or_else(|| {
            EnvironmentError::new(
                GUIDE_NAME,
                "invalid implicit guide name: the value is not valid UTF-8",
            )
        })?;
        if guide_input::validate_implicit_name(name).is_err() {
            return Err(EnvironmentError::new(
                GUIDE_NAME,
                "invalid implicit guide name: expected exactly one nonempty filename component",
            ));
        }
        Ok(name.to_string())
    }

    /// Accept exactly `quiet`, `default`, or `verbose`
    /// (`proven::settings::log_choice`).
    pub(super) fn log_mode(&self) -> Result<Option<String>, EnvironmentError> {
        selected_choice(self.log_mode.as_ref(), LOG_MODE, |value| {
            settings::log_choice(value).is_some()
        })
    }

    /// Accept exactly `default`, `post-tool-use`, `pre-commit-hook`, or
    /// `github-actions` (`proven::settings::mode_choice`).
    pub(super) fn execution_mode(&self) -> Result<Option<String>, EnvironmentError> {
        selected_choice(self.execution_mode.as_ref(), EXECUTION_MODE, |value| {
            settings::mode_choice(value).is_some()
        })
    }
}

/// Select a path or root default. The verified
/// `proven::settings::path_default` rejects exactly an empty value; a
/// nonempty one stays an operating-system path without lossy conversion.
fn selected_path(
    raw: Option<&OsString>,
    variable: &'static str,
) -> Result<Option<PathBuf>, EnvironmentError> {
    match settings::path_default(raw.map(|raw| raw.is_empty())) {
        PathDefault::Unset => Ok(None),
        PathDefault::Empty => Err(EnvironmentError::new(
            variable,
            "expected a nonempty filesystem path",
        )),
        PathDefault::Selected => Ok(raw.map(PathBuf::from)),
    }
}

/// Select a mode default whose UTF-8 bytes the verified kernel `accepts`.
fn selected_choice(
    raw: Option<&OsString>,
    variable: &'static str,
    accepts: fn(&[u8]) -> bool,
) -> Result<Option<String>, EnvironmentError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let value = raw
        .to_str()
        .filter(|value| accepts(value.as_bytes()))
        .ok_or_else(|| {
            EnvironmentError::new(
                variable,
                match variable {
                    LOG_MODE => "expected one of: quiet, default, verbose",
                    EXECUTION_MODE => {
                        "expected one of: default, post-tool-use, pre-commit-hook, github-actions"
                    }
                    _ => "the value is invalid",
                },
            )
        })?;
    Ok(Some(value.to_string()))
}

#[derive(Debug)]
pub(super) struct EnvironmentError {
    variable: &'static str,
    expectation: &'static str,
}

impl EnvironmentError {
    fn new(variable: &'static str, expectation: &'static str) -> Self {
        Self {
            variable,
            expectation,
        }
    }
}

impl fmt::Display for EnvironmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid environment default {}: {}; the value is not shown",
            self.variable, self.expectation
        )
    }
}

impl std::error::Error for EnvironmentError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_46_mode_environment_values_are_exact_and_complete() {
        for value in ["quiet", "default", "verbose"] {
            let defaults = EnvironmentDefaults {
                log_mode: Some(OsString::from(value)),
                ..EnvironmentDefaults::default()
            };
            assert_eq!(
                defaults.log_mode().expect("valid log mode").as_deref(),
                Some(value)
            );
        }

        for value in [
            "default",
            "post-tool-use",
            "pre-commit-hook",
            "github-actions",
        ] {
            let defaults = EnvironmentDefaults {
                execution_mode: Some(OsString::from(value)),
                ..EnvironmentDefaults::default()
            };
            assert_eq!(
                defaults
                    .execution_mode()
                    .expect("valid execution mode")
                    .as_deref(),
                Some(value)
            );
        }

        for value in [
            "",
            "Quiet",
            " quiet",
            "verbose ",
            "unknown",
            "quiet\nforged",
        ] {
            let defaults = EnvironmentDefaults {
                log_mode: Some(OsString::from(value)),
                ..EnvironmentDefaults::default()
            };
            let diagnostic = defaults
                .log_mode()
                .expect_err("invalid log mode")
                .to_string();
            assert!(diagnostic.contains(LOG_MODE));
            assert!(!diagnostic.contains("forged"));
        }

        let defaults = EnvironmentDefaults {
            execution_mode: Some(OsString::from("ISSUE46_EXECUTION_SECRET\nforged-line")),
            ..EnvironmentDefaults::default()
        };
        let diagnostic = defaults
            .execution_mode()
            .expect_err("control-bearing execution mode")
            .to_string();
        assert!(!diagnostic.contains("ISSUE46_EXECUTION_SECRET"));
        assert!(!diagnostic.contains("forged-line"));
    }

    #[test]
    fn issue_46_guide_names_remain_implicit_single_components() {
        for value in ["GUIDE.md", "導航.md", "é.md"] {
            let defaults = EnvironmentDefaults {
                guide_name: Some(OsString::from(value)),
                ..EnvironmentDefaults::default()
            };
            assert_eq!(defaults.guide_name().expect("valid guide name"), value);
        }

        for value in ["", ".", "..", "nested/GUIDE.md", r"nested\GUIDE.md"] {
            let defaults = EnvironmentDefaults {
                guide_name: Some(OsString::from(value)),
                ..EnvironmentDefaults::default()
            };
            let diagnostic = defaults
                .guide_name()
                .expect_err("invalid guide name")
                .to_string();
            assert!(diagnostic.contains(GUIDE_NAME));
            assert!(diagnostic.contains("invalid implicit guide name"));
        }

        let defaults = EnvironmentDefaults {
            guide_name: Some(OsString::from("ISSUE46_NAME_SECRET\nforged/guide")),
            ..EnvironmentDefaults::default()
        };
        let diagnostic = defaults
            .guide_name()
            .expect_err("control-bearing guide name")
            .to_string();
        assert!(!diagnostic.contains("ISSUE46_NAME_SECRET"));
        assert!(!diagnostic.contains("forged"));
    }

    #[cfg(unix)]
    #[test]
    fn issue_46_path_defaults_preserve_non_utf8_while_text_defaults_reject_it() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};

        let raw = OsString::from_vec(vec![b'p', 0xff, b't', b'h']);
        let path_defaults = EnvironmentDefaults {
            guide_path: Some(raw.clone()),
            guide_root: Some(raw.clone()),
            ..EnvironmentDefaults::default()
        };
        assert_eq!(
            path_defaults
                .guide_path()
                .expect("non-UTF-8 path")
                .expect("configured path")
                .as_os_str()
                .as_bytes(),
            raw.as_os_str().as_bytes()
        );
        assert_eq!(
            path_defaults
                .guide_root()
                .expect("non-UTF-8 root")
                .expect("configured root")
                .as_os_str()
                .as_bytes(),
            raw.as_os_str().as_bytes()
        );

        let name_defaults = EnvironmentDefaults {
            guide_name: Some(raw),
            ..EnvironmentDefaults::default()
        };
        let diagnostic = name_defaults
            .guide_name()
            .expect_err("non-UTF-8 name")
            .to_string();
        assert!(diagnostic.contains(GUIDE_NAME));
        assert!(diagnostic.contains("not valid UTF-8"));
    }
}
