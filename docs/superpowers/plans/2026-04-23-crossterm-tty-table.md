# Crossterm TTY Table Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a dedicated printer boundary so `git-all` can keep current non-TTY output unchanged while rendering a plain-text live crossterm table in TTY mode.

**Architecture:** Split rendering concerns out of `rust/src/runner.rs` into a new `rust/src/printer.rs` module. The runner keeps ownership of execution, ordered result handling, and tracing, while `PlainPrinter` preserves current non-TTY behavior and `TtyTablePrinter` owns the live alpha-window table plus footer. Extend `script/bench` with a TTY mode so performance can be compared against `main` under both non-TTY and pseudo-terminal conditions.

**Tech Stack:** Rust, `crossterm`, existing runner trace infrastructure, `cargo test`, `script/test`, `script/bench`, `/usr/bin/script`, and hyperfine.

## Success Criteria

- Non-TTY output remains behaviorally unchanged: same ordered plain-text rows, no ANSI sequences, and existing trace coverage stays green.
- Execution and rendering are cleanly separated: `runner.rs` owns repo execution and result flow, while printer implementations own formatting and terminal behavior.
- TTY mode renders a plain-text live table with no header row, alpha-ordered rows, inline `running` placeholders, a footer showing visible slice, complete count, running count, and elapsed time, and the final table remains on screen.
- TTY behavior is visually verified in a real terminal session using `tmux`, with captured live and final output from real repo roots under `~/work` and `~/src/oss`.
- `script/bench` can benchmark both non-TTY and pseudo-terminal runs so `main` and `crossterm-smart-tty` can be compared for `status` and `pull` in `~/work` and `~/src/oss` across multiple runs.
- Final validation includes Rust tests plus the full benchmark matrix and records the results in `docs/dev/benchmarks.md`.

---

## File Structure

- Create: `rust/src/printer.rs`
  Contains printer traits and types, the shared row model, the plain printer, the TTY table printer, and focused unit tests for viewport/footer behavior.
- Modify: `rust/src/main.rs`
  Registers the new printer module.
- Modify: `rust/src/runner.rs`
  Replaces direct `println!()` output with printer-driven rendering while keeping execution and trace ownership in the runner.
- Modify: `rust/src/commands/status.rs`
  Carries the mutable execution context through `run_parallel`.
- Modify: `rust/src/commands/pull.rs`
  Carries the mutable execution context through `run_parallel`.
- Modify: `rust/src/commands/fetch.rs`
  Carries the mutable execution context through `run_parallel`.
- Modify: `rust/src/commands/passthrough.rs`
  Carries the mutable execution context through `run_parallel`.
- Modify: `rust/tests/trace_test.rs`
  Verifies non-TTY behavior remains plain and trace output remains available.
- Modify: `script/bench`
  Adds a pseudo-terminal benchmark mode so crossterm rendering is measured under TTY conditions.
- Modify: `docs/dev/benchmarks.md`
  Documents the intended benchmark matrix and how TTY runs differ from current non-TTY hyperfine runs.

## Task 1: Introduce The Printer Boundary Without Changing Non-TTY Behavior

**Files:**
- Create: `rust/src/printer.rs`
- Modify: `rust/src/main.rs`
- Modify: `rust/src/runner.rs`
- Modify: `rust/tests/trace_test.rs`

- [ ] **Step 1: Write the failing printer unit tests**

