# Session Report - Codebase Exploration and Fixes

## Overview
I performed a random exploration of the codebase, focusing on `src/monitor`, `src/ballast`, `src/scanner`, and `src/daemon`. I identified and fixed functional bugs, improved pressure response logic, and hardened the safety guardrails.

## Tasks Completed

1.  **Fixed Ballast File Leak (src/ballast/manager.rs)**
    *   **Issue:** Reducing `ballast.file_count` in the configuration left "orphaned" ballast files on disk.
    *   **Fix:** Implemented `prune_orphans` method in `BallastManager` and integrated it into lifecycle methods.
    *   **Verification:** Added test `reducing_file_count_removes_orphans`.

2.  **Fixed Swap Thrash Logic (src/daemon/loop_main.rs)**
    *   **Issue:** The swap thrash detection logic was inverted (detecting lazy swap instead of thrashing).
    *   **Fix:** Renamed `SWAP_THRASH_MIN_AVAILABLE_RAM_BYTES` to `SWAP_THRASH_MAX_AVAILABLE_RAM_BYTES` (1GB) and updated comparison logic.
    *   **Verification:** Added test `test_swap_thrash_logic_correct_behavior`.

3.  **Dynamic Pressure Response (src/monitor/pid.rs)**
    *   **Observation:** The `max_delete_batch` size was static for Red/Critical levels, limiting throughput during rapid pressure spikes.
    *   **Improvement:** Updated `response_policy` to scale batch size linearly with urgency (e.g., Red can scale from 20 to 50 items per batch).
    *   **Verification:** Added test `response_policy_scales_batch_size_with_urgency`.

4.  **Hardened Guardrails (src/monitor/guardrails.rs)**
    *   **Issue:** Tiny floating-point noise (< 1e-9) during idle periods could cause infinite error ratios in calibration checks, potentially triggering false-positive safety fallbacks.
    *   **Fix:** Updated `rate_error_ratio` to ignore errors when both predicted and actual rates are trivial (< 1.0 byte/sec).
    *   **Verification:** Added test `rate_error_ratio_ignores_idle_noise`.

5.  **Math Verification (src/monitor/ewma.rs)**
    *   **Analysis:** Verified the quadratic time-to-exhaustion formula, including the use of the conjugate form for numerical stability when decelerating. Confirmed correct handling of negative rates.

## Next Steps
The shell environment remains unstable (`Signal: 1`), preventing execution of `cargo test`. All changes include regression tests that should be run once the environment is restored.
