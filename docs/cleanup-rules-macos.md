# macOS Cleanup Rules

This is the operator trust document for macOS cleanup. It lists every built-in
macOS cleanup rule, what `sbh` may do with it, and which safety gates must pass
first.

`sbh` does not treat a high cleanup score as permission to delete. Protected
paths, sacred-path overlaps, `.sbh-protect` markers, parent checks, source-root
checks, active-reference evidence, and minimum age checks are hard gates.

## Safety Vocabulary

| Term | Meaning |
| --- | --- |
| `Definite` | The path shape is known rebuildable or downloadable state. It can be removed automatically after all hard gates pass. |
| `Likely` | The path shape usually describes rebuildable cache or build output. It can be removed automatically only after stronger context checks pass. |
| `Unclear` | The path name is ambiguous. `sbh` surfaces it for review instead of silently deciding. |
| `ReportOnly` | `sbh` reports the reclaim opportunity but does not delete the path. |
| `Sacred` | `sbh` refuses deletion. This is user data, credentials, source state, or creative/project state. |

| Reclaim command | Behavior |
| --- | --- |
| `RemoveTree` | Remove a matched directory subtree after hard gates pass. |
| `RemoveMatchingFiles` | Remove only files matching the rule after hard gates pass. |
| `ThinLocalSnapshots` | Ask Time Machine/APFS to thin local snapshots; this is not path deletion. |
| `PromptBeforeRemove` | Require explicit review before removal. |
| `ReportOnly` | Report only. |
| `Refuse` | Hard refusal. |

Every destructive path rule requires parent safety checks and sacred-overlap
checks. Rules marked with an active-file check also verify that visible
processes do not currently hold the candidate or a descendant open. User-scope
macOS runs may not see other users' processes; when visibility is incomplete,
`sbh` reports that limitation instead of pretending the check is complete.

## Automatic Cleanup Candidates

These rules can delete only after the path matches exactly, the age threshold is
met, active-reference checks pass where required, the parent is safe, and no
sacred path or marker is inside the candidate.

| Rule | Pattern | Age | Active-file check | Command | Confidence | What it means |
| --- | --- | --- | --- | --- | --- | --- |
| `xcode-derived-data` | `~/Library/Developer/Xcode/DerivedData/*` | 24 hours | Required | `RemoveTree` | `Definite` | Per-project Xcode build products. The DerivedData root itself is not one broad delete target. |
| `core-simulator-caches` | `~/Library/Developer/CoreSimulator/Caches/*` | 24 hours | Required | `RemoveTree` | `Definite` | CoreSimulator cache entries. Simulator `Devices` state is not covered by this rule. |
| `electron-cache` | `~/Library/Application Support/*/Cache/*` | 1 hour | Required | `RemoveTree` | `Likely` | Electron and Chromium-style cache children. |
| `electron-cache-root` | `~/Library/Application Support/*/Cache` | 1 hour | Required | `RemoveTree` | `Likely` | Electron and Chromium-style cache roots. |
| `electron-service-worker-cache` | `~/Library/Application Support/*/Service Worker/CacheStorage/*` | 1 hour | Required | `RemoveTree` | `Likely` | Service worker cache children. |
| `electron-service-worker-cache-root` | `~/Library/Application Support/*/Service Worker/CacheStorage` | 1 hour | Required | `RemoveTree` | `Likely` | Service worker cache roots. |
| `electron-code-cache` | `~/Library/Application Support/*/Code Cache/*` | 1 hour | Required | `RemoveTree` | `Likely` | Electron code cache children. |
| `electron-code-cache-root` | `~/Library/Application Support/*/Code Cache` | 1 hour | Required | `RemoveTree` | `Likely` | Electron code cache roots. |
| `electron-gpu-cache` | `~/Library/Application Support/*/GPUCache/*` | 1 hour | Required | `RemoveTree` | `Likely` | Electron GPU cache children. |
| `electron-gpu-cache-root` | `~/Library/Application Support/*/GPUCache` | 1 hour | Required | `RemoveTree` | `Likely` | Electron GPU cache roots. |
| `electron-indexed-db` | `~/Library/Application Support/*/IndexedDB/*` | 1 hour | Required | `RemoveTree` | `Likely` | IndexedDB cache entries under application support. Browser profile roots remain sacred. |
| `electron-indexed-db-root` | `~/Library/Application Support/*/IndexedDB` | 1 hour | Required | `RemoveTree` | `Likely` | IndexedDB cache roots under application support. |
| `electron-vm-bundles` | `~/Library/Application Support/*/vm_bundles/*` | 24 hours | Required | `RemoveTree` | `Likely` | Rebuildable VM bundle cache children. |
| `electron-vm-bundles-root` | `~/Library/Application Support/*/vm_bundles` | 24 hours | Required | `RemoveTree` | `Likely` | Rebuildable VM bundle cache roots. |
| `tmp-dash-target` | `/private/tmp/*-target` | 24 hours | Required | `RemoveTree` | `Likely` | Temporary build output with a `-target` suffix. |
| `tmp-underscore-target` | `/private/tmp/*_target` | 24 hours | Required | `RemoveTree` | `Likely` | Temporary build output with an `_target` suffix. |
| `tmp-target-underscore-prefix` | `/private/tmp/target_*` | 24 hours | Required | `RemoveTree` | `Likely` | Temporary build output with a `target_` prefix. |
| `release-work-buildroot` | `~/release-work/*[-_]buildroot` | 7 days | Required | `RemoveTree` | `Likely` | Stale release staging buildroots. Embedded repos, Beads state, databases, credentials, or markers block cleanup. |
| `user-logs` | `~/Library/Logs/*` | 7 days | Required | `RemoveMatchingFiles` | `Likely` | User log files. This rule does not imply arbitrary `~/Library` data is disposable. |
| `ipsw-software-updates` | `~/Library/iTunes/iPhone Software Updates/*.ipsw` | 30 days | Not required | `RemoveMatchingFiles` | `Definite` | Downloaded iPhone firmware update files. Only `.ipsw` files in that update directory match. |

