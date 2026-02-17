use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;

#[cfg(target_os = "linux")]
use sbh_lib::scanner::walker::is_path_open;

#[test]
#[cfg(target_os = "linux")]
fn repro_symlink_loop_dos() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("scan_root");
    fs::create_dir(&root).unwrap();

    // Create a symlink loop: root/loop -> root
    let loop_link = root.join("loop");
    std::os::unix::fs::symlink(&root, &loop_link).unwrap();

    // Create a deeply nested structure to verify it traverses
    let deep = root.join("a/b/c");
    fs::create_dir_all(&deep).unwrap();

    let open_inodes = HashSet::new();

    // Run in a separate thread with a timeout because it WILL hang/crash without the fix.
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        // This should return false (no open files) quickly.
        // If it hangs or stacks overflows, the test fails.
        let result = is_path_open(&root, &open_inodes);
        tx.send(result).unwrap();
    });

    // If it takes more than 2 seconds, it's likely stuck in the loop.
    match rx.recv_timeout(Duration::from_secs(2)) {
        Ok(res) => assert!(!res, "Should not find open files"),
        Err(_) => panic!("is_path_open timed out - likely stuck in symlink loop"),
    }
}
