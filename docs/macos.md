# macOS Operations Guide

This guide explains how `sbh` behaves on macOS and what an operator should
expect from APFS, launchd, Full Disk Access, Homebrew-style installs, custom
paths, and safety controls.

The 2026-05-03 Mac disk-pressure incident is documented in
`docs/macos-incident-case-study.md`: a 264 GB
`/private/tmp/frankenterm-trash-20260503-092725` staging directory, a 9.8 GB
Claude `vm_bundles` cache, and about 330 GB of active `/private/tmp/ft-*-target`
build directories that sbh should detect but not delete while builds hold them
open.

## Quick Start

For a user-scoped launchd service:

```bash
sbh install --launchd --scope user
sbh doctor --pal
sbh status
```

`sbh install --auto` chooses launchd on macOS, user scope, detected watched
paths, and the medium ballast preset. Use `--scope system` only when the daemon
must monitor system-wide paths or processes; system scope requires root and
installs a LaunchDaemon.

## Platform Defaults

macOS uses native Application Support paths by default:

| Scope | Config | State, logs, ballast, update cache |
| --- | --- | --- |
| User LaunchAgent | `~/Library/Application Support/sbh/config.toml` | `~/Library/Application Support/sbh/` |
| System LaunchDaemon | `/Library/Application Support/sbh/config.toml` | `/private/var/sbh/` |

Config path precedence is:

1. `--config <PATH>`
2. `SBH_CONFIG`
3. Platform-native user default
4. Platform-native system fallback when no user config exists

On macOS, XDG layout is still supported for operators who already use it. `sbh`
uses XDG paths when `SBH_USE_XDG_PATHS=1`, `XDG_CONFIG_HOME`, or
`XDG_DATA_HOME` is set, or when `~/.config/sbh/config.toml` already exists and
the native Application Support config does not.

## launchd Integration

`sbh` generates launchd plists instead of systemd units on macOS.

| Scope | Plist location | Logs |
| --- | --- | --- |
| User | `~/Library/LaunchAgents/` | `~/Library/Logs/sbh/` |
| System | `/Library/LaunchDaemons/` | `/var/log/sbh/` |

The generated plist sets:

- `RunAtLoad=true`, so the daemon starts on login or boot.
- `KeepAlive` with `SuccessfulExit=false`, so crashes restart but clean exits
  stay stopped.
- `ThrottleInterval=10`, to avoid rapid restart loops.
- `Nice=19` and `LowPriorityIO=true`, so cleanup work yields to foreground
  user work and builds.

Use these commands for service health:

```bash
sbh service --launchd --scope user status
sbh service --launchd --scope user restart
sbh doctor --pal
```

`sbh doctor --pal` checks launchd status and prints the exact remediation when
launchctl cannot load or inspect the service.

## APFS Capacity

APFS space accounting is different from fixed Linux filesystems. Multiple
volumes often share one APFS container, so a volume can report a logical size
that is not the actual physical ceiling for disk pressure.

On macOS, `sbh` combines:

- `statfs` for live filesystem capacity.
- `diskutil apfs list -plist` for APFS container and volume metadata.
- Foundation important-usage capacity when available.
- `tmutil listlocalsnapshots` for local Time Machine snapshot inventory.

Status JSON exposes APFS metadata under the mount payload:

```json
{
  "platform": {
    "darwin": {
      "apfs": {
        "container_id": "/dev/disk3",
        "container_total_bytes": 1000,
        "container_available_bytes": 250,
        "volume_role": "Data",
        "purgeable_bytes": 32,
        "local_snapshot_bytes": 64,
        "free_excludes_purgeable": true
      }
    }
  }
}
```

The important invariant is `free_excludes_purgeable: true`. `sbh` reports
purgeable storage, but it does not count purgeable bytes as free space when
making pressure decisions. Purgeable storage is controlled by macOS and may not
be immediately reclaimable when a build or daemon needs space right now.

## Purgeable Space

Finder and System Settings may show "available" space that includes purgeable
content. `sbh` separates that from real free space because cleanup decisions
must be conservative.

When purgeable APFS storage is present, `sbh status` prints a separate
Purgeable Storage section and JSON includes `purgeable_bytes`. Treat it as
diagnostic context, not as guaranteed emergency headroom.

