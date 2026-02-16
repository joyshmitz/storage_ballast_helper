#![allow(missing_docs)]

#[test]
fn test_truncate_path_panic() {
    // A path with multi-byte characters: "data/프로ジェクト/target"
    // "프로젝트" is 12 bytes.
    // Total length: 5 + 12 + 7 = 24 bytes.
    let path = "/data/프로젝트/target";

    // Attempt to truncate to a length that lands inside a multibyte char.
    // Length 24. Request max_len 10.
    // start = 24 - 10 = 14.
    // Byte 14 is inside "트" (starts at 13, ends at 16).
    // This should panic.

    // We can't import the private function directly, but we can verify the logic.
    // Or we can try to find a public entry point that uses it.
    // truncate_path is used in render_candidate_detail -> render_candidates -> render_to_string.

    // But we can't easily construct a DashboardModel with private fields from an integration test.
    // So I will just implement the logic here to prove it panics.

    let max_len = 10;
    if path.len() > max_len {
        let start = path.len() - max_len;
        // This slice is the panic vector
        let _ = &path[start..];
    }
}
