//! Safe wrappers for modern `launchctl` lifecycle commands.

use std::error::Error as StdError;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::core::errors::SbhError;

/// launchd domain target used by modern `launchctl` commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchctlDomain {
    /// System-wide LaunchDaemon domain.
    System,
    /// Per-user non-GUI domain.
    User(u32),
    /// Per-user GUI LaunchAgent domain.
    Gui(u32),
}

impl LaunchctlDomain {
    /// Render the domain target syntax expected by `launchctl`.
    #[must_use]
    pub fn as_arg(&self) -> String {
        match self {
            Self::System => "system".to_string(),
            Self::User(uid) => format!("user/{uid}"),
            Self::Gui(uid) => format!("gui/{uid}"),
        }
    }
}

/// Full service target, e.g. `gui/501/com.sbh.daemon`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchctlServiceTarget {
    domain: LaunchctlDomain,
    label: String,
}

impl LaunchctlServiceTarget {
    /// Create a launchd service target from a domain and label.
    #[must_use]
    pub fn new(domain: LaunchctlDomain, label: impl Into<String>) -> Self {
        Self {
            domain,
            label: label.into(),
        }
    }

    /// Render the service-target syntax expected by `launchctl`.
    #[must_use]
    pub fn as_arg(&self) -> String {
        format!("{}/{}", self.domain.as_arg(), self.label)
    }
}

/// Captured stdout/stderr from a successful `launchctl` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchctlOutput {
    /// Arguments passed after the `launchctl` executable.
    pub args: Vec<String>,
    /// BSD-style process exit code. `None` means the process ended by signal.
    pub exit_code: Option<i32>,
    /// Captured stdout, decoded lossily as UTF-8.
    pub stdout: String,
    /// Captured stderr, decoded lossily as UTF-8.
    pub stderr: String,
}

/// Parsed subset of `launchctl print <service-target>` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedStatus {
    /// Service target that was queried.
    pub target: String,
    /// Raw `launchctl print` stdout.
    pub raw: String,
    /// Whether `launchctl print` found the service.
    pub loaded: bool,
    /// launchd state value when present.
    pub state: Option<String>,
    /// Running process id when present.
    pub pid: Option<u32>,
    /// Active reference count when reported by launchd.
    pub active_count: Option<u32>,
    /// Plist path reported by launchd.
    pub plist_path: Option<String>,
    /// Last exit status when present.
    pub last_exit_status: Option<i32>,
}

impl ParsedStatus {
    /// Human-readable status used by the service manager.
    #[must_use]
    pub fn summary(&self) -> String {
        self.state.as_ref().map_or_else(
            || {
                if self.pid.is_some() {
                    "running".to_string()
                } else if self.loaded {
                    "loaded".to_string()
                } else {
                    "not loaded".to_string()
                }
            },
            Clone::clone,
        )
    }
}

/// Errors returned by `launchctl` wrappers.
#[derive(Debug)]
pub enum LaunchctlError {
    /// The plist path could not be represented as UTF-8 for `launchctl`.
    InvalidPath {
        /// Invalid path.
        path: PathBuf,
    },
    /// The `launchctl` executable could not be spawned.
    Spawn {
        /// Spawn failure.
        source: std::io::Error,
    },
    /// `launchctl` returned a non-zero exit status.
    CommandFailed {
        /// Arguments passed after the executable.
        args: Vec<String>,
        /// BSD-style exit code. `None` means the process ended by signal.
        exit_code: Option<i32>,
        /// Captured stdout.
        stdout: String,
        /// Captured stderr.
        stderr: String,
    },
}

impl LaunchctlError {
    /// Human-facing diagnostic with the failed command and concrete next steps.
    #[must_use]
    pub fn diagnostic(&self) -> String {
        match self {
            Self::InvalidPath { path } => format!(
                "launchctl cannot use plist path {} because it is not valid UTF-8. \
                 Use a UTF-8 plist path and verify the plist with: plutil -lint {}",
                path.display(),
                path.display()
            ),
            Self::Spawn { source } => format!(
                "failed to run launchctl: {source}. \
                 Verify launchctl is available with: /bin/launchctl version"
            ),
            Self::CommandFailed {
                args,
                exit_code,
                stdout,
                stderr,
            } => format!(
                "{} failed with exit {}. {} {}",
                format_launchctl_command(args),
                exit_code.map_or_else(|| "signal".to_string(), |code| code.to_string()),
                format_captured_output(stdout, stderr),
                remediation_for_args(args)
            ),
        }
    }