Create `rust/src/printer.rs` with only the tests below first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_printer_formats_repo_and_output_without_ansi() {
        let mut output = Vec::new();
        let rows = vec![RepoRow::new(0, "agentic-dev".to_string(), "running".to_string())];

        {
            let mut printer = PlainPrinter::new(&mut output, 12);
            printer.start(&rows).expect("plain printer start");
            printer.finish_row(0, &rows[0]).expect("plain printer finish");
        }

        let rendered = String::from_utf8(output).expect("utf8");
        assert_eq!(rendered, "[agentic-dev ] running\n");
        assert!(!rendered.contains('\u{1b}'));
    }

    #[test]
    fn viewport_follows_first_unfinished_repo() {
        let rows = vec![
            RepoRow::finished(0, "activities".to_string(), "clean".to_string()),
            RepoRow::finished(1, "agentic-dev".to_string(), "clean".to_string()),
            RepoRow::running(2, "amion-api".to_string()),
            RepoRow::running(3, "api-gateway".to_string()),
            RepoRow::running(4, "billing".to_string()),
        ];

        let viewport = Viewport::for_rows(&rows, 3);

        assert_eq!(viewport.start, 2);
        assert_eq!(viewport.end, 5);
    }

    #[test]
    fn footer_includes_slice_counts_and_elapsed_time() {
        let footer = FooterState {
            visible_start: 24,
            visible_end: 47,
            total_rows: 98,
            complete: 41,
            running: 8,
            elapsed_ms: 2100,
        };

        assert_eq!(
            footer.render(),
            "showing 24-47 of 98 | 41 complete | 8 running | 2.1s"
        );
    }
}
```

- [ ] **Step 2: Run the printer test target to verify it fails**

Run:

```bash
cargo test printer::tests::plain_printer_formats_repo_and_output_without_ansi
```

Expected: FAIL because `rust/src/printer.rs`, `RepoRow`, `PlainPrinter`, `Viewport`, and `FooterState` do not exist yet.

- [ ] **Step 3: Implement the minimal printer scaffolding**

Create `rust/src/printer.rs` with the following minimal structure:

```rust
use std::io::{self, Write};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowState {
    Running,
    Finished,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoRow {
    pub index: usize,
    pub repo: String,
    pub output: String,
    pub state: RowState,
}

impl RepoRow {
    pub fn new(index: usize, repo: String, output: String) -> Self {
        Self {
            index,
            repo,
            output,
            state: RowState::Finished,
        }
    }

    pub fn running(index: usize, repo: String) -> Self {
        Self {
            index,
            repo,
            output: "running".to_string(),
            state: RowState::Running,
        }
    }

    pub fn finished(index: usize, repo: String, output: String) -> Self {
        Self {
            index,
            repo,
            output,
            state: RowState::Finished,
        }
    }
}

pub struct Viewport {
    pub start: usize,
    pub end: usize,
}

impl Viewport {
    pub fn for_rows(rows: &[RepoRow], height: usize) -> Self {
        let height = height.max(1);
        let anchor = rows
            .iter()
            .position(|row| row.state == RowState::Running)
            .unwrap_or(rows.len().saturating_sub(height));
        let start = anchor.min(rows.len().saturating_sub(height));
        let end = (start + height).min(rows.len());
        Self { start, end }
    }
}

pub struct FooterState {
    pub visible_start: usize,
    pub visible_end: usize,
    pub total_rows: usize,
    pub complete: usize,
    pub running: usize,
    pub elapsed_ms: u128,
}

impl FooterState {
    pub fn render(&self) -> String {
        format!(
            "showing {}-{} of {} | {} complete | {} running | {:.1}s",
            self.visible_start,
            self.visible_end,
            self.total_rows,
            self.complete,
            self.running,
            self.elapsed_ms as f64 / 1000.0,
        )
    }
}

pub trait Printer {
    fn start(&mut self, rows: &[RepoRow]) -> io::Result<()>;
    fn finish_row(&mut self, row_index: usize, row: &RepoRow) -> io::Result<()>;
    fn complete(&mut self, rows: &[RepoRow], elapsed_ms: u128) -> io::Result<()>;
}

pub struct PlainPrinter<W: Write> {
    writer: W,
    repo_width: usize,
}

impl<W: Write> PlainPrinter<W> {
    pub fn new(writer: W, repo_width: usize) -> Self {
        Self { writer, repo_width }
    }
}

impl<W: Write> Printer for PlainPrinter<W> {
    fn start(&mut self, _rows: &[RepoRow]) -> io::Result<()> {
        Ok(())
    }

    fn finish_row(&mut self, _row_index: usize, row: &RepoRow) -> io::Result<()> {
        writeln!(
            self.writer,
            "[{:<width$}] {}",
            row.repo,
            row.output,
            width = self.repo_width
        )
    }

    fn complete(&mut self, _rows: &[RepoRow], _elapsed_ms: u128) -> io::Result<()> {
        Ok(())
    }
}
```

Then add the module declaration to `rust/src/main.rs`:

```rust
mod printer;
```

- [ ] **Step 4: Run the new printer unit tests to verify they pass**

Run:

```bash
cargo test printer::tests::plain_printer_formats_repo_and_output_without_ansi
cargo test printer::tests::viewport_follows_first_unfinished_repo
cargo test printer::tests::footer_includes_slice_counts_and_elapsed_time
```

Expected: PASS for all three tests.

- [ ] **Step 5: Route non-TTY output through `PlainPrinter`**

Modify `rust/src/runner.rs` so it constructs plain row state and passes finished rows to `PlainPrinter` while leaving current ordered behavior intact:

```rust
use crate::printer::{PlainPrinter, Printer, RepoRow, RowState};

let mut rows: Vec<RepoRow> = repos
    .iter()
    .enumerate()
    .map(|(idx, repo)| RepoRow::running(idx, repo_display_name(repo, ctx.display_root())))
    .collect();

let stdout = std::io::stdout();
let mut printer = PlainPrinter::new(stdout.lock(), name_width);
printer.start(&rows)?;

// when a repo is ready to print:
rows[next_to_print].output = output_text;
rows[next_to_print].state = RowState::Finished;
printer.finish_row(next_to_print, &rows[next_to_print])?;
```

Keep the trace emission exactly where printed rows become visible.

- [ ] **Step 6: Run the non-TTY integration tests to verify behavior remains stable**

Run:

```bash
cargo test --test trace_test
script/test -t rust
```

Expected: PASS. Existing ordered non-TTY behavior and trace output remain green.

- [ ] **Step 7: Commit the printer boundary**

Run:

```bash
git add rust/src/main.rs rust/src/printer.rs rust/src/runner.rs rust/tests/trace_test.rs
git commit -m "add printer boundary for runner output"
```

## Task 2: Add The TTY Table Printer With Alpha Window And Footer

**Files:**
- Modify: `rust/src/printer.rs`
- Modify: `rust/src/runner.rs`
- Test: `rust/src/printer.rs`
- Test: `rust/tests/trace_test.rs`

- [ ] **Step 1: Add failing unit tests for the TTY table printer**

Extend `rust/src/printer.rs` tests with:

```rust
#[test]
fn tty_table_printer_renders_running_rows_without_headers() {
    let rows = vec![
        RepoRow::running(0, "activities".to_string()),
        RepoRow::running(1, "agentic-dev".to_string()),
    ];
    let mut output = Vec::new();

    {
        let mut printer = TtyTablePrinter::new(&mut output, 6, 14);
        printer.start(&rows).expect("tty start");
    }

    let rendered = String::from_utf8(output).expect("utf8");
    assert!(rendered.contains("activities"));
    assert!(rendered.contains("running"));
    assert!(!rendered.contains("REPO"));
    assert!(!rendered.contains("OUTPUT"));
}

#[test]
fn tty_table_printer_keeps_completed_rows_in_place() {
    let mut rows = vec![
        RepoRow::running(0, "activities".to_string()),
        RepoRow::running(1, "agentic-dev".to_string()),
    ];
    let mut output = Vec::new();

    {
        let mut printer = TtyTablePrinter::new(&mut output, 6, 14);
        printer.start(&rows).expect("tty start");
        rows[0] = RepoRow::finished(0, "activities".to_string(), "clean".to_string());
        printer.finish_row(0, &rows[0]).expect("tty finish");
    }

    let rendered = String::from_utf8(output).expect("utf8");
    assert!(rendered.contains("activities"));
    assert!(rendered.contains("clean"));
    assert!(rendered.contains("agentic-dev"));
}
```

- [ ] **Step 2: Run the targeted TTY printer tests to verify they fail**

Run:

```bash
cargo test printer::tests::tty_table_printer_renders_running_rows_without_headers
```

Expected: FAIL because `TtyTablePrinter` does not exist yet.

- [ ] **Step 3: Add `crossterm` and implement `TtyTablePrinter`**

Update `rust/Cargo.toml`:

```toml
[dependencies]
clap = { version = "4.5", features = ["derive"] }
anyhow = "1.0"
crossterm = "0.29"
```

Then extend `rust/src/printer.rs` with a minimal TTY printer:

```rust
use crossterm::cursor::{MoveToColumn, MoveUp};
use crossterm::terminal::{Clear, ClearType};
use crossterm::{execute, queue};

pub struct TtyTablePrinter<W: Write> {
    writer: W,
    terminal_rows: usize,
    repo_width: usize,
}

impl<W: Write> TtyTablePrinter<W> {
    pub fn new(writer: W, terminal_rows: usize, repo_width: usize) -> Self {
        Self {
            writer,
            terminal_rows,
            repo_width,
        }
    }

    fn render_row(&self, row: &RepoRow) -> String {
        format!("{:<width$}  {}", row.repo, row.output, width = self.repo_width)
    }
}
```

Implement `start`, `finish_row`, and `complete` so the printer:

- renders only the visible alpha window
- uses `running` for unfinished rows
- redraws rows and footer in place
- leaves the final table on screen

Use a startup terminal height snapshot and reserve one row for the footer:

```rust
let visible_height = self.terminal_rows.saturating_sub(1).max(1);
let viewport = Viewport::for_rows(rows, visible_height);
```

- [ ] **Step 4: Select `TtyTablePrinter` only when stdout is a TTY**

Modify `rust/src/runner.rs` to branch once at startup:

```rust
use crossterm::tty::IsTty;

let stdout = std::io::stdout();
let is_tty = stdout.is_tty();
```

For the first version:

- if `is_tty`, create `TtyTablePrinter`
- otherwise create `PlainPrinter`

Do not alter non-TTY printed text.

- [ ] **Step 5: Run the printer and integration tests to verify they pass**

Run:

```bash
cargo test printer::tests::tty_table_printer_renders_running_rows_without_headers
cargo test printer::tests::tty_table_printer_keeps_completed_rows_in_place
cargo test --test trace_test
script/test -t rust
```

Expected: PASS. TTY printer unit tests pass, trace tests remain green, and the full Rust suite passes.

- [ ] **Step 6: Perform a visual TTY check in `tmux` with real repo roots**

Run the built binary inside detached `tmux` sessions so live output can be captured while the command is still running:

```bash
script/build -t rust
tmux kill-session -t git-all-tty-work-status 2>/dev/null || true
tmux new-session -d -s git-all-tty-work-status "cd ~/work && /Users/rsanheim/.dx-worktrees/rsanheim/git-all/crossterm-smart-tty/bin/git-all-rust -n 8 status; tmux wait-for -S git-all-tty-work-status-done"
sleep 1
tmux capture-pane -pt git-all-tty-work-status > /tmp/git-all-tty-work-status-live.txt
tmux wait-for git-all-tty-work-status-done
tmux capture-pane -pt git-all-tty-work-status > /tmp/git-all-tty-work-status-final.txt

tmux kill-session -t git-all-tty-oss-status 2>/dev/null || true
tmux new-session -d -s git-all-tty-oss-status "cd ~/src/oss && /Users/rsanheim/.dx-worktrees/rsanheim/git-all/crossterm-smart-tty/bin/git-all-rust -n 8 status; tmux wait-for -S git-all-tty-oss-status-done"
sleep 1
tmux capture-pane -pt git-all-tty-oss-status > /tmp/git-all-tty-oss-status-live.txt
tmux wait-for git-all-tty-oss-status-done
tmux capture-pane -pt git-all-tty-oss-status > /tmp/git-all-tty-oss-status-final.txt
```

Then inspect the captured panes:

```bash
sed -n '1,80p' /tmp/git-all-tty-work-status-live.txt
sed -n '1,80p' /tmp/git-all-tty-work-status-final.txt
sed -n '1,80p' /tmp/git-all-tty-oss-status-live.txt
sed -n '1,80p' /tmp/git-all-tty-oss-status-final.txt
```

Check manually:

- no visible header row
- rows show repo and current formatter output
- at least one in-flight row displays `running` in the live capture
- footer includes visible slice, complete count, running count, and elapsed time
- final capture still shows the final table
- both `~/work` and `~/src/oss` render correctly under a real TTY

- [ ] **Step 7: Commit the TTY table printer**

Run:

```bash
git add rust/Cargo.toml rust/src/printer.rs rust/src/runner.rs rust/tests/trace_test.rs
git commit -m "add crossterm tty table printer"
```

## Task 3: Add TTY Benchmark Support To `script/bench`

**Files:**
- Modify: `script/bench`
- Modify: `docs/dev/benchmarks.md`
- Test: `script/bench`

- [ ] **Step 1: Add a failing shell-level check for TTY command construction**

Add this helper test block near the bottom of `script/bench` guarded behind a test env var:

```bash
if [[ "${GIT_ALL_BENCH_SELFTEST:-}" == "1" ]]; then
    TTY_MODE="1"
    build_bench_cmd "/tmp/git-all-rust-main-abc123" "status" "8" "/tmp/work"
    exit 0
fi
```

Add a new helper function call that currently does not exist:

```bash
build_bench_cmd "/tmp/git-all-rust-main-abc123" "status" "8" "/tmp/work"
```

- [ ] **Step 2: Run the self-test to verify it fails**

Run:

```bash
GIT_ALL_BENCH_SELFTEST=1 script/bench git
```

Expected: FAIL because `build_bench_cmd` and `TTY_MODE` handling do not exist yet.

- [ ] **Step 3: Implement `--tty` mode in `script/bench`**

Modify `script/bench` to add:

```bash
TTY_MODE="0"
```

Extend help output with:

```text
  --tty                  Run benchmark commands under a pseudo-terminal
```

Parse the flag in `cmd_git`:

```bash
            --tty) TTY_MODE="1"; shift ;;
