# Contributing

This repository is a Rust 2024 Cargo workspace for `sbh`. Keep changes narrow,
make platform behavior explicit, and verify both the code path you changed and
the platform assumptions behind it.

## Standard Gates

Use `rch` for CPU-heavy Cargo work in normal agent and maintainer sessions:

```bash
cargo fmt --check
rch exec "cargo check --all-targets"
rch exec "cargo clippy --all-targets -- -D warnings"
rch exec "cargo test --lib"
rch exec "cargo test --bin sbh"
rch exec "cargo test --test integration_tests"
```

For the full repository gate, prefer the maintained runbook:

```bash
./scripts/quality-gate.sh
```

CI runs Cargo directly because `rch` is not available inside GitHub-hosted
runners. Local human-only validation may also use
`./scripts/quality-gate.sh --local` when the point is to exercise host-specific
macOS APIs.

## macOS Local Validation

Use this lane when a change touches launchd, codesigning, Xcode toolchain
detection, `Library/*` paths, `/private/tmp`, APFS behavior, or macOS PAL
implementations.

```bash
sw_vers
uname -m
xcode-select -p
xcrun --find cc
xcrun --find ar
cc --version
```

Then run the focused macOS test slice plus the standard gates:

```bash
cargo fmt --check
rch exec "cargo test --test integration_tests mac_ -- --nocapture"
rch exec "cargo check --all-targets"
rch exec "cargo clippy --all-targets -- -D warnings"
```

For host-only smoke checks that cannot be exercised through `rch`, run them on
a macOS host or rely on the `macos-platform` CI job. Examples include
`launchctl`, `codesign`, and real Apple developer-tool discovery.

## Linux Validation From a Mac

There are two levels of Linux validation from a Mac.

Cross-compile checks catch many cfg and dependency problems:

```bash
rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
rch exec "cargo check --target x86_64-unknown-linux-gnu --all-targets"
rch exec "cargo check --target aarch64-unknown-linux-gnu --all-targets"
```

Executable Linux tests need a Linux runtime. Use a Linux VM when validating
`/proc`, systemd, Linux permission semantics, Linux mount behavior, or anything
that depends on real Linux syscalls.

Inside the VM, run:

```bash
cargo fmt --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --lib
cargo test --bin sbh
cargo test --test integration_tests
```

If a test depends on Linux filesystem semantics, run it from a checkout copied
onto the VM's Linux filesystem. A host-mounted macOS directory can hide
permission, inode, hard-link, and file-lock behavior.

### Lima

Lima's default template is Ubuntu. Create a writable instance with the current
checkout mounted, then run the Linux commands inside it:

```bash
limactl start --name=sbh-linux --mount "$PWD:w" template:default
limactl shell --workdir "$PWD" sbh-linux cargo test --lib
```

For a full Linux-filesystem proof, clone or copy the repository inside the VM
and run the same gate sequence there instead of using the host mount.

### OrbStack

OrbStack exposes Mac files inside Linux machines under `/mnt/mac`. Create an
Ubuntu machine and run the Linux gate from the mounted checkout:

```bash
orb create ubuntu sbh-linux
orb -m sbh-linux bash -lc 'cd /mnt/mac'"$PWD"' && cargo test --lib'
```

For x86_64 Linux behavior on Apple Silicon, create an amd64 machine:

```bash
orb create --arch amd64 ubuntu sbh-linux-amd64
```

### UTM

UTM works well when you need a fuller VM boundary or a manually managed Linux
desktop/server guest. Create an Ubuntu VM, install the Rust stable toolchain,
clone or share this repository, and run the Linux gate sequence through SSH.
If the UTM Guest Agent is installed, UTM scripting can execute guest commands;
SSH remains the simpler default for repeatable test runs.

## References

- Lima `limactl start`: https://lima-vm.io/docs/reference/limactl_start/
- Lima `limactl shell`: https://lima-vm.io/docs/reference/limactl_shell/
- Lima templates: https://lima-vm.io/docs/templates/
- OrbStack Linux machines: https://docs.orbstack.dev/machines/
- OrbStack commands: https://docs.orbstack.dev/machines/commands
- UTM documentation: https://docs.getutm.app/
- UTM scripting reference: https://docs.getutm.app/scripting/reference/
