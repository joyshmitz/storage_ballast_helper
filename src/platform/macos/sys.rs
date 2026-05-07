//! Safe macOS filesystem syscall adapters.

#![cfg(target_os = "macos")]
#![allow(missing_docs)]

use std::ffi::OsStr;
use std::io;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use nix::mount::MntFlags;
use nix::sys::statfs::{Statfs, statfs as nix_statfs};
use parking_lot::RwLock;
use plist::Value;
use std::sync::OnceLock;

const STATFS_STRUCT_SIZE_BYTES: usize = core::mem::size_of::<libc::statfs>();
const STATFS_MOUNT_NAME_BYTES: usize = core::mem::size_of::<[libc::c_char; 1024]>();
const STATFS_TYPE_NAME_BYTES: usize = core::mem::size_of::<[libc::c_char; 16]>();

const _: [(); 2168] = [(); STATFS_STRUCT_SIZE_BYTES];
const _: [(); 1024] = [(); STATFS_MOUNT_NAME_BYTES];
const _: [(); 16] = [(); STATFS_TYPE_NAME_BYTES];

const APFS_CACHE_TTL_SECS: u64 = 5 * 60;
const APFS_CACHE_TTL: Duration = Duration::from_secs(APFS_CACHE_TTL_SECS);
static APFS_INVENTORY_CACHE: OnceLock<RwLock<Option<(Instant, ApfsInventory)>>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatfsSnapshot {
    pub mount_point: PathBuf,
    pub device: String,
    pub fs_type: String,
    pub block_size: u64,
    pub blocks: u64,
    pub blocks_free: u64,
    pub blocks_available: u64,
    pub is_readonly: bool,
}

