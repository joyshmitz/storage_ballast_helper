//! launchd service integration.

use std::env;
use std::fs;
use std::path::PathBuf;

use crate::core::errors::{Result, SbhError};
use crate::platform::pal::ServiceManager;

use super::launchctl::{self, LaunchctlDomain, LaunchctlServiceTarget};
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

    fn domain_target(&self) -> LaunchctlDomain {
        launchctl::domain_for_scope(self.config.user_scope)
    }

    fn service_target(&self) -> LaunchctlServiceTarget {
        LaunchctlServiceTarget::new(self.domain_target(), LAUNCHD_LABEL)
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

        let domain = self.domain_target();
        match launchctl::bootstrap(&domain, &plist_path) {
            Ok(_) => {}
            Err(error) if error.is_already_loaded() => {
                let target = LaunchctlServiceTarget::new(domain.clone(), LAUNCHD_LABEL);
                let _ = launchctl::bootout(&target);
                launchctl::bootstrap(&domain, &plist_path)?;
            }
            Err(error) => return Err(error.into()),
        }

        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        let plist_path = self.config.plist_path();

        if let Err(error) = launchctl::bootout(&self.service_target())
            && !error.is_not_loaded()
        {
            return Err(error.into());
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
        match launchctl::print(&self.service_target()) {
            Ok(status) => Ok(status.summary()),
            Err(error) if error.is_not_loaded() => Ok("not loaded".to_string()),
            Err(error) => Err(error.into()),
        }
    }

    fn restart(&self) -> Result<()> {
        launchctl::kickstart(&self.service_target(), true)?;
        Ok(())
    }

    fn logs_path(&self) -> Result<Option<PathBuf>> {
        Ok(Some(self.config.stdout_log.clone()))
    }

    fn is_loaded(&self) -> Result<bool> {
        match launchctl::print(&self.service_target()) {
            Ok(_) => Ok(true),
            Err(error) if error.is_not_loaded() => Ok(false),
            Err(error) => Err(error.into()),
        }
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