    /// Whether the failure means the service/domain is absent.
    #[must_use]
    pub fn is_not_loaded(&self) -> bool {
        match self {
            Self::CommandFailed { stdout, stderr, .. } => {
                let output = format!("{stdout}\n{stderr}").to_ascii_lowercase();
                output.contains("could not find service")
                    || output.contains("no such process")
                    || output.contains("service is not loaded")
                    || output.contains("does not exist")
            }
            Self::InvalidPath { .. } | Self::Spawn { .. } => false,
        }
    }

    /// Whether the failure means the service is already bootstrapped.
    #[must_use]
    pub fn is_already_loaded(&self) -> bool {
        match self {
            Self::CommandFailed { stdout, stderr, .. } => {
                let output = format!("{stdout}\n{stderr}").to_ascii_lowercase();
                output.contains("service is already loaded")
                    || output.contains("already bootstrapped")
                    || output.contains("bootstrap failed: 5")
            }
            Self::InvalidPath { .. } | Self::Spawn { .. } => false,
        }
    }
}

impl fmt::Display for LaunchctlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.diagnostic())
    }
}

impl StdError for LaunchctlError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Spawn { source } => Some(source),
            Self::InvalidPath { .. } | Self::CommandFailed { .. } => None,
        }
    }
}

impl From<LaunchctlError> for SbhError {
    fn from(value: LaunchctlError) -> Self {
        match value {
            LaunchctlError::Spawn { source } => Self::Io {
                path: PathBuf::from("launchctl"),
                source,
            },
            other => Self::Runtime {
                details: other.to_string(),
            },
        }
    }
}

fn format_launchctl_command(args: &[String]) -> String {
    std::iter::once("launchctl")
        .chain(args.iter().map(String::as_str))
        .map(shell_quote)
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    if value.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':' | '@' | '%' | '+')
    }) {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn format_captured_output(stdout: &str, stderr: &str) -> String {
    format!(
        "stdout: {}; stderr: {}.",
        compact_output(stdout),
        compact_output(stderr)
    )
}

fn compact_output(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "<empty>".to_string();
    }
    let mut lines = trimmed.lines();
    let mut rendered = lines.by_ref().take(6).collect::<Vec<_>>().join("\\n");
    if lines.next().is_some() {
        rendered.push_str("\\n...");
    }
    rendered
}

fn remediation_for_args(args: &[String]) -> String {
    match args.first().map(String::as_str) {
        Some("bootstrap") => bootstrap_remediation(args),
        Some("bootout") => target_remediation(
            args.get(1).map(String::as_str),
            "Inspect the loaded state with",
            "If the service is already absent, no action is needed.",
        ),
        Some("kickstart") => {
            let target = args.last().map(String::as_str);
            target_remediation(
                target,
                "Verify the service is loaded with",
                "If it is not loaded, run launchctl bootstrap for the plist before kickstart.",
            )
        }
        Some("print") => target_remediation(
            args.get(1).map(String::as_str),
            "Retry the status probe with",
            "If the target is absent, install with sbh install --launchd --scope user or choose the correct scope.",
        ),
        _ => format!("Retry manually with: {}", format_launchctl_command(args)),
    }
}

fn bootstrap_remediation(args: &[String]) -> String {
    let Some(domain) = args.get(1) else {
        return "Retry manually with the intended launchctl bootstrap domain and plist path."
            .to_string();
    };
    let Some(plist) = args.get(2) else {
        return format!(
            "Inspect the launchd domain with: launchctl print {}",
            shell_quote(domain)
        );
    };
    let target = bootstrap_target(domain, plist).map_or_else(
        || "the stale service target".to_string(),
        |target| shell_quote(&target),
    );
    format!(
        "Verify the plist with: plutil -lint {}. \
         Inspect the launchd domain with: launchctl print {}. \
         If launchd has stale state, run: launchctl bootout {}; then retry: {}",
        shell_quote(plist),
        shell_quote(domain),
        target,
        format_launchctl_command(args)
    )
}

fn bootstrap_target(domain: &str, plist: &str) -> Option<String> {
    let label = Path::new(plist).file_stem()?.to_str()?;
    Some(format!("{domain}/{label}"))
}

fn target_remediation(target: Option<&str>, probe_prefix: &str, fallback: &str) -> String {
    target.map_or_else(
        || fallback.to_string(),
        |target| {
            format!(
                "{probe_prefix}: launchctl print {}. {fallback}",
                shell_quote(target)
            )
        },
    )
}

/// Pick the launchd target domain for an install scope.
#[must_use]
pub fn domain_for_scope(user_scope: bool) -> LaunchctlDomain {
    if !user_scope {
        return LaunchctlDomain::System;
    }

    let uid = current_uid();
    let gui = LaunchctlDomain::Gui(uid);
    if domain_exists(&gui) {
        gui
    } else {
        LaunchctlDomain::User(uid)
    }
}