impl StatfsSnapshot {
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.blocks.saturating_mul(self.block_size)
    }

    #[must_use]
    pub fn free_bytes(&self) -> u64 {
        self.blocks_free.saturating_mul(self.block_size)
    }

    #[must_use]
    pub fn available_bytes(&self) -> u64 {
        self.blocks_available.saturating_mul(self.block_size)
    }

    #[must_use]
    pub fn is_ram_backed(&self) -> bool {
        matches!(
            self.fs_type.to_ascii_lowercase().as_str(),
            "devfs" | "mfs" | "ramfs" | "tmpfs"
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApfsInventory {
    pub containers: Vec<ApfsContainer>,
    pub volumes: Vec<ApfsVolume>,
}

impl ApfsInventory {
    #[must_use]
    pub fn volume_for_device(&self, device_id: &str) -> Option<&ApfsVolume> {
        let normalized = normalize_device_id(device_id);
        self.volumes
            .iter()
            .find(|volume| volume.device_id == normalized)
            .or_else(|| {
                parent_apfs_volume_device(&normalized).and_then(|parent| {
                    self.volumes
                        .iter()
                        .find(|volume| volume.device_id == parent)
                })
            })
    }

    #[must_use]
    pub fn sibling_volume_names(&self, volume: &ApfsVolume) -> Vec<String> {
        let mut siblings: Vec<String> = self
            .volumes
            .iter()
            .filter(|candidate| candidate.container_id == volume.container_id)
            .filter(|candidate| candidate.device_id != volume.device_id)
            .map(ApfsVolume::display_name)
            .collect();
        siblings.sort();
        siblings.dedup();
        siblings
    }

    #[must_use]
    pub fn unattributed_container_used_bytes(&self, container_id: &str) -> Option<u64> {
        let container = self
            .containers
            .iter()
            .find(|candidate| candidate.container_id == container_id)?;
        let total_bytes = container.capacity_total_bytes?;
        let available_bytes = container.capacity_available_bytes?;
        let used_bytes = total_bytes.checked_sub(available_bytes)?;
        let volume_bytes = self
            .volumes
            .iter()
            .filter(|volume| volume.container_id == container_id)
            .filter_map(|volume| volume.capacity_in_use_bytes)
            .fold(0_u64, u64::saturating_add);
        used_bytes.checked_sub(volume_bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApfsContainer {
    pub container_id: String,
    pub uuid: Option<String>,
    pub capacity_total_bytes: Option<u64>,
    pub capacity_available_bytes: Option<u64>,
    pub physical_stores: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApfsVolume {
    pub device_id: String,
    pub container_id: String,
    pub name: Option<String>,
    pub roles: Vec<ApfsVolumeRole>,
    pub capacity_in_use_bytes: Option<u64>,
    pub container_total_bytes: Option<u64>,
    pub container_available_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalSnapshotInfo {
    pub name: String,
    pub date: Option<String>,
    pub retained_bytes_estimate: Option<u64>,
    pub mount_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApfsVolumeRole {
    Data,
    System,
    Preboot,
    Vm,
    Update,
    Recovery,
    Other(String),
}

impl ApfsVolume {
    #[must_use]
    pub fn has_role(&self, role: &ApfsVolumeRole) -> bool {
        self.roles.iter().any(|candidate| candidate == role)
    }

    #[must_use]
    pub fn role_label(&self) -> Option<String> {
        if self.roles.is_empty() {
            return None;
        }
        Some(
            self.roles
                .iter()
                .map(ApfsVolumeRole::as_str)
                .collect::<Vec<_>>()
                .join(","),
        )
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.device_id.clone())
    }
}

pub fn statfs(path: &Path) -> io::Result<StatfsSnapshot> {
    let raw = nix_statfs(path).map_err(nix_error)?;
    let location = whichdisk::resolve(path).ok();
    let mount_point = location.as_ref().map_or_else(
        || path.to_path_buf(),
        |info| info.mount_point().to_path_buf(),
    );
    let device = location
        .as_ref()
        .map_or_else(String::new, |info| os_str_to_string(info.device()));

    Ok(snapshot_from_statfs(&raw, mount_point, device))
}

pub fn mounted_filesystems() -> io::Result<Vec<StatfsSnapshot>> {
    let mut filesystems = Vec::new();
    for mount in whichdisk::list().map_err(io::Error::other)? {
        match statfs_for_mount(mount.mount_point(), mount.device()) {
            Ok(snapshot) => filesystems.push(snapshot),
            Err(error) => eprintln!(
                "[sbh] warning: skipping macOS mount {}: {error}",
                mount.mount_point().display()
            ),
        }
    }
    filesystems.sort_by(|left, right| {
        right
            .mount_point
            .as_os_str()
            .len()
            .cmp(&left.mount_point.as_os_str().len())
            .then_with(|| left.mount_point.cmp(&right.mount_point))
    });
    Ok(filesystems)
}

pub fn apfs_inventory() -> io::Result<ApfsInventory> {
    let cache = APFS_INVENTORY_CACHE.get_or_init(|| RwLock::new(None));
    {
        let cached = cache.read();
        if let Some((collected_at, inventory)) = &*cached
            && collected_at.elapsed() < APFS_CACHE_TTL
        {
            return Ok(inventory.clone());
        }
    }

    let output = Command::new("/usr/sbin/diskutil")
        .args(["apfs", "list", "-plist"])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "diskutil apfs list -plist failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let inventory = parse_apfs_inventory(&output.stdout)?;
    *cache.write() = Some((Instant::now(), inventory.clone()));
    Ok(inventory)
}

pub fn local_time_machine_snapshots(
    mount: &Path,
    inventory: Option<&ApfsInventory>,
    volume: Option<&ApfsVolume>,
) -> io::Result<Vec<LocalSnapshotInfo>> {
    let output = Command::new("/usr/bin/tmutil")
        .arg("listlocalsnapshots")
        .arg(mount)
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "tmutil listlocalsnapshots {} failed with status {}: {}",
            mount.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let retained_total_estimate = inventory.zip(volume).and_then(|(inventory, volume)| {
        // APFS does not expose per-snapshot byte counts through tmutil; treat
        // container usage that is not attributed to mounted volumes as a
        // retained-by-snapshots estimate.
        inventory.unattributed_container_used_bytes(&volume.container_id)
    });
    Ok(parse_tmutil_local_snapshots(
        &String::from_utf8_lossy(&output.stdout),
        mount,
        retained_total_estimate,
    ))
}

#[must_use]
pub fn parse_tmutil_local_snapshots(
    raw: &str,
    mount_path: &Path,
    retained_total_estimate: Option<u64>,
) -> Vec<LocalSnapshotInfo> {
    let names: Vec<String> = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.starts_with("Snapshots for disk "))
        .filter(|line| line.starts_with("com.apple.TimeMachine."))
        .map(ToOwned::to_owned)
        .collect();
    let estimates = distribute_snapshot_estimate(retained_total_estimate, names.len());
    names
        .into_iter()
        .enumerate()
        .map(|(index, name)| LocalSnapshotInfo {
            date: local_snapshot_date(&name),
            name,
            retained_bytes_estimate: estimates.get(index).copied().flatten(),
            mount_path: mount_path.to_path_buf(),
        })
        .collect()
}

pub fn parse_apfs_inventory(raw: &[u8]) -> io::Result<ApfsInventory> {
    let value = Value::from_reader(Cursor::new(raw)).map_err(io::Error::other)?;
    let root = value.as_dictionary().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "APFS plist root is not a dictionary",
        )
    })?;
    let containers = root
        .get("Containers")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "APFS plist has no Containers array",
            )
        })?;

    let mut parsed_containers = Vec::new();
    let mut parsed_volumes = Vec::new();
    for container_value in containers {
        let container = container_value.as_dictionary().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "APFS container is not a dictionary",
            )
        })?;
        let container_id = required_device_id(container, "ContainerReference")?;
        let capacity_total_bytes = optional_u64(container, "CapacityCeiling");
        let capacity_available_bytes = optional_u64(container, "CapacityFree");
        let physical_stores = container
            .get("PhysicalStores")
            .and_then(Value::as_array)
            .map(|stores| {
                stores
                    .iter()
                    .filter_map(Value::as_dictionary)
                    .filter_map(|store| optional_device_id(store, "DeviceIdentifier"))
                    .collect()
            })
            .unwrap_or_default();

        parsed_containers.push(ApfsContainer {
            container_id: container_id.clone(),
            uuid: optional_string(container, "APFSContainerUUID"),
            capacity_total_bytes,
            capacity_available_bytes,
            physical_stores,
        });

        if let Some(volumes) = container.get("Volumes").and_then(Value::as_array) {
            for volume_value in volumes {
                let volume = volume_value.as_dictionary().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "APFS volume is not a dictionary",
                    )
                })?;
                let device_id = required_device_id(volume, "DeviceIdentifier")?;
                let roles = volume_roles(volume);
                parsed_volumes.push(ApfsVolume {
                    device_id,
                    container_id: container_id.clone(),
                    name: optional_string(volume, "Name"),
                    roles,
                    capacity_in_use_bytes: optional_u64(volume, "CapacityInUse"),
                    container_total_bytes: capacity_total_bytes,
                    container_available_bytes: capacity_available_bytes,
                });
            }
        }
    }

    Ok(ApfsInventory {
        containers: parsed_containers,
        volumes: parsed_volumes,
    })
}

