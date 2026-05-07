//! systemd service integration.

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::core::errors::{Result, SbhError};
use crate::platform::pal::ServiceManager;

use super::{SYSTEMD_UNIT_NAME, resolve_sbh_binary};

/// Parameters controlling systemd unit file generation and lifecycle commands.
#[derive(Debug, Clone)]
pub struct SystemdConfig {
    /// Whether to operate in user scope (`--user`).
    pub user_scope: bool,
    /// Absolute path to the sbh binary baked into the unit file.
    pub binary_path: PathBuf,
    /// Paths sbh needs read-write access to under `ProtectSystem=strict`.
    pub read_write_paths: Vec<PathBuf>,
}

impl SystemdConfig {
    /// Build a config from the current environment.
    ///
    /// `user_scope` controls system vs user service placement.
    pub fn from_env(user_scope: bool) -> Result<Self> {
        let binary_path = resolve_sbh_binary()?;
        let read_write_paths = default_read_write_paths(user_scope);
        Ok(Self {
            user_scope,
            binary_path,
            read_write_paths,
        })
    }

    /// Directory where the unit file is written.
    #[must_use]
    pub fn unit_dir(&self) -> PathBuf {
        if self.user_scope {
            let home = env::var_os("HOME").map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
            home.join(".config/systemd/user")
        } else {
            PathBuf::from("/etc/systemd/system")
        }
    }

    /// Full path to the generated unit file.
    #[must_use]
    pub fn unit_path(&self) -> PathBuf {
        self.unit_dir().join(SYSTEMD_UNIT_NAME)
    }
}

/// [`ServiceManager`] implementation that drives `systemctl` and generates
/// a hardened systemd unit file.
#[derive(Debug, Clone)]
pub struct SystemdServiceManager {
    config: SystemdConfig,
}

impl SystemdServiceManager {
    /// Create a new manager with the given config.
    #[must_use]
    pub fn new(config: SystemdConfig) -> Self {
        Self { config }
    }

    /// Create a manager from the current environment.
    pub fn from_env(user_scope: bool) -> Result<Self> {
        Ok(Self::new(SystemdConfig::from_env(user_scope)?))
    }

    /// Access the underlying config (for reading unit path, etc.).
    #[must_use]
    pub fn config(&self) -> &SystemdConfig {
        &self.config
    }