```

Add a helper to construct the actual benchmark command:

```bash
build_bench_cmd() {
    local bin="$1"
    local cmd="$2"
    local workers="$3"
    local dir="$4"

    local base="$bin -n $workers $cmd"
    [[ -n "$dir" ]] && base="cd '$dir' && $base"

    if [[ "$TTY_MODE" == "1" ]]; then
        printf "script -q /dev/null bash -lc %q" "$base"
    else
        printf "%s" "$base"
    fi
}
```

Then use it for both baseline and target:

```bash
local BASELINE_CMD
local TARGET_CMD
BASELINE_CMD=$(build_bench_cmd "$BASELINE_BIN" "$CMD" "$WORKERS" "$BENCH_DIR")
TARGET_CMD=$(build_bench_cmd "$TARGET_BIN" "$CMD" "$WORKERS" "$BENCH_DIR")
```

- [ ] **Step 4: Update benchmark documentation**

Add a TTY benchmarking section to `docs/dev/benchmarks.md`:

```markdown
## TTY Benchmarking

The crossterm renderer is only exercised when stdout is a TTY. Use:

```bash
script/bench git -I rust -b main -t crossterm-smart-tty -d ~/work -c status -n 8 --tty
script/bench git -I rust -b main -t crossterm-smart-tty -d ~/work -c pull -n 8 --tty
script/bench git -I rust -b main -t crossterm-smart-tty -d ~/src/oss -c status -n 8 --tty
script/bench git -I rust -b main -t crossterm-smart-tty -d ~/src/oss -c pull -n 8 --tty
```