/// Run `launchctl bootstrap <domain-target> <plist-path>`.
pub fn bootstrap(
    domain: &LaunchctlDomain,
    plist_path: &Path,
) -> Result<LaunchctlOutput, LaunchctlError> {
    run_launchctl(bootstrap_args(domain, plist_path)?)
}

/// Run `launchctl bootout <service-target>`.
pub fn bootout(target: &LaunchctlServiceTarget) -> Result<LaunchctlOutput, LaunchctlError> {
    run_launchctl(vec!["bootout".to_string(), target.as_arg()])
}

/// Run `launchctl kickstart [-k] <service-target>`.
pub fn kickstart(
    target: &LaunchctlServiceTarget,
    kill_existing: bool,
) -> Result<LaunchctlOutput, LaunchctlError> {
    let mut args = vec!["kickstart".to_string()];
    if kill_existing {
        args.push("-k".to_string());
    }
    args.push(target.as_arg());
    run_launchctl(args)
}

/// Run `launchctl print <service-target>` and parse the result.
pub fn print(target: &LaunchctlServiceTarget) -> Result<ParsedStatus, LaunchctlError> {
    let target_arg = target.as_arg();
    let output = run_launchctl(vec!["print".to_string(), target_arg.clone()])?;
    Ok(parse_print_output(target_arg, &output.stdout))
}

fn domain_exists(domain: &LaunchctlDomain) -> bool {
    let domain_arg = domain.as_arg();
    Command::new("launchctl")
        .args(["print", domain_arg.as_str()])
        .output()
        .is_ok_and(|output| output.status.success())
}

fn current_uid() -> u32 {
    #[cfg(unix)]
    {
        nix::unistd::geteuid().as_raw()
    }
    #[cfg(not(unix))]
    {
        0
    }
}

fn bootstrap_args(
    domain: &LaunchctlDomain,
    plist_path: &Path,
) -> Result<Vec<String>, LaunchctlError> {
    let path = plist_path
        .to_str()
        .ok_or_else(|| LaunchctlError::InvalidPath {
            path: plist_path.to_path_buf(),
        })?
        .to_string();
    Ok(vec!["bootstrap".to_string(), domain.as_arg(), path])
}

fn run_launchctl(args: Vec<String>) -> Result<LaunchctlOutput, LaunchctlError> {
    let output = Command::new("launchctl")
        .args(&args)
        .output()
        .map_err(|source| LaunchctlError::Spawn { source })?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let exit_code = output.status.code();
    if output.status.success() {
        Ok(LaunchctlOutput {
            args,
            exit_code,
            stdout,
            stderr,
        })
    } else {
        Err(LaunchctlError::CommandFailed {
            args,
            exit_code,
            stdout,
            stderr,
        })
    }
}

fn parse_print_output(target: String, raw: &str) -> ParsedStatus {
    ParsedStatus {
        target,
        raw: raw.to_string(),
        loaded: true,
        state: parse_string_assignment(raw, "state"),
        pid: parse_u32_assignment(raw, "pid"),
        active_count: parse_u32_assignment(raw, "active count"),
        plist_path: parse_string_assignment(raw, "path"),
        last_exit_status: parse_i32_assignment(raw, "last exit code")
            .or_else(|| parse_i32_assignment(raw, "last exit status")),
    }
}

fn parse_string_assignment(raw: &str, key: &str) -> Option<String> {
    assignment_value(raw, key).map(ToOwned::to_owned)
}

fn parse_u32_assignment(raw: &str, key: &str) -> Option<u32> {
    assignment_value(raw, key)?.parse().ok()
}

fn parse_i32_assignment(raw: &str, key: &str) -> Option<i32> {
    assignment_value(raw, key)?.parse().ok()
}