    /// Generate the full systemd unit file content.
    #[must_use]
    pub fn generate_unit_file(&self) -> String {
        let binary = self.config.binary_path.display();
        let rw_paths = self
            .config
            .read_write_paths
            .iter()
            .map(|p| {
                let s = p.display().to_string();
                if s.contains(' ') || s.contains('"') {
                    // Systemd quote escaping: escape internal quotes and wrap in quotes.
                    format!("\"{}\"", s.replace('"', "\\\""))
                } else {
                    s
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        let mut unit = String::with_capacity(2048);

        writeln!(unit, "[Unit]").ok();
        writeln!(
            unit,
            "Description=Storage Ballast Helper - Disk Space Guardian"
        )
        .ok();
        writeln!(
            unit,
            "Documentation=https://github.com/Dicklesworthstone/storage_ballast_helper"
        )
        .ok();
        writeln!(unit, "After=local-fs.target").ok();
        writeln!(unit, "Wants=local-fs.target").ok();
        writeln!(unit).ok();

        writeln!(unit, "[Service]").ok();
        if self.config.user_scope {
            writeln!(unit, "Type=simple").ok();
        } else {
            writeln!(unit, "Type=notify").ok();
            writeln!(unit, "WatchdogSec=60").ok();
        }

        writeln!(unit, "ExecStart={binary} daemon").ok();
        writeln!(unit, "ExecReload=/bin/kill -HUP $MAINPID").ok();
        writeln!(unit, "Restart=on-failure").ok();
        writeln!(unit, "RestartSec=10").ok();
        writeln!(unit, "TimeoutStopSec=30").ok();
        writeln!(unit).ok();

        writeln!(unit, "# Low priority - never compete with build workloads").ok();
        writeln!(unit, "Nice=19").ok();
        writeln!(unit, "IOSchedulingClass=idle").ok();
        writeln!(unit, "IOSchedulingPriority=7").ok();
        writeln!(unit).ok();

        writeln!(unit, "# Security hardening").ok();
        writeln!(unit, "NoNewPrivileges=true").ok();

        if !self.config.user_scope {
            writeln!(unit, "ProtectSystem=strict").ok();
            writeln!(unit, "ReadWritePaths={rw_paths}").ok();
            writeln!(unit, "ProtectHome=false").ok();
            writeln!(unit, "PrivateTmp=false").ok();
            writeln!(unit, "ProtectKernelTunables=true").ok();
            writeln!(unit, "ProtectControlGroups=true").ok();
            writeln!(unit, "RestrictSUIDSGID=true").ok();
            writeln!(unit, "LimitNOFILE=4096").ok();
        }
        writeln!(unit).ok();

        writeln!(unit, "# Resource limits").ok();
        writeln!(unit, "MemoryMax=256M").ok();
        writeln!(unit, "CPUQuota=10%").ok();
        writeln!(unit).ok();

        if !self.config.user_scope {
            writeln!(unit, "# Logging").ok();
            writeln!(unit, "StandardOutput=journal").ok();
            writeln!(unit, "StandardError=journal").ok();
            writeln!(unit, "SyslogIdentifier=sbh").ok();
            writeln!(unit).ok();
        }

        writeln!(unit, "[Install]").ok();
        if self.config.user_scope {
            writeln!(unit, "WantedBy=default.target").ok();
        } else {
            writeln!(unit, "WantedBy=multi-user.target").ok();
        }

        unit
    }

    fn check_binary_ownership(&self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if !self.config.user_scope
                && let Ok(meta) = fs::metadata(&self.config.binary_path)
                && meta.uid() != 0
            {
                eprintln!(
                    "[SBH-WARN] SECURITY RISK: System service binary '{}' is NOT owned by root (uid={}).",
                    self.config.binary_path.display(),
                    meta.uid()
                );
                eprintln!(
                    "[SBH-WARN] A non-root user could replace this binary and gain root privileges."
                );
                eprintln!(
                    "[SBH-WARN] Recommendation: 'sudo chown root:root {}'",
                    self.config.binary_path.display()
                );
            }
        }
    }

    fn systemctl_args(&self, args: &[&str]) -> Vec<String> {
        let mut cmd_args: Vec<String> = Vec::with_capacity(args.len() + 1);
        if self.config.user_scope {
            cmd_args.push("--user".to_string());
        }
        cmd_args.extend(args.iter().map(|s| (*s).to_string()));
        cmd_args
    }

    fn run_systemctl(&self, args: &[&str]) -> Result<String> {
        let full_args = self.systemctl_args(args);
        let output = Command::new("systemctl")
            .args(&full_args)
            .output()
            .map_err(|source| SbhError::Io {
                path: PathBuf::from("systemctl"),
                source,
            })?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if output.status.success() {
            Ok(stdout.trim().to_string())
        } else {
            Err(SbhError::Runtime {
                details: format!(
                    "systemctl {} failed (exit {}): {}",
                    full_args.join(" "),
                    output.status.code().unwrap_or(-1),
                    stderr.trim()
                ),
            })
        }
    }

    fn run_systemctl_lenient(&self, args: &[&str]) -> String {
        let full_args = self.systemctl_args(args);
        let output = Command::new("systemctl").args(&full_args).output();
        match output {
            Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
            Err(_) => String::new(),
        }
    }
}

impl ServiceManager for SystemdServiceManager {
    fn install(&self) -> Result<()> {
        self.check_binary_ownership();

        let unit_dir = self.config.unit_dir();
        let unit_path = self.config.unit_path();
        let unit_content = self.generate_unit_file();

        fs::create_dir_all(&unit_dir).map_err(|source| SbhError::Io {
            path: unit_dir.clone(),
            source,
        })?;

        fs::write(&unit_path, &unit_content).map_err(|source| SbhError::Io {
            path: unit_path.clone(),
            source,
        })?;

        self.run_systemctl(&["daemon-reload"])?;
        self.run_systemctl(&["enable", SYSTEMD_UNIT_NAME])?;

        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        let unit_path = self.config.unit_path();

        self.run_systemctl_lenient(&["stop", SYSTEMD_UNIT_NAME]);
        self.run_systemctl_lenient(&["disable", SYSTEMD_UNIT_NAME]);

        if unit_path.exists() {
            fs::remove_file(&unit_path).map_err(|source| SbhError::Io {
                path: unit_path.clone(),
                source,
            })?;
        }

        self.run_systemctl(&["daemon-reload"])?;

        Ok(())
    }

    fn status(&self) -> Result<String> {
        let state = self.run_systemctl_lenient(&["is-active", SYSTEMD_UNIT_NAME]);
        if state.is_empty() {
            return Ok("unknown".to_string());
        }
        Ok(state)
    }

    fn watchdog_enabled(&self, watchdog_sec: u64) -> bool {
        let socket_path = systemd_notify_socket();
        systemd_watchdog_enabled(watchdog_sec, socket_path.as_deref())
    }

    fn notify_watchdog(&self, status: &str) -> Result<()> {
        if let Some(socket_path) = systemd_notify_socket() {
            sd_notify_watchdog(status, &socket_path);
        }
        Ok(())
    }

    fn restart(&self) -> Result<()> {
        self.run_systemctl(&["restart", SYSTEMD_UNIT_NAME])?;
        Ok(())
    }

    fn logs_path(&self) -> Result<Option<PathBuf>> {
        Ok(None)
    }

    fn is_loaded(&self) -> Result<bool> {
        let state = self.run_systemctl_lenient(&["is-enabled", SYSTEMD_UNIT_NAME]);
        Ok(matches!(
            state.as_str(),
            "enabled" | "static" | "linked" | "generated" | "transient"
        ))
    }
}

fn systemd_notify_socket() -> Option<String> {
    env::var("NOTIFY_SOCKET")
        .ok()
        .filter(|path| !path.is_empty())
}

fn systemd_watchdog_enabled(watchdog_sec: u64, socket_path: Option<&str>) -> bool {
    watchdog_sec > 0 && socket_path.is_some_and(|path| !path.is_empty())
}

fn sd_notify_watchdog(status: &str, socket_path: &str) {
    #[cfg(target_os = "linux")]
    {
        sd_notify_linux(status, socket_path);
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = status;
        let _ = socket_path;
    }
}

#[cfg(target_os = "linux")]
fn sd_notify_linux(status: &str, socket_path: &str) {
    use std::os::unix::net::UnixDatagram;

    let msg = format!("WATCHDOG=1\nSTATUS={status}\n");
    let Ok(sock) = UnixDatagram::unbound() else {
        return;
    };

    let _ = sock.send_to(msg.as_bytes(), socket_path);
}

fn default_read_write_paths(user_scope: bool) -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("/tmp"), PathBuf::from("/var/tmp")];
    if !user_scope {
        paths.push(PathBuf::from("/var/lib/sbh"));
    }
    for candidate in ["/data", "/data/tmp"] {
        let p = PathBuf::from(candidate);
        if p.is_dir() {
            paths.push(p);
        }
    }
    if let Some(home) = env::var_os("HOME") {
        let home = PathBuf::from(home);
        paths.push(home.join(".local/share/sbh"));
        paths.push(home.join(".config/sbh"));
    }
    paths
}

#[cfg(test)]
mod watchdog_tests {
    use super::*;

    #[test]
    fn systemd_watchdog_requires_timeout_and_notify_socket() {
        assert!(systemd_watchdog_enabled(60, Some("/run/systemd/notify")));
        assert!(!systemd_watchdog_enabled(0, Some("/run/systemd/notify")));
        assert!(!systemd_watchdog_enabled(60, None));
        assert!(!systemd_watchdog_enabled(60, Some("")));
    }
}
