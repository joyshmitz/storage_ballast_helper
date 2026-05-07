//! launchd service integration.

use std::env;
use std::fs;
use std::path::PathBuf;

use plist::{Dictionary, Value};

use crate::core::config::PathsConfig;
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
    /// Directory launchd should use as the daemon working directory.
    pub working_directory: PathBuf,
    /// Config path exported to the daemon environment.
    pub config_path: PathBuf,
    /// RUST_LOG value exported to the daemon environment.
    pub rust_log: String,
}

impl LaunchdConfig {
    /// Build a config from the current environment.
    pub fn from_env(user_scope: bool) -> Result<Self> {
        if !user_scope && !is_running_as_root() {
            return Err(SbhError::PermissionDenied {
                path: PathBuf::from("/Library/LaunchDaemons"),
            });
        }

        let binary_path = resolve_sbh_binary()?;
        let (stdout_log, stderr_log) = default_launchd_log_paths(user_scope);
        let paths = PathsConfig::default();
        let working_directory = paths
            .state_file
            .parent()
            .map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
        let config_path = env::var_os("SBH_CONFIG_PATH")
            .or_else(|| env::var_os("SBH_CONFIG"))
            .map_or(paths.config_file, PathBuf::from);
        let rust_log = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
        Ok(Self {
            user_scope,
            binary_path,
            stdout_log,
            stderr_log,
            working_directory,
            config_path,
            rust_log,
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
        let mut bytes = Vec::new();
        self.plist_value()
            .to_writer_xml(&mut bytes)
            .expect("writing launchd plist to memory should not fail");
        String::from_utf8(bytes).expect("plist crate must emit UTF-8 XML")
    }

    fn plist_value(&self) -> Value {
        let mut keep_alive = Dictionary::new();
        keep_alive.insert("SuccessfulExit".to_string(), Value::Boolean(false));
        keep_alive.insert("Crashed".to_string(), Value::Boolean(true));

        let mut env_vars = Dictionary::new();
        env_vars.insert(
            "SBH_CONFIG_PATH".to_string(),
            Value::String(self.config.config_path.to_string_lossy().into_owned()),
        );
        env_vars.insert(
            "SBH_CONFIG".to_string(),
            Value::String(self.config.config_path.to_string_lossy().into_owned()),
        );
        env_vars.insert(
            "RUST_LOG".to_string(),
            Value::String(self.config.rust_log.clone()),
        );

        let mut root = Dictionary::new();
        root.insert(
            "Label".to_string(),
            Value::String(LAUNCHD_LABEL.to_string()),
        );
        root.insert(
            "ProgramArguments".to_string(),
            Value::Array(vec![
                Value::String(self.config.binary_path.to_string_lossy().into_owned()),
                Value::String("daemon".to_string()),
            ]),
        );
        root.insert("RunAtLoad".to_string(), Value::Boolean(true));
        root.insert("KeepAlive".to_string(), Value::Dictionary(keep_alive));
        root.insert("ThrottleInterval".to_string(), Value::Integer(60.into()));
        root.insert("Nice".to_string(), Value::Integer(19.into()));
        root.insert(
            "ProcessType".to_string(),
            Value::String("Background".to_string()),
        );
        root.insert("LowPriorityIO".to_string(), Value::Boolean(true));
        root.insert(
            "WorkingDirectory".to_string(),
            Value::String(self.config.working_directory.to_string_lossy().into_owned()),
        );
        root.insert(
            "StandardOutPath".to_string(),
            Value::String(self.config.stdout_log.to_string_lossy().into_owned()),
        );
        root.insert(
            "StandardErrorPath".to_string(),
            Value::String(self.config.stderr_log.to_string_lossy().into_owned()),
        );
        root.insert(
            "EnvironmentVariables".to_string(),
            Value::Dictionary(env_vars),
        );
        Value::Dictionary(root)
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