`/tmp` and `/private/tmp` are treated as aliases for matching. That does not
make every temporary directory disposable: only the listed shapes are candidates,
and every candidate still passes the hard gates.

## Review-Only Candidates

These rules intentionally avoid silent deletion because the path name is too
ambiguous.

| Rule | Pattern | Command | Confidence | Behavior |
| --- | --- | --- | --- | --- |
| `user-named-trash-exact` | `/private/tmp/trash` | `PromptBeforeRemove` | `Unclear` | Report for review. |
| `user-named-trashed-exact` | `/private/tmp/trashed` | `PromptBeforeRemove` | `Unclear` | Report for review. |
| `user-named-trash` | `/private/tmp/*-trash-*` | `PromptBeforeRemove` | `Unclear` | Report for review. |

If one of these directories contains `.git`, `.beads`, databases, credentials,
or a `.sbh-protect` marker, the sacred-path system blocks it even before human
review.

## Report-Only Rules

These rules exist to make space pressure visible without deleting the matching
paths.

| Rule | Pattern | Command | Confidence | Behavior |
| --- | --- | --- | --- | --- |
| `home-trash-report` | `~/.Trash/*` | `ReportOnly` | `ReportOnly` | Shows local Trash usage. `sbh` does not empty Trash automatically. |
| `icloud-trash-report` | `~/Library/Mobile Documents/com~apple~CloudDocs/.Trash/*` | `ReportOnly` | `ReportOnly` | Shows iCloud Drive Trash usage. `sbh` does not turn local cleanup into a cloud-side delete. |
| `spotlight-index-report` | `/.Spotlight-V100` | `ReportOnly` | `ReportOnly` | Shows Spotlight index size. Rebuilding Spotlight can be disruptive, so `sbh` does not casually remove it. |

## Time Machine Snapshots

| Rule | Target | Command | Confidence | Behavior |
| --- | --- | --- | --- | --- |
| `time-machine-local-snapshots` | `/` | `ThinLocalSnapshots` | `Likely` | Runs the Time Machine/APFS thinning path. This asks the system to reclaim snapshot space and does not delete a user path. |

Snapshot thinning matters because APFS snapshots can retain bytes after files
or ballast are unlinked. `sbh` treats this as a system reclaim operation, not as
permission to remove user files.

## Sacred macOS Cleanup Catalog Entries

These entries are embedded in the cleanup catalog as refusal rules:

| Rule | Pattern | Command | Confidence | Why |
| --- | --- | --- | --- | --- |
| `photos-library-sacred` | `~/Pictures/Photos Library.photoslibrary` | `Refuse` | `Sacred` | Photos libraries contain originals, edits, albums, metadata, and import history. |
| `mail-library-sacred` | `~/Library/Mail/*` | `Refuse` | `Sacred` | Mail stores messages, local mailboxes, attachments, indexes, and account state. |
| `messages-library-sacred` | `~/Library/Messages/*` | `Refuse` | `Sacred` | Messages stores conversations, attachments, and local chat databases. |
| `final-cut-library-sacred` | `~/Movies/*.fcpbundle` | `Refuse` | `Sacred` | Final Cut Pro bundles are creative project state and media organization. |

`docs/sacred-paths.md` is the broader sacred-path contract. It also protects
Git metadata, Beads state, SQLite and database files, credentials, iCloud data,
Photos/iMovie/Logic/GarageBand/Lightroom/Capture One projects, browser profiles,
tax and finance documents, and user-configured `scanner.protected_paths`.

## What `sbh` Does Not Infer

`sbh` does not infer that these are safe merely because they are large or live
under a cache-like parent:

- Photos libraries, Mail, Messages, iCloud Drive documents, Final Cut bundles,
  iMovie libraries, Logic or GarageBand work, Lightroom or Capture One catalogs.
- Browser profile roots, saved sessions, credentials, history, extensions, or
  preferences.
- Arbitrary `~/Library/Application Support` directories.
- CoreSimulator `Devices` state.
- Arbitrary `~/Library/Developer` contents outside the listed cache shapes.
- Arbitrary `/private/tmp` trees that do not match a cleanup rule.
- `~/.Trash` or iCloud Drive Trash contents.
- Anything containing `.git`, `.beads`, `beads.db`, SQLite/database files,
  credentials, or `.sbh-protect`.

## Operator Controls

Use a marker for durable data inside broad scan roots:

```bash
sbh protect /Users/me/Projects/important-repo
```

Use config globs for path families:

```toml
[scanner]
protected_paths = [
  "/Users/me/Projects/client-*",
  "/Users/me/Library/Mobile Documents/com~apple~CloudDocs/Client Records/*",
]
```

Both mechanisms are hard vetoes. If a cleanup candidate is the protected path,
inside the protected path, or a parent that would contain it, `sbh` skips the
candidate.

## How To Verify Before Trusting It

Start with non-destructive commands:

```bash
sbh doctor --pal
sbh status --json
sbh status --sacred
sbh scan /private/tmp --json
sbh scan /private/tmp --show-protected
sbh protect --list
sbh clean /private/tmp --dry-run
```

Read `candidates`, `protected_paths`, sacred-overlap reasons, active-reference
visibility, and dry-run plans before enabling daemon cleanup in enforcing policy
modes.