Run the same matrix without `--tty` to verify non-TTY behavior remains stable.
```

- [ ] **Step 5: Run the self-test and a real benchmark smoke check**

Run:

```bash
GIT_ALL_BENCH_SELFTEST=1 script/bench git
script/bench git -I rust -b main -t crossterm-smart-tty -d ~/work -c status -n 8 --tty --show-output
```

Expected:

- self-test exits successfully
- smoke check runs both refs under a pseudo-terminal and produces hyperfine output

- [ ] **Step 6: Commit the benchmark support**

Run:

```bash
git add script/bench docs/dev/benchmarks.md
git commit -m "add tty benchmark mode for crossterm runs"
```

## Task 4: Run The Full Validation Matrix And Record Results

**Files:**
- Modify: `docs/dev/benchmarks.md`

- [ ] **Step 1: Run the non-TTY benchmark matrix**

Run:

```bash
script/bench git -I rust -b main -t crossterm-smart-tty -d ~/work -c status -n 8
script/bench git -I rust -b main -t crossterm-smart-tty -d ~/work -c pull -n 8
script/bench git -I rust -b main -t crossterm-smart-tty -d ~/src/oss -c status -n 8
script/bench git -I rust -b main -t crossterm-smart-tty -d ~/src/oss -c pull -n 8
```

Expected: hyperfine comparison output for four non-TTY cases.

- [ ] **Step 2: Run the TTY benchmark matrix**

Run:

```bash
script/bench git -I rust -b main -t crossterm-smart-tty -d ~/work -c status -n 8 --tty
script/bench git -I rust -b main -t crossterm-smart-tty -d ~/work -c pull -n 8 --tty
script/bench git -I rust -b main -t crossterm-smart-tty -d ~/src/oss -c status -n 8 --tty
script/bench git -I rust -b main -t crossterm-smart-tty -d ~/src/oss -c pull -n 8 --tty
```

Expected: hyperfine comparison output for four TTY cases.

- [ ] **Step 3: Capture visual TTY runs for `pull` in both real repo roots**

Run detached `tmux` sessions for the live `pull` command and capture both the in-flight and final screen state:

```bash
tmux kill-session -t git-all-tty-work-pull 2>/dev/null || true
tmux new-session -d -s git-all-tty-work-pull "cd ~/work && /Users/rsanheim/.dx-worktrees/rsanheim/git-all/crossterm-smart-tty/bin/git-all-rust -n 8 pull; tmux wait-for -S git-all-tty-work-pull-done"
sleep 2
tmux capture-pane -pt git-all-tty-work-pull > /tmp/git-all-tty-work-pull-live.txt
tmux wait-for git-all-tty-work-pull-done
tmux capture-pane -pt git-all-tty-work-pull > /tmp/git-all-tty-work-pull-final.txt