## Local Time Machine Snapshots

Local snapshots can retain blocks after files are deleted. This matters during
incidents: `sbh ballast release` can unlink ballast files immediately, but APFS
may not show the recovered bytes in `df`, Finder, or status output until the
snapshot retaining those blocks is thinned or expires.

Dry-run the snapshot thinning plan:

```bash
sbh clean --thin-local-snapshots --dry-run
```

Execute it for the root mount:

```bash
sudo sbh clean --thin-local-snapshots --yes
```

Target a specific APFS mount:

```bash
sudo sbh clean --thin-local-snapshots --yes --local-snapshot-mount /System/Volumes/Data
```

The underlying command is:

```bash
sudo tmutil thinlocalsnapshots <mount> 9999999999999999 4
```

Thinning can take 30 seconds or longer. The exact bytes released are controlled
by Time Machine and APFS, not by `sbh`.

## Ballast On APFS

The default user ballast path is:

```text
~/Library/Application Support/sbh/ballast.bin
```

The default system ballast path is:

```text
/private/var/sbh/ballast.bin
```

`[paths].ballast_dir` can move the ballast pool to another volume. Put ballast
on the same volume that needs emergency headroom. A ballast pool on the wrong
mount does not help the full mount.

When APFS local snapshots are present, released ballast blocks may remain
retained by snapshots. If `sbh ballast release` warns about snapshots, thin
snapshots and then re-check:

```bash
sbh status
df -h /
```

## Full Disk Access

macOS Transparency, Consent, and Control protects user data under locations
such as Mail, Messages, and parts of `~/Library`. `sbh` probes Full Disk Access
by attempting to read the Mail Envelope Index under:

```text
~/Library/Mail/V*/MailData/Envelope Index
```

Check the current grant:

```bash
sbh doctor --pal
```

When access is missing, doctor output includes a `macos_full_disk_access`
follow-up and points to `docs/macos-full-disk-access.md`. Grant access before
relying on macOS cleanup scans that need protected user data.

After changing Full Disk Access, restart the launchd service or rerun the
command that needs access:

```bash
sbh service --launchd --scope user restart
sbh doctor --pal
```

Development builds need their own Full Disk Access entry if they run from a
different path than the installed `sbh` binary.

## Homebrew And Install Paths

Apple Silicon Homebrew normally installs under:

```text
/opt/homebrew/bin/sbh
```

Intel Homebrew normally installs under:

```text
/usr/local/bin/sbh
```

Bootstrap and repair checks also inspect common Homebrew sbin paths and Cellar
layouts under `/opt/homebrew` and `/usr/local`. That lets `sbh bootstrap` detect
stale binaries, stale launchd plists, and legacy footprints after a move
between manual and Homebrew-style locations.

The tap skeleton lives in:

```text
packaging/homebrew/Formula/sbh.rb
```

Tagged releases copy that file into
`Dicklesworthstone/homebrew-sbh/Formula/sbh.rb`, replace the placeholder SHA-256
values with the per-architecture checksums for the released
`sbh-v<version>-<target>.tar.xz` archives, push an `update-sbh-v*` branch to the
tap, and open or update a formula update PR. The release workflow requires a
`HOMEBREW_TAP_TOKEN` secret with write access to the tap repository. The formula
installs the prebuilt `sbh` binary, runs
`sbh setup --verify --bin-dir <keg>/bin` as a post-install sanity check, defines
a `brew services` daemon entry, and prints the Full Disk Access reminder in its
caveats.

Until the external tap is published, a manually installed or from-source binary
can still live in one of the standard Homebrew prefixes as long as the launchd
plist points at the actual binary path.

## Code Signing And Hardened Runtime

macOS CI and release builds sign `sbh` with Hardened Runtime enabled. The
entitlements file is intentionally minimal:

```text
.github/macos/sbh.entitlements.plist
```

That file contains an empty entitlement dictionary. `sbh` does not need JIT,
library-validation bypasses, camera, microphone, or network-server entitlements.
Pull-request CI signs the release-style binary ad hoc with:

```bash
codesign --force --sign - --options runtime --timestamp=none \
  --entitlements .github/macos/sbh.entitlements.plist target/release/sbh
```

