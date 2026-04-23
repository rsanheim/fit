# Spike 2 Reserved TTY Rows Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add TTY-only reserved row updates while preserving the current non-TTY completion-order stream and keeping the trace text format unchanged.

**Architecture:** Start from commit `f035766` on branch `spike/tty-row-updates`. Split rendering concerns out of `runner.rs` into a dedicated printer module so the runner can keep emitting completion events while the printer chooses either reserved-row TTY rendering or the existing append-only completion-order fallback.

**Tech Stack:** Rust stdlib I/O and terminal detection, existing trace tests, `script/test`, `script/build`, and manual TTY capture via `script`.

---

**Worktree Setup**

Run this before Task 1 so the spike stacks on validated Spike 1 output behavior:

```bash
git worktree add ../git-all-spike2 f035766
cd ../git-all-spike2
git checkout -b spike/tty-row-updates
```

**File Structure**

- Create: `rust/src/printer.rs`
  Hold TTY/non-TTY rendering decisions and the reserved-row rendering logic.
- Modify: `rust/src/main.rs`
  Register the new printer module.
- Modify: `rust/src/runner.rs`
  Emit completion events into the new printer abstraction instead of calling `println!()` directly.
- Modify: `rust/tests/trace_test.rs`
  Keep the non-TTY completion-order trace regression stable.
- Modify: `docs/plans/output-spike-tracker.md`
  Record both the trace metrics and the manual UX notes for the TTY spike.

### Task 1: Create The Printer Module With Testable Writers

**Files:**
- Create: `rust/src/printer.rs`
- Modify: `rust/src/main.rs`
- Test: `rust/src/printer.rs`

- [ ] **Step 1: Write the failing printer tests**

Create `rust/src/printer.rs` with only these tests first:

```rust
use std::io::{self, Write};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tty_row_printer_rewrites_completed_row_in_place() {
        let mut output = Vec::new();
        let rows = vec![
            RepoRow::new(1, "activities".to_string()),
            RepoRow::new(2, "agentic-dev".to_string()),
            RepoRow::new(3, "amion-api".to_string()),
        ];

        {
            let mut printer = TtyRowPrinter::new(&mut output, 20);
            printer.start(&rows).expect("start tty rows");
            printer.finish(&rows[1], "clean").expect("rewrite row");
        }

        let rendered = String::from_utf8(output).expect("utf8 output");
        assert!(rendered.contains("[001 activities          ] running..."));
        assert!(rendered.contains("[002 agentic-dev         ] running..."));
        assert!(rendered.contains("\u{1b}[2A"));
        assert!(rendered.contains("\u{1b}[2K"));
        assert!(rendered.contains("[002 agentic-dev         ] clean"));
    }

    #[test]
    fn stream_printer_emits_plain_lines_without_escape_codes() {
        let mut output = Vec::new();
        let row = RepoRow::new(2, "agentic-dev".to_string());

        {
            let mut printer = StreamPrinter::new(&mut output, 20);
            printer.finish(&row, "clean").expect("plain line");
        }

        let rendered = String::from_utf8(output).expect("utf8 output");
        assert_eq!(rendered, "[002 agentic-dev         ] clean\n");
    }
}
```

- [ ] **Step 2: Run the unit tests to verify they fail**

Run:

```bash
cargo test printer::tests::tty_row_printer_rewrites_completed_row_in_place
```

Expected: FAIL because `RepoRow`, `TtyRowPrinter`, and `StreamPrinter` do not exist yet.

- [ ] **Step 3: Implement the minimal printer types**

Fill in `rust/src/printer.rs` with a small printer API:

