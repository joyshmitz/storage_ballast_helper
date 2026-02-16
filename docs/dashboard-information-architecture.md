# Dashboard Information Architecture and Navigation Map (bd-xzt.1.4)

This document defines the operator-facing information architecture for the SBH
dashboard overhaul. It is implementation-facing and intended to remove UI
guesswork for downstream `bd-xzt.2*` and `bd-xzt.3*` work.

References:
- `docs/dashboard-status-contract-baseline.md` (C-01..C-18 parity constraints)
- `docs/frankentui-triage-matrix.md` (screen/pattern shortlist)
- `docs/adr-tui-integration-strategy.md` (selected adaptation strategy)

## 1. IA Goals and Non-Negotiables

### IA goals

1. Minimize time-to-correct-action during pressure incidents.
2. Keep safety signals visible before optimization details.
3. Preserve deterministic behavior and explainability across all screens.
4. Make the same workflow reachable in <= 3 interactions from the default
   screen.

### Non-negotiables

1. Baseline contract parity remains intact (C-01..C-18).
2. Emergency mode remains zero-write and outside TUI dependencies.
3. Degraded mode must preserve critical situational awareness when daemon state
   is stale or missing.
4. Terminal lifecycle guarantees (raw mode/alt-screen cleanup and safe exit)
   remain unchanged.

## 2. Operator Journey Priorities

### Primary journeys (incident-time, high urgency)

1. Pressure triage: detect critical volume, confirm trend, choose cleanup vs
   ballast action.
2. Ballast response: release ballast quickly on the pressured mount and verify
   recovery.

### Secondary journeys (diagnostic/forensic)

1. Explainability drill-down: inspect why a cleanup or policy decision happened.
2. Cleanup candidate review: inspect score breakdowns and safety vetoes before
   action.
3. Timeline/log investigation: reconstruct event sequence after an incident.

## 3. Screen Topology

### Top-level screens

| ID | Screen | Purpose | Default Entry |
| --- | --- | --- | --- |
| S1 | Overview | Global pressure + fast action routing | Yes |
| S2 | Action Timeline | Ordered event stream and severity filtering | No |
| S3 | Explainability Cockpit | Decision evidence and posterior trace | No |
| S4 | Scan Candidates | Candidate ranking + factor/veto inspection | No |
| S5 | Ballast Operations | Per-volume ballast inventory and release/replenish actions | No |

### Overlays / transient surfaces

| ID | Surface | Scope | Trigger |
| --- | --- | --- | --- |
| O1 | Command Palette | Global | `Ctrl-P` or `:` |
| O2 | Help Overlay | Global contextual key map | `?` |
| O3 | VOI Overlay | Contextual (S1/S4/S3) | `v` |
| O4 | Log Search Panel | S2 and S3 context extension | `/` |
| O5 | Confirmation Dialog | Mutating actions (especially S5) | `Enter` on action |

## 4. Cross-Screen Navigation Model

### Global navigation contract

1. Screen switching is direct and deterministic:
   - `1` => S1 Overview
   - `2` => S2 Timeline
   - `3` => S3 Explainability
   - `4` => S4 Scan Candidates
   - `5` => S5 Ballast Operations
2. `[` and `]` cycle top-level screens in order (S1 -> S5 wraparound).
3. `Tab` / `Shift-Tab` cycles focusable panes within current screen.
4. `Enter` drills into selected entity (event, decision, candidate, mount).
5. `Backspace` returns to prior focus context within screen.
6. `Esc` closes top overlay first; if no overlay, clears transient selection.
7. `q` exits dashboard from any non-confirmation state.

### Interaction state precedence

Input handling precedence:
1. Confirmation dialog (O5)
2. Command palette/help/overlay (O1/O2/O3/O4)
3. In-screen focused pane
4. Global screen navigation

This prevents accidental screen switches during destructive confirmation flows.

### Cross-screen route map

```text
S1 Overview
  -> (2) S2 Action Timeline
  -> (3) S3 Explainability Cockpit
  -> (4) S4 Scan Candidates
  -> (5) S5 Ballast Operations
  -> (Enter on alert/event) contextual route to S2/S3/S4/S5

S2 Action Timeline
  -> (Enter on decision event) S3 Explainability [decision preselected]
  -> (Enter on cleanup event) S4 Scan Candidates [candidate filter retained]
  -> (5) S5 Ballast Operations [if event category = ballast]

S3 Explainability Cockpit
  -> (Open related candidate) S4 [candidate id preselected]
  -> (Open related timeline) S2 [cursor at originating event]

S4 Scan Candidates
  -> (Explain selected) S3 [decision/candidate linkage]
  -> (Ballast fallback) S5 [when target reclaim gap remains]

S5 Ballast Operations
  -> (Post-action review) S2 [new ballast event highlighted]
  -> (Pressure stabilized) S1 Overview

Any screen
  -> O1 Command Palette
  -> O2 Help Overlay
  -> O3 VOI Overlay
```

