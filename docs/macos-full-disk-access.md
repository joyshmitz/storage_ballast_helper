# macOS Full Disk Access

Some macOS locations, including Mail data under `~/Library/Mail`, are protected
by Transparency, Consent, and Control. `sbh` uses a read-only probe against
`~/Library/Mail/V*/MailData/Envelope Index` to tell whether the running binary
has Full Disk Access.

## Grant Access

1. Open System Settings.
2. Open Privacy & Security.
3. Open Full Disk Access.
4. Click the plus button and authenticate if macOS asks.
5. Press Command+Shift+G in the file picker.
6. Enter `~/.local/bin`.
7. Select `sbh`.
8. Turn `sbh` on in the Full Disk Access list.
9. Restart the `sbh` launchd service or rerun the command that needed access.
10. Run `sbh doctor --pal` and confirm `full_disk_access_status` reports
    `granted`.

If you are testing a development build, add that development binary as well.
`sbh doctor --pal` prints the exact running executable path when it detects a
missing grant.

## What The Screens Should Show

- Privacy & Security should be selected in the System Settings sidebar.
- Full Disk Access should be open in the right-hand pane.
- The `sbh` row should be present and toggled on.

Apple's security guide is the upstream visual reference for the current macOS
privacy settings flow: <https://support.apple.com/guide/security/secddd1d86a6/web>.

## Daemon Rechecks

The daemon checks Full Disk Access at startup and then rechecks about every
five minutes. When access is missing, it logs a one-time reminder with the
`sbh doctor --pal` recheck command. When access is granted or becomes granted,
it logs:

```text
macOS Full Disk Access granted for sbh
```

## JSON Automation

When the grant is missing, `sbh doctor --pal --json` includes a `follow_up`
entry with `id = "macos_full_disk_access"`, the docs path, a recheck command,
and the same ordered steps shown in human output.
