//! Shared CLI error formatting for guide commands.

use agentic_navigation_guide::errors::{AppError, ErrorFormatter};
use agentic_navigation_guide::types::{Config, ExecutionMode};

/// Guide command context for command-specific error text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GuideCommand {
    /// `check` command.
    Check,
    /// `verify` command.
    Verify,
}

/// Format and print a guide-related command error.
pub(crate) fn report_guide_error(
    error: &AppError,
    config: &Config,
    file_content: Option<&str>,
    command: GuideCommand,
) {
    let formatted = format_guide_error(error, config, file_content, command);
    eprintln!("{formatted}");
}

fn format_guide_error(
    error: &AppError,
    config: &Config,
    file_content: Option<&str>,
    command: GuideCommand,
) -> String {
    match (command, config.execution_mode) {
        (GuideCommand::Verify, ExecutionMode::PostToolUse) => {
            format_verify_post_tool_use_error(error, config, file_content)
        }
        (_, ExecutionMode::GitHubActions) => {
            format_github_actions_error(error, config, file_content, command)
        }
        _ => ErrorFormatter::format_with_context(error, file_content),
    }
}

fn format_verify_post_tool_use_error(
    error: &AppError,
    config: &Config,
    file_content: Option<&str>,
) -> String {
    let display_guide_path = config
        .original_guide_path
        .as_deref()
        .unwrap_or("AGENTIC_NAVIGATION_GUIDE.md");

    let display_root_path = config.original_root_path.as_deref().unwrap_or("./");

    match error {
        AppError::Syntax(_) => {
            let error_detail = ErrorFormatter::format_with_context(error, file_content);

            format!(
                "The agentic navigation guide at {display_guide_path} has a syntax error:\n\n{error_detail}"
            )
        }
        AppError::Semantic(semantic_error) => {
            let error_detail = semantic_error.to_string();

            format!(
                "The agentic navigation guide has become out-of-date vis-a-vis the current state of the file system.\n\n\
                - guide: {display_guide_path}\n\
                - root: {display_root_path}\n\
                - details:\n  - {error_detail}"
            )
        }
        _ => ErrorFormatter::format_with_context(error, file_content),
    }
}

fn format_github_actions_error(
    error: &AppError,
    config: &Config,
    file_content: Option<&str>,
    command: GuideCommand,
) -> String {
    // Callers set this to match their historical diagnostic path: `check`
    // reports the resolved guide file, while `verify` preserves user input.
    let display_guide_path = config
        .original_guide_path
        .as_deref()
        .unwrap_or("AGENTIC_NAVIGATION_GUIDE.md");
    let mut output = String::new();

    output.push_str(match command {
        GuideCommand::Check => "❌ Navigation guide syntax check failed\n\n",
        GuideCommand::Verify => "❌ Navigation guide verification failed\n\n",
    });

    if let Some(line_num) = error_line_number(error) {
        output.push_str(&format!("{display_guide_path}:{line_num}: {error}\n"));

        if let Some(content) = file_content {
            if let Some(line) = content.lines().nth(line_num.saturating_sub(1)) {
                let trimmed_line = line.trim_end();
                output.push_str(&format!("  {trimmed_line}\n"));
            }
        }
    } else {
        output.push_str(&format!("{display_guide_path}: {error}\n"));
    }

    output
}

fn error_line_number(error: &AppError) -> Option<usize> {
    match error {
        AppError::Syntax(e) => e.line_number(),
        AppError::Semantic(e) => Some(e.line_number()),
        _ => None,
    }
}