fn assignment_value<'a>(raw: &'a str, key: &str) -> Option<&'a str> {
    raw.lines().find_map(|line| {
        let (left, right) = line.trim().split_once('=')?;
        if left.trim() == key {
            Some(right.trim().trim_matches('"'))
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        LaunchctlDomain, LaunchctlError, LaunchctlServiceTarget, bootstrap_args,
        format_launchctl_command, parse_print_output,
    };

    #[test]
    fn domain_targets_render_modern_launchctl_syntax() {
        assert_eq!(LaunchctlDomain::System.as_arg(), "system");
        assert_eq!(LaunchctlDomain::Gui(501).as_arg(), "gui/501");
        assert_eq!(LaunchctlDomain::User(501).as_arg(), "user/501");
    }

    #[test]
    fn service_target_appends_label_to_domain() {
        let target = LaunchctlServiceTarget::new(LaunchctlDomain::Gui(501), "com.sbh.daemon");

        assert_eq!(target.as_arg(), "gui/501/com.sbh.daemon");
    }

    #[test]
    fn bootstrap_args_use_domain_and_plist_path() {
        let args = bootstrap_args(
            &LaunchctlDomain::System,
            Path::new("/Library/LaunchDaemons/com.sbh.daemon.plist"),
        )
        .expect("path should be valid UTF-8");

        assert_eq!(
            args,
            vec![
                "bootstrap",
                "system",
                "/Library/LaunchDaemons/com.sbh.daemon.plist"
            ]
        );
    }

    #[test]
    fn print_parser_extracts_state_pid_and_exit_status() {
        let raw = r"
gui/501/com.sbh.daemon = {
    active count = 1
    path = /Users/me/Library/LaunchAgents/com.sbh.daemon.plist
    state = running
    pid = 4242
    last exit code = 0
}
";

        let status = parse_print_output("gui/501/com.sbh.daemon".to_string(), raw);

        assert!(status.loaded);
        assert_eq!(status.state.as_deref(), Some("running"));
        assert_eq!(status.pid, Some(4242));
        assert_eq!(status.active_count, Some(1));
        assert_eq!(
            status.plist_path.as_deref(),
            Some("/Users/me/Library/LaunchAgents/com.sbh.daemon.plist")
        );
        assert_eq!(status.last_exit_status, Some(0));
        assert_eq!(status.summary(), "running");
    }

    #[test]
    fn failed_print_not_loaded_is_detected_from_stderr() {
        let error = LaunchctlError::CommandFailed {
            args: vec!["print".to_string(), "gui/501/com.sbh.daemon".to_string()],
            exit_code: Some(113),
            stdout: String::new(),
            stderr: "Could not find service \"com.sbh.daemon\" in domain for user gui: 501"
                .to_string(),
        };

        assert!(error.is_not_loaded());
    }

    #[test]
    fn bootstrap_error_five_is_treated_as_already_loaded() {
        let error = LaunchctlError::CommandFailed {
            args: vec![
                "bootstrap".to_string(),
                "gui/501".to_string(),
                "/tmp/com.sbh.daemon.plist".to_string(),
            ],
            exit_code: Some(5),
            stdout: String::new(),
            stderr: "Bootstrap failed: 5: Input/output error".to_string(),
        };

        assert!(error.is_already_loaded());
    }

    #[test]
    fn launchctl_command_diagnostic_includes_command_output_and_fix() {
        let error = LaunchctlError::CommandFailed {
            args: vec![
                "bootstrap".to_string(),
                "gui/501".to_string(),
                "/Users/me/Library/LaunchAgents/com.sbh.daemon.plist".to_string(),
            ],
            exit_code: Some(5),
            stdout: "domain bootstrap details".to_string(),
            stderr: "Bootstrap failed: 5: Input/output error".to_string(),
        };

        let diagnostic = error.to_string();

        assert!(diagnostic.contains(
            "launchctl bootstrap gui/501 /Users/me/Library/LaunchAgents/com.sbh.daemon.plist failed with exit 5"
        ));
        assert!(diagnostic.contains("stdout: domain bootstrap details"));
        assert!(diagnostic.contains("stderr: Bootstrap failed: 5: Input/output error"));
        assert!(diagnostic.contains(
            "Verify the plist with: plutil -lint /Users/me/Library/LaunchAgents/com.sbh.daemon.plist"
        ));
        assert!(
            diagnostic.contains("launchctl bootout gui/501/com.sbh.daemon"),
            "diagnostic should name the stale launchd target: {diagnostic}"
        );
    }

    #[test]
    fn launchctl_kickstart_diagnostic_names_status_probe() {
        let error = LaunchctlError::CommandFailed {
            args: vec![
                "kickstart".to_string(),
                "-k".to_string(),
                "gui/501/com.sbh.daemon".to_string(),
            ],
            exit_code: Some(113),
            stdout: String::new(),
            stderr: "Could not find service".to_string(),
        };

        let diagnostic = error.to_string();

        assert!(diagnostic.contains("launchctl kickstart -k gui/501/com.sbh.daemon"));
        assert!(diagnostic.contains("launchctl print gui/501/com.sbh.daemon"));
        assert!(diagnostic.contains("run launchctl bootstrap for the plist before kickstart"));
    }

    #[test]
    fn launchctl_command_formatter_quotes_spaces() {
        let command = format_launchctl_command(&[
            "bootstrap".to_string(),
            "gui/501".to_string(),
            "/Users/me/Library/LaunchAgents/com example.sbh.plist".to_string(),
        ]);

        assert_eq!(
            command,
            "launchctl bootstrap gui/501 '/Users/me/Library/LaunchAgents/com example.sbh.plist'"
        );
    }
}