fn statfs_for_mount(mount_point: &Path, device: &OsStr) -> io::Result<StatfsSnapshot> {
    let raw = nix_statfs(mount_point).map_err(nix_error)?;
    Ok(snapshot_from_statfs(
        &raw,
        mount_point.to_path_buf(),
        os_str_to_string(device),
    ))
}

fn snapshot_from_statfs(raw: &Statfs, mount_point: PathBuf, device: String) -> StatfsSnapshot {
    StatfsSnapshot {
        mount_point,
        device,
        fs_type: raw.filesystem_type_name().to_string(),
        block_size: u64::from(raw.block_size()),
        blocks: raw.blocks(),
        blocks_free: raw.blocks_free(),
        blocks_available: raw.blocks_available(),
        is_readonly: raw.flags().contains(MntFlags::MNT_RDONLY),
    }
}

fn os_str_to_string(value: &OsStr) -> String {
    value.to_string_lossy().into_owned()
}

fn nix_error(error: nix::errno::Errno) -> io::Error {
    io::Error::from_raw_os_error(error as i32)
}

fn required_device_id(dict: &plist::Dictionary, key: &'static str) -> io::Result<String> {
    optional_device_id(dict, key).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("APFS plist entry missing {key}"),
        )
    })
}

fn optional_device_id(dict: &plist::Dictionary, key: &str) -> Option<String> {
    optional_string(dict, key).map(|device_id| with_dev_prefix(&device_id))
}

fn optional_string(dict: &plist::Dictionary, key: &str) -> Option<String> {
    dict.get(key)
        .and_then(Value::as_string)
        .map(ToOwned::to_owned)
}

