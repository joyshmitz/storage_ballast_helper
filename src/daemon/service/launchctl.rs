//! Safe wrappers for modern `launchctl` lifecycle commands.

use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

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
#[derive(Debug, Error)]
pub enum LaunchctlError {
    /// The plist path could not be represented as UTF-8 for `launchctl`.
    #[error("launchctl path is not valid UTF-8: {path}")]
    InvalidPath {
        /// Invalid path.
        path: PathBuf,
    },
    /// The `launchctl` executable could not be spawned.
    #[error("failed to run launchctl: {source}")]
    Spawn {
        /// Spawn failure.
        #[source]
        source: std::io::Error,
    },
    /// `launchctl` returned a non-zero exit status.
    #[error("launchctl {args:?} failed with exit {exit_code:?}: {stderr}")]
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
        LaunchctlDomain, LaunchctlError, LaunchctlServiceTarget, bootstrap_args, parse_print_output,
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
}