The ad-hoc identity used in PR CI is only for build validation. Tagged macOS
releases import the Developer ID Application certificate from Actions secrets
into a temporary keychain and sign the same binary with:

```bash
codesign --force --sign "${APPLE_DEVELOPER_ID_IDENTITY}" --options runtime \
  --timestamp --entitlements .github/macos/sbh.entitlements.plist target/release/sbh
```

CI runs the ad-hoc signing check on both `push` and `pull_request` events so
PRs exercise the macOS codesign path without requiring a Developer ID
certificate. Tagged releases fail before packaging if the Developer ID
certificate secrets are absent or the resulting binary is not signed by a
`Developer ID Application` authority.

Tagged macOS releases also run an explicit notarization phase. Apple accepts
notary uploads as ZIP archives, disk images, or signed flat packages, while the
existing release artifact remains `sbh-{tag}-{target}.tar.xz`; the workflow
therefore creates a temporary ZIP around the signed `sbh` binary for Apple's
scanner and keeps the tarball naming contract unchanged.

The release workflow uses these GitHub secrets:

```text
APPLE_DEVELOPER_ID_CERTIFICATE_P12_BASE64
APPLE_DEVELOPER_ID_CERTIFICATE_PASSWORD
APPLE_DEVELOPER_ID_IDENTITY
APPLE_ID
APPLE_TEAM_ID
APPLE_APP_SPECIFIC_PASSWORD
HOMEBREW_TAP_TOKEN
```

Apple Developer Program enrollment is confirmed for this project. Use the
already-enrolled Apple Developer account or team that owns the Developer ID
Application certificate and notarization credentials. The repository workflow is
intentionally team-agnostic: Organization and Individual memberships both use
the same secret names, and the selected Team ID is represented by
`APPLE_TEAM_ID` rather than by branching release logic.

Developer ID certificate setup is intentionally outside the repository because
it handles private key material:

1. Create a `Developer ID Application` certificate in the Apple Developer
   portal for the selected account or team.
2. Install the certificate in Keychain Access on a trusted Mac and verify that
   `security find-identity -v -p codesigning` lists a `Developer ID
   Application` identity for the selected Team ID.
3. Export that identity, including the private key, as an encrypted P12 file.
   Keep the P12 outside the repository and protect it with a unique password.
4. Set the release secrets from stdin so the values do not appear in shell
   history:

   ```bash
   base64 < "$P12_PATH" | gh secret set APPLE_DEVELOPER_ID_CERTIFICATE_P12_BASE64 \
     -R Dicklesworthstone/storage_ballast_helper --body-file -
   printf '%s' "$P12_PASSWORD" | gh secret set APPLE_DEVELOPER_ID_CERTIFICATE_PASSWORD \
     -R Dicklesworthstone/storage_ballast_helper --body-file -
   printf '%s' "$DEVELOPER_ID_IDENTITY" | gh secret set APPLE_DEVELOPER_ID_IDENTITY \
     -R Dicklesworthstone/storage_ballast_helper --body-file -
   printf '%s' "$APPLE_ID" | gh secret set APPLE_ID \
     -R Dicklesworthstone/storage_ballast_helper --body-file -
   printf '%s' "$APPLE_TEAM_ID" | gh secret set APPLE_TEAM_ID \
     -R Dicklesworthstone/storage_ballast_helper --body-file -
   printf '%s' "$APPLE_APP_SPECIFIC_PASSWORD" | gh secret set APPLE_APP_SPECIFIC_PASSWORD \
     -R Dicklesworthstone/storage_ballast_helper --body-file -
   printf '%s' "$HOMEBREW_TAP_TOKEN" | gh secret set HOMEBREW_TAP_TOKEN \
     -R Dicklesworthstone/storage_ballast_helper --body-file -
   gh secret list -R Dicklesworthstone/storage_ballast_helper
   ```

Rotate the Developer ID certificate and app-specific password every 12 months,
or immediately after any maintainer, runner, or secret exposure incident. During
rotation, create and store the replacement secrets first, run the
`Developer ID Certificate Expiration` workflow manually, then publish the next
tagged release only after the release workflow signs, notarizes, and verifies
the binary with the new identity.

