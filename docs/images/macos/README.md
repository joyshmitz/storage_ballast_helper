# macOS Image Manifest

This directory holds optional macOS screenshots used by operator docs. The text
walkthrough is authoritative; screenshots are supporting evidence only and must
never be required to complete a Full Disk Access grant.

Do not generate or mock screenshots. Only commit images captured from a real,
current macOS System Settings UI.

## Full Disk Access Slots

| File | Required alt text | Status |
| ---- | ----------------- | ------ |
| `full-disk-access-privacy-security.png` | System Settings with Privacy & Security selected in the sidebar. | Not captured |
| `full-disk-access-pane.png` | Full Disk Access pane open in System Settings. | Not captured |
| `full-disk-access-sbh-enabled.png` | Full Disk Access list showing `sbh` present and enabled. | Not captured |

Do not link a screenshot from `docs/macos-full-disk-access.md` until the file
exists and the alt text above is present next to the image reference.

## Capture Rules

- Capture real System Settings windows on the current public macOS release.
- Redact names, Apple IDs, hostnames, device names, and unrelated application
  entries before committing.
- Crop to the System Settings window or pane; avoid desktop backgrounds and
  unrelated windows.
- Keep filenames stable so docs and tests can track staleness.
- Prefer text changes over screenshot churn when the UI flow is unchanged.

## Annual Refresh Policy

Review this manifest once per macOS major release and within 30 days of the
public release date. During the review:

1. Run through the Full Disk Access flow from `docs/macos-full-disk-access.md`.
2. Confirm the filenames and required alt text above still match the UI.
3. Refresh screenshots only if the System Settings flow or visible labels
   changed.
4. Update `docs/macos-full-disk-access.md` if any step changed.
5. File a follow-up bead if screenshots are stale but no current macOS machine is
   available to recapture them.