fn optional_u64(dict: &plist::Dictionary, key: &str) -> Option<u64> {
    dict.get(key).and_then(Value::as_unsigned_integer)
}

fn distribute_snapshot_estimate(total: Option<u64>, count: usize) -> Vec<Option<u64>> {
    let Some(total) = total else {
        return vec![None; count];
    };
    if count == 0 {
        return Vec::new();
    }
    let count_u64 = u64::try_from(count).unwrap_or(u64::MAX);
    let per_snapshot = total / count_u64;
    let mut remainder = total % count_u64;
    (0..count)
        .map(|_| {
            let extra = u64::from(remainder > 0);
            remainder = remainder.saturating_sub(extra);
            Some(per_snapshot.saturating_add(extra))
        })
        .collect()
}

fn local_snapshot_date(name: &str) -> Option<String> {
    let suffix = name.strip_prefix("com.apple.TimeMachine.")?;
    let timestamp = suffix.strip_suffix(".local").unwrap_or(suffix);
    if timestamp.len() >= "YYYY-MM-DD-HHMMSS".len()
        && timestamp
            .chars()
            .take("YYYY-MM-DD-HHMMSS".len())
            .all(|candidate| candidate.is_ascii_digit() || candidate == '-')
    {
        Some(timestamp[.."YYYY-MM-DD-HHMMSS".len()].to_string())
    } else {
        None
    }
}

fn volume_roles(dict: &plist::Dictionary) -> Vec<ApfsVolumeRole> {
    let mut roles: Vec<ApfsVolumeRole> = dict
        .get("Roles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_string)
        .map(ApfsVolumeRole::from)
        .collect();
    roles.sort();
    roles.dedup();
    roles
}

fn normalize_device_id(device_id: &str) -> String {
    with_dev_prefix(device_id.trim())
}

fn with_dev_prefix(device_id: &str) -> String {
    if device_id.starts_with("/dev/") {
        device_id.to_string()
    } else {
        format!("/dev/{device_id}")
    }
}

fn parent_apfs_volume_device(device_id: &str) -> Option<String> {
    let device_name = device_id.rsplit('/').next().unwrap_or(device_id);
    let after_disk = device_name.strip_prefix("disk")?;
    let disk_number_len = after_disk.find('s')?;
    if disk_number_len == 0
        || !after_disk[..disk_number_len]
            .chars()
            .all(|candidate| candidate.is_ascii_digit())
        || after_disk.matches('s').count() < 2
    {
        return None;
    }

    let suffix_start = device_id.rfind('s')?;
    Some(device_id[..suffix_start].to_string())
}

impl From<&str> for ApfsVolumeRole {
    fn from(value: &str) -> Self {
        match value {
            "Data" => Self::Data,
            "System" => Self::System,
            "Preboot" => Self::Preboot,
            "VM" => Self::Vm,
            "Update" => Self::Update,
            "Recovery" => Self::Recovery,
            other => Self::Other(other.to_string()),
        }
    }
}

impl Ord for ApfsVolumeRole {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.sort_key()
            .cmp(&other.sort_key())
            .then_with(|| match (self, other) {
                (Self::Other(left), Self::Other(right)) => left.cmp(right),
                _ => std::cmp::Ordering::Equal,
            })
    }
}

