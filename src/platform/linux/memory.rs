//! Linux memory readers for the PAL.

#![cfg(target_os = "linux")]
#![allow(missing_docs)]

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::core::errors::{Result, SbhError};
use crate::platform::pal::MemoryInfo;

pub(super) fn read_memory_info() -> Result<MemoryInfo> {
    let raw = fs::read_to_string("/proc/meminfo").map_err(|source| SbhError::Io {
        path: PathBuf::from("/proc/meminfo"),
        source,
    })?;
    parse_meminfo(&raw)
}

pub(super) fn parse_meminfo(raw: &str) -> Result<MemoryInfo> {
    let mut values = HashMap::<String, u64>::new();

    for line in raw.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Some((key, rest)) = line.split_once(':') else {
            return Err(SbhError::MountParse {
                details: format!("invalid meminfo line (missing ':'): {line}"),
            });
        };
        let mut parts = rest.split_whitespace();
        let Some(value_raw) = parts.next() else {
            return Err(SbhError::MountParse {
                details: format!("missing meminfo value in line: {line}"),
            });
        };
        let value = value_raw
            .parse::<u64>()
            .map_err(|err| SbhError::MountParse {
                details: format!("invalid meminfo numeric value in line {line:?}: {err}"),
            })?;

        let bytes = match parts.next() {
            None => value,
            Some("kB") => value.saturating_mul(1024),
            Some(unit) => {
                return Err(SbhError::MountParse {
                    details: format!("unsupported meminfo unit in line {line:?}: {unit}"),
                });
            }
        };
        values.insert(key.trim().to_string(), bytes);
    }

    let required = |key: &str| {
        values
            .get(key)
            .copied()
            .ok_or_else(|| SbhError::MountParse {
                details: format!("missing required meminfo field: {key}"),
            })
    };

    Ok(MemoryInfo {
        total_bytes: required("MemTotal")?,
        available_bytes: required("MemAvailable")?,
        swap_total_bytes: required("SwapTotal")?,
        swap_free_bytes: required("SwapFree")?,
    })
}

#[cfg(test)]
mod tests {
    use crate::core::errors::SbhError;

    use super::parse_meminfo;

    #[test]
    fn parses_meminfo_with_kib_units() {
        let info = parse_meminfo(
            "MemTotal:       32768000 kB\n\
             MemAvailable:   16384000 kB\n\
             SwapTotal:       8192000 kB\n\
             SwapFree:        4096000 kB\n",
        )
        .expect("meminfo should parse");
        assert_eq!(info.total_bytes, 33_554_432_000);
        assert_eq!(info.available_bytes, 16_777_216_000);
        assert_eq!(info.swap_total_bytes, 8_388_608_000);
        assert_eq!(info.swap_free_bytes, 4_194_304_000);
    }

    #[test]
    fn parses_meminfo_without_unit_suffix() {
        let info = parse_meminfo(
            "MemTotal:       1024 kB\n\
             MemAvailable:   512 kB\n\
             SwapTotal:      0\n\
             SwapFree:       0\n",
        )
        .expect("meminfo should parse");
        assert_eq!(info.total_bytes, 1_048_576);
        assert_eq!(info.available_bytes, 524_288);
        assert_eq!(info.swap_total_bytes, 0);
        assert_eq!(info.swap_free_bytes, 0);
    }

    #[test]
    fn rejects_meminfo_with_unknown_unit_suffix() {
        let error = parse_meminfo(
            "MemTotal:       1024 blocks\n\
             MemAvailable:   512 kB\n\
             SwapTotal:      0 kB\n\
             SwapFree:       0 kB\n",
        )
        .expect_err("unknown unit suffix should fail");

        assert!(
            matches!(error, SbhError::MountParse { .. }),
            "expected mount-parse error, got: {error:?}"
        );
        assert!(
            error
                .to_string()
                .contains("unsupported meminfo unit in line"),
            "expected unsupported-unit context, got: {error}"
        );
    }
}
