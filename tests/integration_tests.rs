//! Integration smoke tests for the scaffolded `sbh` CLI surface.

mod common;

#[test]
fn help_command_prints_usage() {
    let result = common::run_cli_case("help_command_prints_usage", &["--help"]);
    assert!(
        result.status.success(),
        "expected success; log: {}",
        result.log_path.display()
    );
    assert!(
        result.stdout.contains("Usage: sbh <COMMAND>"),
        "missing help banner; log: {}",
        result.log_path.display()
    );
}

#[test]
fn version_command_prints_version() {
    let result = common::run_cli_case("version_command_prints_version", &["--version"]);
    assert!(
        result.status.success(),
        "expected success; log: {}",
        result.log_path.display()
    );
    assert!(
        result.stdout.contains("storage_ballast_helper")
            || result.stdout.contains("sbh")
            || result.stderr.contains("storage_ballast_helper"),
        "missing version output; log: {}",
        result.log_path.display()
    );
}

#[test]
fn subcommands_have_scaffolded_handlers() {
    let cases = [
        ("install", "install: not yet implemented"),
        ("uninstall", "uninstall: not yet implemented"),
        ("status", "status: not yet implemented"),
        ("stats", "stats: not yet implemented"),
        ("scan", "scan: not yet implemented"),
        ("clean", "clean: not yet implemented"),
        ("ballast", "ballast: not yet implemented"),
        ("config", "config: not yet implemented"),
        ("daemon", "daemon: not yet implemented"),
    ];

    for (cmd, expected) in cases {
        let case_name = format!("subcommand_{cmd}");
        let result = common::run_cli_case(&case_name, &[cmd]);
        assert!(
            result.status.success(),
            "subcommand {cmd} failed; log: {}",
            result.log_path.display()
        );
        assert!(
            result.stdout.contains(expected) || result.stderr.contains(expected),
            "subcommand {cmd} output mismatch; log: {}",
            result.log_path.display()
        );
    }
}