impl PartialOrd for ApfsVolumeRole {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl ApfsVolumeRole {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Data => "Data",
            Self::System => "System",
            Self::Preboot => "Preboot",
            Self::Vm => "VM",
            Self::Update => "Update",
            Self::Recovery => "Recovery",
            Self::Other(value) => value,
        }
    }

    fn sort_key(&self) -> u8 {
        match self {
            Self::System => 0,
            Self::Data => 1,
            Self::Preboot => 2,
            Self::Vm => 3,
            Self::Update => 4,
            Self::Recovery => 5,
            Self::Other(_) => 6,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        ApfsVolumeRole, mounted_filesystems, parent_apfs_volume_device, parse_apfs_inventory,
        parse_tmutil_local_snapshots, statfs,
    };

    #[test]
    fn statfs_tmp_reports_plausible_values() {
        let stats = statfs(Path::new("/tmp")).expect("/tmp statfs should work on macOS");
        assert_eq!(stats.block_size, 4096);
        assert!(stats.blocks > 0);
        assert!(stats.blocks_available > 0);
        assert!(stats.total_bytes() > stats.available_bytes());
        assert!(!stats.fs_type.is_empty());
        assert!(stats.mount_point.is_absolute());
    }

    #[test]
    fn mounted_filesystems_include_root() {
        let mounts = mounted_filesystems().expect("mounted filesystems should be discoverable");
        assert!(!mounts.is_empty());
        assert!(
            mounts
                .iter()
                .any(|mount| mount.mount_point == Path::new("/"))
        );
    }

    #[test]
    fn parses_diskutil_apfs_inventory() {
        let sample = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
 "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Containers</key>
  <array>
    <dict>
      <key>ContainerReference</key><string>disk3</string>
      <key>APFSContainerUUID</key><string>container-uuid</string>
      <key>CapacityCeiling</key><integer>1995218165760</integer>
      <key>CapacityFree</key><integer>667402285056</integer>
      <key>PhysicalStores</key>
      <array>
        <dict><key>DeviceIdentifier</key><string>disk0s2</string></dict>
      </array>
      <key>Volumes</key>
      <array>
        <dict>
          <key>DeviceIdentifier</key><string>disk3s1</string>
          <key>Name</key><string>Macintosh HD</string>
          <key>Roles</key><array><string>System</string></array>
          <key>CapacityInUse</key><integer>12269772800</integer>
        </dict>
        <dict>
          <key>DeviceIdentifier</key><string>disk3s5</string>
          <key>Name</key><string>Data</string>
          <key>Roles</key><array><string>Data</string></array>
          <key>CapacityInUse</key><integer>1290235314176</integer>
        </dict>
        <dict>
          <key>DeviceIdentifier</key><string>disk3s6</string>
          <key>Name</key><string>VM</string>
          <key>Roles</key><array><string>VM</string></array>
        </dict>
      </array>
    </dict>
  </array>
</dict>
</plist>"#;

        let inventory = parse_apfs_inventory(sample).expect("sample should parse");
        assert_eq!(inventory.containers.len(), 1);
        assert_eq!(inventory.volumes.len(), 3);

        let data = inventory
            .volume_for_device("/dev/disk3s5")
            .expect("data volume should be indexed");
        assert_eq!(data.container_id, "/dev/disk3");
        assert!(data.has_role(&ApfsVolumeRole::Data));
        assert_eq!(data.container_total_bytes, Some(1_995_218_165_760));
        assert_eq!(data.container_available_bytes, Some(667_402_285_056));

        let system_snapshot = inventory
            .volume_for_device("/dev/disk3s1s1")
            .expect("system snapshot should map to its parent volume");
        assert!(system_snapshot.has_role(&ApfsVolumeRole::System));
    }

    #[test]
    fn parent_apfs_volume_strips_snapshot_suffix() {
        assert_eq!(
            parent_apfs_volume_device("/dev/disk3s1s1").as_deref(),
            Some("/dev/disk3s1")
        );
        assert_eq!(parent_apfs_volume_device("/dev/disk3s1"), None);
    }

    #[test]
    fn parses_tmutil_local_snapshot_names_and_distributes_estimate() {
        let raw = "\
Snapshots for disk /:
com.apple.TimeMachine.2026-05-07-010203.local
com.apple.TimeMachine.2026-05-07-040506.local
";

        let snapshots = parse_tmutil_local_snapshots(raw, Path::new("/"), Some(101));

        assert_eq!(snapshots.len(), 2);
        assert_eq!(
            snapshots[0].name,
            "com.apple.TimeMachine.2026-05-07-010203.local"
        );
        assert_eq!(snapshots[0].date.as_deref(), Some("2026-05-07-010203"));
        assert_eq!(snapshots[0].retained_bytes_estimate, Some(51));
        assert_eq!(snapshots[1].retained_bytes_estimate, Some(50));
        assert_eq!(snapshots[0].mount_path, Path::new("/"));
    }

    #[test]
    fn tmutil_parser_ignores_header_and_non_time_machine_snapshots() {
        let raw = "\
Snapshots for disk /:
com.apple.os.update-abc
";

        let snapshots = parse_tmutil_local_snapshots(raw, Path::new("/"), Some(100));

        assert!(snapshots.is_empty());
    }
}