If any secret is missing, the macOS release job fails before packaging. The P12
secret is the base64-encoded Developer ID Application certificate plus private
key exported from Keychain Access, and `APPLE_DEVELOPER_ID_IDENTITY` is the full
codesigning identity string, for example `Developer ID Application: Example LLC
(TEAMID)`. When all credentials are present, the workflow imports the P12 into a
temporary keychain, signs with Hardened Runtime, verifies that the binary was
signed by a `Developer ID Application` authority, then submits the temporary ZIP
with `xcrun notarytool submit`, extracts the submission id, polls `xcrun
notarytool info` every 30 seconds for up to 30 minutes, and downloads `xcrun
notarytool log` output on both success and failure. `Invalid`, `Rejected`, and
timeout states fail the release with the notary log printed into the Actions
output.

The `Developer ID Certificate Expiration` workflow runs nightly and on manual
dispatch. It decodes `APPLE_DEVELOPER_ID_CERTIFICATE_P12_BASE64`, extracts the
leaf certificate with `openssl pkcs12`, reports the `notAfter` timestamp, fails
if the certificate is already expired, and emits a GitHub Actions warning when
the certificate expires within 30 days. Until the Developer ID certificate
secrets are configured, the workflow emits a warning that certificate expiration monitoring is inactive and exits successfully.

## Release Readiness Diagnostics

Run this before cutting a signed macOS release:

```bash
sbh doctor --release
```

The release doctor does not print secret values. It checks only readiness
signals:

1. `security find-identity -v -p codesigning` must list a `Developer ID
   Application` identity.
2. `xcrun notarytool history --keychain-profile sbh-notary --output-format json`
   must authenticate successfully with the configured keychain profile.
3. `gh secret list -R Dicklesworthstone/storage_ballast_helper --json name` must
   report every release secret used by the GitHub Actions workflow, including
   `HOMEBREW_TAP_TOKEN`.

For automation or handoff checks, use:

```bash
sbh doctor --release --json
```

Treat any `FAIL` result as a release blocker. The command intentionally reports
missing local signing identity, missing notary profile, and missing GitHub
Actions secrets as explicit diagnostics so the external Apple/GitHub credential
setup can be finished without inspecting workflow internals.

The release doctor also prints a non-secret credential setup plan. The plan
uses placeholder environment variables such as `$P12_PATH`, `$P12_PASSWORD`,
`$APPLE_ID`, `$APPLE_TEAM_ID`, `$APPLE_APP_SPECIFIC_PASSWORD`, and
`$HOMEBREW_TAP_TOKEN`; it never prints secret values. The GitHub secret commands
use `--body-file -` so secret material can be piped from stdin instead of being
stored in shell history. After completing the plan, rerun:

```bash
sbh doctor --release --json
```

The JSON `setup_steps` field is stable enough for handoff automation that wants
to display the same Developer ID, notary, Homebrew token, and final recheck
commands without scraping this document.

The current CLI tarball flow does not staple a ticket because `stapler` supports
app bundles, disk images, and signed flat packages rather than the `.tar.xz`
artifact. Gatekeeper can still find the online notary ticket for the signed
binary. A future `.pkg` or `.dmg` distribution path should staple and validate
that package after notarization.

## Self-Update Verification

`sbh update` verifies the downloaded archive checksum before extraction. On
macOS, it also verifies the extracted candidate binary before the atomic replacement step:

1. Execute the candidate with `sbh --version` to catch noexec mounts and dynamic
   linker failures.
2. Run `codesign --verify --strict --verbose=2 <candidate>` to reject unsigned
   or malformed signatures.
3. Run `spctl -a -t execute -vv <candidate>` so Gatekeeper accepts the signed
   and notarized binary.
4. Rename the verified candidate into place atomically and roll back to the
   previous binary if any pre-swap check or rename fails.

The unsafe `sbh update --no-verify` escape hatch bypasses checksum, Sigstore, codesign, and Gatekeeper checks. Use it only for deliberate recovery from a trusted local bundle.

## Watched Paths

The install wizard auto-detects watched paths from:

- `/data/projects`
- platform temp directories, including `/tmp`
- the user's home directory when available

The static config defaults also include common Linux-style roots such as
`/data/projects`, `/tmp`, `/data/tmp`, `/var/tmp`, `/home`, and `/root`.
Review generated config on macOS and keep watched roots intentionally narrow.

Set custom roots in config:

```toml
[scanner]
root_paths = [
  "/Users/me/Projects",
  "/private/tmp",
]
```

Use protections for durable data inside broad roots:

```bash
sbh protect /Users/me/Projects/important-repo
```

Or use config globs:

```toml
[scanner]
protected_paths = [
  "/Users/me/Projects/client-*",
  "/Users/me/Library/Mobile Documents/com~apple~CloudDocs/Client Records/*",
]
```

Sample configs for common Mac scenarios live under `docs/configs/`:

- `docs/configs/developer-mac.toml` for source-heavy developer laptops.
- `docs/configs/creative-mac.toml` for media-heavy Macs where most user data is
  sacred and dry-run-first.
- `docs/configs/shared-mac-launchdaemon.toml` for system-scope shared Macs.

See `docs/cleanup-rules-macos.md` for the exhaustive macOS cleanup contract and
`docs/sacred-paths.md` for the built-in sacred catalog and the reasoning for
every protected pattern.

## Migrating From Visual Cleanup Tools

If you already use CleanMyMac, OmniDiskSweeper, DaisyDisk, or GrandPerspective,
keep those tools for visual review and personal-file decisions. Add `sbh` for
continuous pressure monitoring, ballast headroom, dry-run artifact cleanup,
protected-path vetoes, APFS/Time Machine snapshot warnings, and audit output
that can run under launchd.

See `docs/migrating-from-other-tools.md` for the migration checklist and the
side-by-side comparison.

## macOS Cleanup Safety Model

macOS cleanup rules are specific and conservative. This section is a summary;
the exhaustive operator trust document is `docs/cleanup-rules-macos.md`.

- Xcode DerivedData cleanup targets immediate children of
  `~/Library/Developer/Xcode/DerivedData/`, not the root as one broad delete.
- Electron cleanup targets regenerated cache shapes such as `Cache`,
  `Code Cache`, `GPUCache`, `IndexedDB`, `Service Worker/CacheStorage`, and
  `vm_bundles`.
- `/private/tmp/*-target`, `*_target`, and `target_*` are treated as likely
  build artifacts only after age and safety checks.
- User-named trash directories under temporary roots are ambiguous and require
  review unless another hard veto keeps them.
- Time Machine snapshot thinning uses `tmutil`; it is not path deletion.
- `~/.Trash` and iCloud Drive trash are report-only. `sbh` does not auto-empty
  user trash.

Every cleanup candidate still passes hard vetoes: sacred-overlap checks,
`.sbh-protect` markers, parent checks, active-reference evidence where visible,
minimum age, and source-root checks.

User-scope macOS runs can have incomplete visibility into other users'
processes. When active-reference checks are incomplete, `sbh` surfaces that
reason in scan output instead of silently pretending visibility is complete.

## Security Model

`sbh` separates observation from mutation:

- `sbh status`, `sbh check`, `sbh scan`, `sbh doctor --pal`, and dry-runs are
  non-destructive.
- `sbh clean --dry-run` prints the plan without deletion.
- `sbh clean --yes`, daemon cleanup in enforcing policy modes, and ballast
  release are mutating operations.
- Protected paths and sacred paths are hard vetoes, not scoring hints.
- Purgeable space is reported separately and excluded from free-space pressure
  decisions.
- launchd runs with low scheduling and IO priority so it yields to foreground
  work.

For incident response, prefer this sequence:

```bash
sbh doctor --pal
sbh status --json
sbh clean --thin-local-snapshots --dry-run
sbh scan /private/tmp --top 20
sbh clean /private/tmp --dry-run
```

Only add `--yes` after the dry-run output names exactly the paths you expect.

## Troubleshooting

| Symptom | Check |
| --- | --- |
| `sbh doctor --pal` reports missing Full Disk Access | Follow `docs/macos-full-disk-access.md`, restart launchd, rerun doctor. |
| `df` does not show space after ballast release | Check local snapshots and run snapshot thinning. |
| launchd says the service is not loaded | Run `sbh service --launchd --scope user status`, then use `docs/launchd-troubleshooting.md` for `launchctl print` interpretation and recovery. |
| Status shows purgeable bytes but pressure remains high | Treat purgeable as informational; free real space or thin snapshots. |
| Cleanup cannot see active references for some processes | Use system scope when system-wide process visibility is required. |