```rust
use std::io::{self, Write};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoRow {
    pub idx: usize,
    pub name: String,
}

impl RepoRow {
    pub fn new(idx: usize, name: String) -> Self {
        Self { idx, name }
    }
}

pub struct StreamPrinter<W: Write> {
    writer: W,
    name_width: usize,
}

impl<W: Write> StreamPrinter<W> {
    pub fn new(writer: W, name_width: usize) -> Self {
        Self { writer, name_width }
    }

    pub fn finish(&mut self, row: &RepoRow, status: &str) -> io::Result<()> {
        writeln!(self.writer, "[{:03} {:<width$}] {}", row.idx, row.name, status, width = self.name_width)
    }
}

pub struct TtyRowPrinter<W: Write> {
    writer: W,
    name_width: usize,
    total_rows: usize,
}

impl<W: Write> TtyRowPrinter<W> {
    pub fn new(writer: W, name_width: usize) -> Self {
        Self {
            writer,
            name_width,
            total_rows: 0,
        }
    }

    pub fn start(&mut self, rows: &[RepoRow]) -> io::Result<()> {
        self.total_rows = rows.len();
        for row in rows {
            writeln!(
                self.writer,
                "[{:03} {:<width$}] running...",
                row.idx,
                row.name,
                width = self.name_width
            )?;
        }
        Ok(())
    }

    pub fn finish(&mut self, row: &RepoRow, status: &str) -> io::Result<()> {
        let rows_up = self.total_rows.saturating_sub(row.idx) + 1;
        write!(self.writer, "\x1b[{}A\r\x1b[2K", rows_up)?;
        writeln!(
            self.writer,
            "[{:03} {:<width$}] {}",
            row.idx,
            row.name,
            status,
            width = self.name_width
        )?;
        write!(self.writer, "\x1b[{}B", rows_up.saturating_sub(1))?;
        self.writer.flush()
    }
}
```

Then register the module in `rust/src/main.rs`:

```rust
mod printer;
```

- [ ] **Step 4: Run the printer unit tests to verify they pass**

Run:

```bash
cargo test printer::tests::tty_row_printer_rewrites_completed_row_in_place
cargo test printer::tests::stream_printer_emits_plain_lines_without_escape_codes
```

Expected: PASS for both tests.

- [ ] **Step 5: Commit the printer scaffolding**

Run:

```bash
git add rust/src/main.rs rust/src/printer.rs
git commit -m "spike: add tty and stream printer scaffolding"
```

### Task 2: Route Runner Output Through The Printer Abstraction

**Files:**
- Modify: `rust/src/runner.rs`
- Modify: `rust/src/printer.rs`
- Modify: `rust/tests/trace_test.rs`
- Test: `rust/tests/trace_test.rs`

- [ ] **Step 1: Add the non-TTY fallback regression check**

Extend `rust/tests/trace_test.rs` with one explicit non-TTY fallback assertion:

```rust
#[cfg(unix)]
#[test]
fn non_tty_output_stays_completion_order_without_escape_codes() {
    let temp = tempfile::tempdir().expect("temp dir");
    create_delay_repos(temp.path());

    let output = Command::new(env!("CARGO_BIN_EXE_git-all"))
        .args(["-n", "3", "delay"])
        .current_dir(temp.path())
        .output()
        .expect("git-all should run");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains('\u{1b}'), "stdout should not contain ANSI escapes: {stdout}");
    assert!(stdout.lines().next().unwrap_or("").starts_with("[002 "));
}
```

- [ ] **Step 2: Run the regression as a safety baseline**

Run:

```bash
cargo test --test trace_test non_tty_output_stays_completion_order_without_escape_codes -- --nocapture
```

Expected: PASS. This verifies the current Spike 1 non-TTY behavior before the TTY refactor begins.

- [ ] **Step 3: Integrate the printers into `run_parallel()`**

Update `rust/src/runner.rs` so it builds a sorted `Vec<RepoRow>` once, initializes the TTY printer only when `stdout` is a terminal, and otherwise preserves the current Spike 1 completion-order streaming behavior:

