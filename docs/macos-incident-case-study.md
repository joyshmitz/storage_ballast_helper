# macOS Incident Case Study: 2026-05-03

One-liner for external copy:

> sbh saved my Mac from the brink: here is exactly what it caught, what it
> would reclaim, and what it would refuse to delete.

On 2026-05-03, the operator's primary Mac reached 147 MB free on a 1.95 TB
internal disk during agent-heavy Rust development. The incident was not one
large mystery file; it was a pileup of generated state, stale staging roots,
and active build output.

## What Filled The Disk

| Path shape | Size | sbh treatment |
| --- | ---: | --- |
| `/private/tmp/frankenterm-trash-20260503-092725` | 264 GB | Report as a high-value review candidate, but keep it if stowaway state such as `.beads/`, `beads.db`, or `.git/` is found. |
| `~/Library/Application Support/Claude/vm_bundles/claudevm.bundle` | 9.8 GB | Classify under regenerated Electron app cache shapes and surface when age/open-file checks allow. |
| `/private/tmp/ft-*-target` | about 330 GB | Detect as build artifact pressure, but keep active target dirs while Cargo or agent workers still hold references. |
| `~/release-work/mcp_agent_mail_rust_buildroot` | 39 GB | Classify as stale release-work buildroot after the seven-day age threshold. |

## The Trust Contract

The important behavior is not "delete every large directory." The important
behavior is to separate reclaimable generated artifacts from expensive work:

- `sbh status --json` should show pressure before the workstation reaches the
  last few hundred MB.
- `sbh scan /private/tmp --top 20` should surface the 264 GB trash staging dir
  and the about 330 GB active target footprint with explainable evidence.
- `sbh clean /private/tmp --dry-run` should show what would be reclaimed before
  mutation.
- Active `/private/tmp/ft-*-target` directories should remain protected while
  build processes still hold files open.
- User-named trash directories containing `.beads/`, `beads.db`, `.git/`,
  `*.sqlite`, or similar state markers should be kept with an explicit
  sacred-overlap reason.

## Why The Mac Port Exists

Before macOS support, this exact machine had an `sbh` binary installed, but the
operational commands returned `SBH-1101 unsupported platform`. The operator had
the tool name in PATH without the protection loop actually running.

The macOS work removes that surprise: install chooses launchd, status uses the
same JSON contract as Linux, scans understand APFS and Mac-specific cache
shapes, and cleanup still goes through the same dry-run, protection, open-file,
and sacred-path vetoes.

Related operator docs:

- `docs/macos.md` for install, launchd, APFS, and operational behavior.
- `docs/cleanup-rules-macos.md` for the full cleanup catalog.
- `docs/sacred-paths.md` for hard keep rules.
- `docs/migrating-from-other-tools.md` for how sbh differs from manual disk
  visualizers and app cleanup tools.