## 5. Always-On vs Drill-Down Information Placement

### Always-on (must remain visible on every top-level screen)

| Signal | Placement | Why always-on |
| --- | --- | --- |
| Worst pressure level + affected mount | Global header/status strip | Immediate incident severity context |
| Daemon mode (`LIVE`/`DEGRADED`) + staleness age | Global header | Prevents false confidence in stale data |
| Predicted exhaustion horizon (if available) | Global header/right rail | Time-to-failure drives urgency |
| Active safety state (veto/circuit-breaker summary) | Global status strip | Prevent unsafe action assumptions |
| Active operation indicator (scan/clean/release in-flight) | Footer/status line | Prevents conflicting operator actions |

### Drill-down (shown only on demand/focus)

| Detail | Primary home | Reveal trigger |
| --- | --- | --- |
| Full evidence ledger entries and posterior math | S3 | `Enter` from decision list |
| Candidate factor contributions and veto reasons | S4 | `Enter` on candidate row |
| Per-volume ballast history and release preview | S5 | Focus ballast table row |
| Full log search context windows | O4 | `/` then search confirm |
| VOI component breakdown and observation payload | O3 | `v` |

## 6. Pane Priority Per Screen

Priority semantics:
- `P0`: must render first; never hidden
- `P1`: visible by default, may collapse on narrow width
- `P2`: optional/secondary; collapses first

### S1 Overview

| Pane | Priority | Narrow (<100 cols) | Wide (>=100 cols) |
| --- | --- | --- | --- |
| Pressure summary + worst-mount alert | P0 | Top block | Left primary column |
| Action recommendation lane (clean vs ballast) | P0 | Directly below pressure | Right top block |
| EWMA/prediction trend | P1 | Collapsible section | Left middle |
| Recent activity snippet | P1 | Tabbed with trends | Right middle |
| Ballast quick status | P1 | Inline compact row | Right lower |
| Extended counters | P2 | Hidden behind expand | Bottom strip |

### S2 Action Timeline

| Pane | Priority |
| --- | --- |
| Event list with severity and timestamp | P0 |
| Event detail pane (selected row) | P1 |
| Filter controls (severity, component, time window) | P1 |
| Auxiliary metrics strip (rate, pressure snapshot at event) | P2 |

### S3 Explainability Cockpit

| Pane | Priority |
| --- | --- |
| Decision list / selector | P0 |
| Evidence summary (why acted / why vetoed) | P0 |
| Posterior and confidence diagnostics | P1 |
| Related candidate/event links | P1 |
| Raw ledger payload view | P2 |

### S4 Scan Candidates

| Pane | Priority |
| --- | --- |
| Candidate table (score, size, age, safety status) | P0 |
| Factor decomposition bars | P1 |
| Safety veto details | P1 |
| Sort/filter controls | P1 |
| Raw metadata panel | P2 |

### S5 Ballast Operations

| Pane | Priority |
| --- | --- |
| Per-volume inventory + pressure linkage | P0 |
| Action controls (release/replenish/verify) | P0 |
| Safety confirmation and projected free-space delta | P1 |
| Historical ballast events | P2 |

## 7. Workflow-to-Screen Paths (Acceptance Mapping)

| Workflow | Primary path | Fallback path | Completion signal |
| --- | --- | --- | --- |
| Pressure triage | S1 -> (Enter alert) -> S2 or S4 | S1 -> S5 if reclaim shortfall | Pressure returns below alert threshold or incident acknowledged |
| Explainability drill-down | S1/S2 -> S3 | Command palette -> S3 | Operator sees decision reason + evidence trail |
| Cleanup candidate review | S1 -> S4 | S2 cleanup event -> S4 | Candidate chosen/rejected with safety rationale |
| Ballast response | S1 -> S5 -> O5 confirm | S2 ballast event -> S5 | Release/replenish event recorded and reflected in S1 |

Each major workflow is reachable from S1 in direct navigation or one contextual
drill-down.

## 8. Implementation Guardrails for Downstream Beads

1. `bd-xzt.2.5` and `bd-xzt.2.6` must implement the global navigation contract
   and pane priority model exactly as defined here.
2. `bd-xzt.3.1` through `bd-xzt.3.5` must expose the listed P0 panes at all
   terminal widths.
3. `bd-xzt.3.11` must preserve always-on signals even under compact preferences.
4. `bd-xzt.4.*` verification must test:
   - screen switching determinism (`1..5`, `[`/`]`)
   - overlay precedence and safe escape behavior
   - workflow paths in the mapping table above

## 9. Exit Criteria for bd-xzt.1.4

This IA is considered complete when:

1. Topology and navigation behavior are specific enough for direct
   implementation without additional UX decisions.
2. The four major SBH workflows each map to explicit screen paths.
3. Always-on and drill-down placement decisions are documented with operator
   speed/safety rationale.