```rust
use std::io::{self, IsTerminal};

use crate::printer::{RepoRow, StreamPrinter, TtyRowPrinter};

// build rows once after computing widths
let rows: Vec<RepoRow> = repos
    .iter()
    .enumerate()
    .map(|(idx, repo)| RepoRow::new(idx + 1, repo_display_name(repo, ctx.display_root())))
    .collect();

let stdout = io::stdout();
let is_tty = stdout.is_terminal();
let mut writer = stdout.lock();

if is_tty {
    let mut printer = TtyRowPrinter::new(&mut writer, name_width);
    printer.start(&rows)?;

    for (idx, repo, result, trace_sample) in rx {
        let row = &rows[idx];
        let status = format_status(&result, formatter);
        printer.finish(row, &status)?;
        // existing trace handling stays unchanged
    }
} else {
    let mut printer = StreamPrinter::new(&mut writer, name_width);

    for (idx, repo, result, trace_sample) in rx {
        let row = &rows[idx];
        let status = format_status(&result, formatter);
        printer.finish(row, &status)?;
        // existing trace handling stays unchanged
    }
}
```

Refactor the old `print_result()` into a pure helper that returns only the status payload:

```rust
fn format_status(
    result: &Result<Output, std::io::Error>,
    formatter: &dyn OutputFormatter,
) -> String {
    match result {
        Ok(output) => formatter.format(output),
        Err(err) => format!("ERROR: {}", err),
    }
}
```

- [ ] **Step 4: Run focused regressions plus the full Rust suite**

Run:

```bash
cargo test --test trace_test completion_order_output_uses_stable_repo_ids -- --nocapture
cargo test --test trace_test non_tty_output_stays_completion_order_without_escape_codes -- --nocapture
cargo test --test trace_test trace_reports_low_ordered_wait_for_completion_order_output -- --nocapture
script/test -t rust
```

Expected: PASS. Non-TTY output should remain completion-order with stable IDs, and trace metrics should stay low.

- [ ] **Step 5: Commit the integrated TTY printer**

Run:

```bash
git add rust/src/runner.rs rust/src/printer.rs rust/tests/trace_test.rs
git commit -m "spike: reserve tty rows for live updates"
```

### Task 3: Capture Manual UX Evidence And Record Results

**Files:**
- Modify: `docs/plans/output-spike-tracker.md`

- [ ] **Step 1: Build the Rust binary for the spike branch**

Run:

```bash
script/build -t rust
```

Expected: PASS and `./bin/git-all-rust` points at the new release build.

- [ ] **Step 2: Capture a traced non-TTY run on `~/work`**

Run:

```bash
cd ~/work
GIT_ALL_TRACE_FILE=/tmp/git-all-spike2.trace /Users/rsanheim/src/rsanheim/git-all/bin/git-all-rust -n 8 status > /tmp/git-all-spike2.stdout
rg 'phase=summary' /tmp/git-all-spike2.trace
sed -n '1,10p' /tmp/git-all-spike2.stdout
```

Expected: the trace summary fields remain comparable to Spike 1 and `stdout` remains completion-order text without ANSI escapes.

- [ ] **Step 3: Capture a TTY session for manual review**

Run from a real terminal:

```bash
cd ~/work
script -q /tmp/git-all-spike2.typescript bash -lc '/Users/rsanheim/src/rsanheim/git-all/bin/git-all-rust -n 8 status'
```

Expected: interactive output shows reserved sorted rows updating in place. After the run, inspect the captured session with:

```bash
sed -n '1,80p' /tmp/git-all-spike2.typescript | cat -v
```

- [ ] **Step 4: Update the tracker doc with metrics and UX notes**

Fill the `Spike 2` row in `docs/plans/output-spike-tracker.md` and add short notes under `TTY UX Notes` covering:

```markdown
- time to first visible feedback
- readability of sorted rows while repos are still running
- whether ANSI output leaked into redirected stdout
- whether the interface felt better than Spike 1 on `~/work`
```

- [ ] **Step 5: Commit the recorded results**

Run:

```bash
git add docs/plans/output-spike-tracker.md
git commit -m "docs: record tty row update results"
```
