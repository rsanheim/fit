# Spike 3 Bounded Worker Queue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the semaphore-gated thread-per-repo runner with a fixed worker queue while keeping the trace text format and the baseline ordered output policy unchanged.

**Architecture:** Start from commit `0c6137c` on branch `spike/bounded-worker-queue`. Keep the printer behavior from that baseline so the spike isolates scheduler cost only: sorted repo discovery, ordered printing, and existing trace fields stay intact while the execution model changes underneath `run_parallel()`.

**Tech Stack:** Rust stdlib threads and synchronization, existing `anyhow`/`clap` CLI, existing trace integration tests, `script/test`, `script/build`, and `script/bench git`.

---

**Worktree Setup**

Run this before Task 1 so the spike stays isolated from `spike/completion-order-output`:

```bash
git worktree add ../git-all-spike3 0c6137c
cd ../git-all-spike3
git checkout -b spike/bounded-worker-queue
```

**File Structure**

- Modify: `rust/src/runner.rs`
  Replace `Semaphore`-gated thread-per-repo spawning with a fixed worker queue and long-lived worker threads.
- Modify: `docs/plans/output-spike-tracker.md`
  Record the benchmark and trace results after the spike is implemented.

### Task 1: Add Queue Primitives To `runner.rs`

**Files:**
- Modify: `rust/src/runner.rs`
- Test: `rust/src/runner.rs`

- [ ] **Step 1: Write the failing unit tests for queue primitives**

Add these tests near the existing `runner.rs` unit tests:

```rust
    #[test]
    fn test_job_queue_returns_jobs_in_sorted_order() {
        let jobs = JobQueue::new(vec![
            QueuedJob::new(0, PathBuf::from("/tmp/a"), GitCommand::new(PathBuf::from("/tmp/a"), vec!["status".into()])),
            QueuedJob::new(1, PathBuf::from("/tmp/b"), GitCommand::new(PathBuf::from("/tmp/b"), vec!["status".into()])),
        ]);

        let first = jobs.next().expect("first queued job");
        let second = jobs.next().expect("second queued job");

        assert_eq!(first.idx, 0);
        assert_eq!(first.repo_path, PathBuf::from("/tmp/a"));
        assert_eq!(second.idx, 1);
        assert_eq!(second.repo_path, PathBuf::from("/tmp/b"));
        assert!(jobs.next().is_none());
    }

    #[test]
    fn test_worker_thread_count_caps_to_repo_count() {
        assert_eq!(worker_thread_count(0, 0), 0);
        assert_eq!(worker_thread_count(0, 3), 3);
        assert_eq!(worker_thread_count(8, 3), 3);
        assert_eq!(worker_thread_count(2, 3), 2);
    }
```

- [ ] **Step 2: Run the unit tests to verify they fail**

Run:

```bash
cargo test runner::tests::test_job_queue_returns_jobs_in_sorted_order
```

Expected: FAIL with unresolved names for `JobQueue`, `QueuedJob`, or `worker_thread_count`.

- [ ] **Step 3: Implement the minimal queue types and worker-count helper**

Add these helpers above `run_parallel()`:

```rust
struct QueuedJob {
    idx: usize,
    repo_path: PathBuf,
    cmd: GitCommand,
}

impl QueuedJob {
    fn new(idx: usize, repo_path: PathBuf, cmd: GitCommand) -> Self {
        Self { idx, repo_path, cmd }
    }
}

struct JobQueue {
    jobs: Mutex<std::vec::IntoIter<QueuedJob>>,
}

impl JobQueue {
    fn new(jobs: Vec<QueuedJob>) -> Self {
        Self {
            jobs: Mutex::new(jobs.into_iter()),
        }
    }

    fn next(&self) -> Option<QueuedJob> {
        self.jobs.lock().unwrap().next()
    }
}

fn worker_thread_count(max_workers: usize, repo_count: usize) -> usize {
    if repo_count == 0 {
        0
    } else if max_workers == 0 {
        repo_count
    } else {
        max_workers.min(repo_count)
    }
}
```

- [ ] **Step 4: Run the unit tests to verify they pass**

Run:

```bash
cargo test runner::tests::test_job_queue_returns_jobs_in_sorted_order
cargo test runner::tests::test_worker_thread_count_caps_to_repo_count
```

Expected: PASS for both tests.

- [ ] **Step 5: Commit the queue primitive scaffolding**

Run:

```bash
git add rust/src/runner.rs
git commit -m "spike: add bounded worker queue primitives"
```

### Task 2: Switch `run_parallel()` To Fixed Workers

**Files:**
- Modify: `rust/src/runner.rs`
- Test: `rust/tests/trace_test.rs`

- [ ] **Step 1: Re-run the ordered-output trace regression before editing**

Run:

```bash
cargo test --test trace_test trace_reports_ordered_wait_for_blocked_repos -- --nocapture
```

