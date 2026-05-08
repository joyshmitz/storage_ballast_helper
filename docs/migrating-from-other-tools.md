# Migrating From Other Mac Cleanup Tools

This guide is for Mac users who already use tools such as CleanMyMac,
OmniDiskSweeper, DaisyDisk, or GrandPerspective and want to add `sbh` without
losing the strengths of those tools.

`sbh` is not a visual disk browser or a general Mac maintenance suite. It is a
continuous disk-pressure guard for developer and agent workloads: it watches the
machine, predicts pressure, releases ballast, scores rebuildable artifacts, and
records why every cleanup decision was or was not allowed.

## When To Keep Existing Tools

Keep a visual or manual cleanup tool when you need to inspect personal files,
compare large media libraries, remove old downloads by sight, or decide whether
a document is still useful. Those are human ownership decisions.

Use `sbh` for the pressure paths that should not depend on a human noticing the
problem in time:

- Build artifacts that can grow while agents are running.
- Burst pressure from parallel compiler, test, or packaging jobs.
- Emergency headroom from ballast files on the affected volume.
- Repeatable dry-run, scan, status, and audit output for automation.
- Protected paths that must remain hard vetoes even when disk pressure is high;
  in `sbh`, protected paths are enforced as policy, not advisory labels.

## Comparison

| Tool | Strength | What `sbh` Adds |
| --- | --- | --- |
| CleanMyMac | Guided Mac maintenance, app cleanup, and user-approved cleanup categories. | A service-grade daemon, ballast release, pressure forecasts, artifact scoring, protected-path vetoes, and JSON/audit logs for developer workloads. |
| OmniDiskSweeper | Fast manual size browsing by folder. | Continuous monitoring, non-interactive scans, hard safety gates, and repeatable policy decisions. |
| DaisyDisk | Visual disk map for finding large user-visible folders. | Daemon behavior, APFS/Time Machine snapshot warnings, dry-run cleanup plans, and machine-parseable status. |
| GrandPerspective | Treemap inspection for manual exploration. | Automated pressure response, ballast pools, active-reference checks, and explainable cleanup evidence. |

The tools can coexist. A common pattern is to keep DaisyDisk or
GrandPerspective for occasional visual reviews, keep OmniDiskSweeper for quick
manual folder inspection, and run `sbh` continuously for build-artifact pressure
and emergency headroom.

## What `sbh` Does That They Usually Do Not

- Runs as a launchd service and keeps watching after the initial scan.
- Predicts pressure with filesystem samples instead of waiting for a manual
  inspection.
- Pre-allocates ballast on the volume that needs emergency headroom and releases
  it quickly during incidents.
- Scores candidates with location, name, age, size, and structure factors, then
  applies hard vetoes before any deletion path.
- Honors `.sbh-protect` markers and `scanner.protected_paths` as hard stops.
- Checks active file references where the platform can expose them and reports
  incomplete visibility instead of silently assuming safety.
- Separates APFS purgeable space from real free space.
- Warns when Time Machine local snapshots may retain bytes after ballast or
  cleanup operations.
- Emits JSON and log evidence for `status`, `scan`, `clean --dry-run`, `stats`,
  `blame`, and `explain`.

## What They Do That `sbh` Does Not

- Show a visual treemap or interactive disk map.
- Decide whether personal media, documents, downloads, or archives are still
  valuable.
- Uninstall applications or remove preference files as an app maintenance suite.
- Empty `~/.Trash` or iCloud Drive trash automatically.
- Remove Photos, Mail, Messages, Final Cut, iMovie, Logic, GarageBand,
  Lightroom, Capture One, browser profile, credential, database, Git, or Beads
  data.
- Treat all of `~/Library`, `/private/tmp`, or `~/Downloads` as disposable.

Those limits are intentional. `sbh` is conservative because the expensive error
is deleting source, credentials, creative work, or user data.

## Migration Steps

1. Install `sbh` and let it choose the platform defaults:

```bash
sbh install --auto
sbh doctor --pal
```

2. Review the generated config and watched paths:

```bash
sbh config show
```

3. Protect durable projects, client folders, and any broad watched roots that
   contain source or creative work:

```bash
sbh protect /Users/me/Projects/important-repo
sbh protect "/Users/me/Library/Mobile Documents/com~apple~CloudDocs/Client Records"
```

4. Run non-destructive visibility checks before enabling cleanup trust:

```bash
sbh status
sbh status --sacred
sbh scan /Users/me/Projects --show-protected
sbh clean /Users/me/Projects --dry-run
```

5. Keep using visual tools for one-off user decisions. Use `sbh` for the
   repeatable pressure lane:

```bash
sbh status --json
sbh stats --window 24h
sbh blame --json
sbh explain --id <decision-id>
```

## Recommended Mac Setup

For most developer Macs, use a user LaunchAgent and intentionally narrow scan
roots:

```toml
[scanner]
root_paths = [
  "/Users/me/Projects",
  "/private/tmp",
]

protected_paths = [
  "/Users/me/Projects/client-*",
  "/Users/me/Projects/*/.git",
]
```

Use system scope only when `sbh` must inspect all users' processes or
system-wide paths:

```bash
sudo sbh install --launchd --scope system --auto
sudo sbh doctor --pal
```

After setup, keep the trust loop dry-run first:

```bash
sbh scan /private/tmp --top 20
sbh clean /private/tmp --dry-run
```

Only add `--yes` when the output names exactly the paths you expect.

## Incident Checklist

When the Mac is already under pressure:

```bash
sbh doctor --pal
sbh status --json
sbh clean --thin-local-snapshots --dry-run
sbh ballast status
sbh scan /private/tmp --top 20
```

Then choose the least risky action:

- Thin Time Machine local snapshots when APFS snapshots are retaining space.
- Release ballast when the affected volume needs immediate headroom.
- Clean only after `clean --dry-run` names rebuildable paths and no sacred or
  protected-path veto appears.
- Use a visual cleanup tool for personal-file decisions outside `sbh`'s artifact
  model.

## Related Docs

- `docs/macos.md` for the full macOS operations guide.
- `docs/cleanup-rules-macos.md` for every built-in macOS cleanup rule.
- `docs/sacred-paths.md` for paths that `sbh` refuses to delete.
- `docs/macos-full-disk-access.md` for Full Disk Access setup.
- `docs/launchd-troubleshooting.md` for launchd recovery.