tmux kill-session -t git-all-tty-oss-pull 2>/dev/null || true
tmux new-session -d -s git-all-tty-oss-pull "cd ~/src/oss && /Users/rsanheim/.dx-worktrees/rsanheim/git-all/crossterm-smart-tty/bin/git-all-rust -n 8 pull; tmux wait-for -S git-all-tty-oss-pull-done"
sleep 2
tmux capture-pane -pt git-all-tty-oss-pull > /tmp/git-all-tty-oss-pull-live.txt
tmux wait-for git-all-tty-oss-pull-done
tmux capture-pane -pt git-all-tty-oss-pull > /tmp/git-all-tty-oss-pull-final.txt
```

Expected: four captured pane files showing live and final TTY output for `pull` under real repo roots.

- [ ] **Step 4: Capture a trace spot-check for ordered-wait sanity**

Run:

```bash
cd ~/work
GIT_ALL_TRACE_FILE=/tmp/git-all-crossterm.trace /Users/rsanheim/.dx-worktrees/rsanheim/git-all/crossterm-smart-tty/bin/git-all-rust -n 8 status >/tmp/git-all-crossterm.stdout
rg 'phase=summary' /tmp/git-all-crossterm.trace
```

Expected: one summary line containing `first_print_ms`, `delayed_repos`, `max_ordered_wait_ms`, and `total_ms`.

- [ ] **Step 5: Record benchmark notes in `docs/dev/benchmarks.md`**

Append a new results section with:

- non-TTY `status` comparison notes for `~/work` and `~/src/oss`
- non-TTY `pull` comparison notes for `~/work` and `~/src/oss`
- TTY `status` comparison notes for `~/work` and `~/src/oss`
- TTY `pull` comparison notes for `~/work` and `~/src/oss`
- one trace spot-check summary line

Use this template:

```markdown
## Crossterm TTY Table Validation

### Non-TTY

- `~/work status`: `<paste hyperfine summary>`
- `~/work pull`: `<paste hyperfine summary>`
- `~/src/oss status`: `<paste hyperfine summary>`
- `~/src/oss pull`: `<paste hyperfine summary>`

### TTY

- `~/work status`: `<paste hyperfine summary>`
- `~/work pull`: `<paste hyperfine summary>`
- `~/src/oss status`: `<paste hyperfine summary>`
- `~/src/oss pull`: `<paste hyperfine summary>`

### Visual Checks

- `~/work status`: `tmux` live/final capture reviewed
- `~/src/oss status`: `tmux` live/final capture reviewed
- `~/work pull`: `tmux` live/final capture reviewed
- `~/src/oss pull`: `tmux` live/final capture reviewed

### Trace Spot Check

- `status ~/work`: `first_print_ms=<...> delayed_repos=<...> max_ordered_wait_ms=<...> total_ms=<...>`
```

- [ ] **Step 6: Run the full Rust verification one final time**

Run:

```bash
script/test -t rust
```

Expected: PASS.

- [ ] **Step 7: Commit the validation results**

Run:

```bash
git add docs/dev/benchmarks.md
git commit -m "record crossterm tty table benchmark results"
```
