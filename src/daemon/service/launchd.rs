//! launchd service integration.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::core::errors::{Result, SbhError};
use crate::platform::pal::ServiceManager;
use crate::platform::types::PalError;

use super::{LAUNCHD_LABEL, resolve_sbh_binary};

/// Parameters controlling launchd plist generation and lifecycle commands.
#[derive(Debug, Clone)]
pub struct LaunchdConfig {
    /// Whether to install as user agent (vs system daemon).
    pub user_scope: bool,
    /// Absolute path to the sbh binary.
    pub binary_path: PathBuf,
    /// Path to stdout log file.
    pub stdout_log: PathBuf,
    /// Path to stderr log file.
    pub stderr_log: PathBuf,
}

impl LaunchdConfig {
    /// Build a config from the current environment.
    ///
    /// If `user_scope` is false but the current process is not running as root,
    /// automatically escalates to user-scope to avoid permission errors when
    /// creating log directories under `/usr/local/var/log/`.
    pub fn from_env(user_scope: bool) -> Result<Self> {
        let effective_user_scope = if !user_scope && !is_running_as_root() {
            eprintln!("[SBH] Not running as root - defaulting to user-scope launchd installation");
            true
        } else {
            user_scope
        };

        let binary_path = resolve_sbh_binary()?;
        let (stdout_log, stderr_log) = default_launchd_log_paths(effective_user_scope);
        Ok(Self {
            user_scope: effective_user_scope,
            binary_path,
            stdout_log,
            stderr_log,
        })
    }

    /// Directory where the plist is installed.
    #[must_use]
    pub fn plist_dir(&self) -> PathBuf {
        if self.user_scope {
            let home = env::var_os("HOME").map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
            home.join("Library/LaunchAgents")
        } else {
            PathBuf::from("/Library/LaunchDaemons")
        }
    }

    /// Full path to the plist file.
    #[must_use]
    pub fn plist_path(&self) -> PathBuf {
        self.plist_dir().join(format!("{LAUNCHD_LABEL}.plist"))
    }
}

/// [`ServiceManager`] implementation that generates a launchd plist and drives
/// `launchctl` for lifecycle operations on macOS.
#[derive(Debug, Clone)]
pub struct LaunchdServiceManager {
    config: LaunchdConfig,
}

impl LaunchdServiceManager {
    /// Create a new manager with the given config.
    #[must_use]
    pub fn new(config: LaunchdConfig) -> Self {
        Self { config }
    }

    /// Create a manager from the current environment.
    pub fn from_env(user_scope: bool) -> Result<Self> {
        Ok(Self::new(LaunchdConfig::from_env(user_scope)?))
    }

    /// Access the underlying config.
    #[must_use]
    pub fn config(&self) -> &LaunchdConfig {
        &self.config
    }

    /// Generate the launchd plist XML content.
    #[must_use]
    pub fn generate_plist(&self) -> String {
        let binary = escape_xml(&self.config.binary_path.to_string_lossy());
        let stdout_log = escape_xml(&self.config.stdout_log.to_string_lossy());
        let stderr_log = escape_xml(&self.config.stderr_log.to_string_lossy());

        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LAUNCHD_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{binary}</string>
        <string>daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>ThrottleInterval</key>
    <integer>10</integer>
    <key>Nice</key>
    <integer>19</integer>
    <key>LowPriorityIO</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{stdout_log}</string>
    <key>StandardErrorPath</key>
    <string>{stderr_log}</string>
</dict>
</plist>
"#
        )
    }

    #[allow(clippy::unused_self)]
    fn run_launchctl(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("launchctl")
            .args(args)
            .output()
            .map_err(|source| SbhError::Io {
                path: PathBuf::from("launchctl"),
                source,
            })?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if output.status.success() {
            Ok(stdout.trim().to_string())
        } else {
            Err(SbhError::Runtime {
                details: format!(
                    "launchctl {} failed (exit {}): {}",
                    args.join(" "),
                    output.status.code().unwrap_or(-1),
                    stderr.trim()
                ),
            })
        }
    }

    #[allow(clippy::unused_self)]
    fn run_launchctl_lenient(&self, args: &[&str]) -> String {
        let output = Command::new("launchctl").args(args).output();
        match output {
            Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
            Err(_) => String::new(),
        }
    }
}

impl ServiceManager for LaunchdServiceManager {
    fn install(&self) -> Result<()> {
        let plist_dir = self.config.plist_dir();
        let plist_path = self.config.plist_path();
        let plist_content = self.generate_plist();

        if let Some(log_parent) = self.config.stdout_log.parent() {
            fs::create_dir_all(log_parent).map_err(|source| SbhError::Io {
                path: log_parent.to_path_buf(),
                source,
            })?;
        }

        fs::create_dir_all(&plist_dir).map_err(|source| SbhError::Io {
            path: plist_dir.clone(),
            source,
        })?;

        fs::write(&plist_path, &plist_content).map_err(|source| SbhError::Io {
            path: plist_path.clone(),
            source,
        })?;

        let path_str = plist_path
            .to_str()
            .ok_or_else(|| SbhError::Runtime {
                details: "plist path is not valid UTF-8".to_string(),
            })?
            .to_string();
        self.run_launchctl(&["load", &path_str])?;

        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        let plist_path = self.config.plist_path();

        if let Ok(path_str) = plist_path.to_str().ok_or(()) {
            self.run_launchctl_lenient(&["unload", path_str]);
        }

        if plist_path.exists() {
            fs::remove_file(&plist_path).map_err(|source| SbhError::Io {
                path: plist_path.clone(),
                source,
            })?;
        }

        Ok(())
    }

    fn status(&self) -> Result<String> {
        let output = self.run_launchctl_lenient(&["list", LAUNCHD_LABEL]);
        if output.is_empty() {
            return Ok("not loaded".to_string());
        }
        if output.contains("\"Label\"") || output.contains(LAUNCHD_LABEL) {
            Ok("loaded".to_string())
        } else {
            Ok("unknown".to_string())
        }
    }

    fn restart(&self) -> Result<()> {
        Err(PalError::not_implemented_with_bead("launchd", "restart", Some("bd-1y7j.3")).into())
    }

    fn logs_path(&self) -> Result<Option<PathBuf>> {
        Ok(Some(self.config.stdout_log.clone()))
    }

    fn is_loaded(&self) -> Result<bool> {
        Ok(self.status()? == "loaded")
    }
}

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

fn is_running_as_root() -> bool {
    #[cfg(unix)]
    {
        nix::unistd::geteuid().is_root()
    }
    #[cfg(not(unix))]
    {
        false
    }
}

fn default_launchd_log_paths(user_scope: bool) -> (PathBuf, PathBuf) {
    if user_scope {
        let home = env::var_os("HOME").map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
        let log_dir = home.join("Library/Logs/sbh");
        (log_dir.join("sbh.log"), log_dir.join("sbh.err"))
    } else {
        (
            PathBuf::from("/usr/local/var/log/sbh/sbh.log"),
            PathBuf::from("/usr/local/var/log/sbh/sbh.err"),
        )
    }
}