Expected: PASS. This is the behavior that must stay stable after the worker-queue refactor.

- [ ] **Step 2: Replace the semaphore path with fixed worker threads**

Update `run_parallel()` so it builds a shared queue up front, spawns only `worker_thread_count(max_workers, repos.len())` threads, and keeps the receive-side ordered printer unchanged:

```rust
    let jobs = Arc::new(JobQueue::new(
        repos.iter()
            .enumerate()
            .map(|(idx, repo)| {
                let repo_path = repo.clone();
                let cmd = build_command(repo);
                QueuedJob::new(idx, repo_path.clone(), cmd)
            })
            .collect(),
    ));

    let worker_count = worker_thread_count(max_workers, repos.len());

    std::thread::scope(|s| -> Result<()> {
        for _ in 0..worker_count {
            let tx = tx.clone();
            let jobs = Arc::clone(&jobs);

            s.spawn(move || {
                while let Some(job) = jobs.next() {
                    let start_ms = if trace_enabled {
                        Some(run_started_at.elapsed().as_millis())
                    } else {
                        None
                    };

                    let spawn_result = job.cmd.spawn(url_scheme);
                    let spawn_ms = if trace_enabled {
                        Some(run_started_at.elapsed().as_millis())
                    } else {
                        None
                    };

                    let result = match spawn_result {
                        Ok(child) => child.wait_with_output(),
                        Err(err) => Err(err),
                    };

                    let trace_sample = if trace_enabled {
                        let exit_ms = run_started_at.elapsed().as_millis();
                        Some(match &result {
                            Ok(output) => RepoTraceSample {
                                start_ms: start_ms.expect("trace enabled start_ms"),
                                spawn_ms: spawn_ms.expect("trace enabled spawn_ms"),
                                exit_ms,
                                stdout_bytes: output.stdout.len(),
                                stderr_bytes: output.stderr.len(),
                                success: output.status.success(),
                            },
                            Err(_) => RepoTraceSample {
                                start_ms: start_ms.expect("trace enabled start_ms"),
                                spawn_ms: spawn_ms.expect("trace enabled spawn_ms"),
                                exit_ms,
                                stdout_bytes: 0,
                                stderr_bytes: 0,
                                success: false,
                            },
                        })
                    } else {
                        None
                    };

                    let _ = tx.send((job.idx, job.repo_path, result, trace_sample));
                }
            });
        }
        drop(tx);

        // Keep the existing ordered receive-side print loop here.
        for (idx, repo, result, trace_sample) in rx {
            // existing print + trace logic
        }
        Ok(())
    })?;
```

Also remove the `Semaphore` type and its related setup from `runner.rs`.

- [ ] **Step 3: Run the focused regressions and the full Rust suite**

Run:

```bash
cargo test --test trace_test trace_reports_ordered_wait_for_blocked_repos -- --nocapture
script/test -t rust
```

Expected: PASS. Ordered output should remain sorted and the full Rust suite should stay green.

- [ ] **Step 4: Commit the worker-queue runner change**

Run:

```bash
git add rust/src/runner.rs
git commit -m "spike: use a bounded worker queue"
```

### Task 3: Capture Benchmark And Trace Results

**Files:**
- Modify: `docs/plans/output-spike-tracker.md`

- [ ] **Step 1: Build the Rust binary for the spike branch**

Run:

```bash
script/build -t rust
```

Expected: PASS and `./bin/git-all-rust` points at the release build for `spike/bounded-worker-queue`.

- [ ] **Step 2: Capture a trace run on `~/work`**

Run:

```bash
cd ~/work
GIT_ALL_TRACE_FILE=/tmp/git-all-spike3.trace /Users/rsanheim/src/rsanheim/git-all/bin/git-all-rust -n 8 status > /tmp/git-all-spike3.stdout
rg 'phase=summary' /tmp/git-all-spike3.trace
```

Expected: a single `phase=summary` line with `first_exit_ms`, `first_print_ms`, `delayed_repos`, `max_ordered_wait_ms`, and `total_ms`.

- [ ] **Step 3: Run the branch-to-branch benchmark against the baseline**

Run:

```bash
cd /Users/rsanheim/src/rsanheim/git-all
script/bench git -I rust -b 0c6137c -t spike/bounded-worker-queue -d ~/work -c status -n 8
```

Expected: `hyperfine` output comparing `0c6137c` against `spike/bounded-worker-queue`.

- [ ] **Step 4: Record the results in the tracker doc**

Update the `Spike 3` row in `docs/plans/output-spike-tracker.md` by replacing the empty metric cells with the measured values from `/tmp/git-all-spike3.trace` and replacing the note cell with a one-line summary of the `script/bench git` result.

- [ ] **Step 5: Commit the recorded results**

Run:

```bash
git add docs/plans/output-spike-tracker.md
git commit -m "docs: record bounded worker queue results"
```
