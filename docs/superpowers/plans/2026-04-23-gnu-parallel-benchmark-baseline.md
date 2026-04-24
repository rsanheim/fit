# GNU Parallel Benchmark Baseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `gnu-parallel` shell baseline as a first-class `git-all-*` executable so `script/bench compare` benchmarks it alongside the language implementations.

**Architecture:** Add a new `bin/git-all-gnu-parallel` wrapper that accepts the benchmark-relevant CLI shape, discovers depth-1 repos, and executes `git` through GNU Parallel. Add a guarded self-test mode in `script/bench` so shell-level behavior can be verified without needing a separate shell test framework.

**Tech Stack:** Bash, GNU Parallel, existing `script/bench` discovery logic, `hyperfine`.

---

### Task 1: Add A Failing Benchmark Self-Test

**Files:**
- Modify: `script/bench`

- [ ] **Step 1: Write the failing test**

Add a guard near the bottom of `script/bench` that runs only when `GIT_ALL_BENCH_SELFTEST=1`. The test should assert both that `discover_impl_names` includes `gnu-parallel` and that the new wrapper emits a deterministic command preview in its own self-test mode.

```bash
if [[ "${GIT_ALL_BENCH_SELFTEST:-}" == "1" ]]; then
    mapfile -t SELFTEST_IMPLS < <(discover_impl_names)
    [[ " ${SELFTEST_IMPLS[*]} " == *" gnu-parallel "* ]] || {
        echo "selftest: expected gnu-parallel implementation discovery" >&2
        exit 1
    }

    STATUS_CMD=$(
        GIT_ALL_GNU_PARALLEL_SELFTEST=1 \
            "${BIN_DIR}/git-all-gnu-parallel" -n 4 status
    )
    [[ "$STATUS_CMD" == *"parallel"* ]] || {
        echo "selftest: expected GNU Parallel command preview for status" >&2
        exit 1
    }
    [[ "$STATUS_CMD" == *"--no-optional-locks status"* ]] || {
        echo "selftest: expected status to use --no-optional-locks" >&2
        exit 1
    }

    FETCH_CMD=$(
        GIT_ALL_GNU_PARALLEL_SELFTEST=1 \
            "${BIN_DIR}/git-all-gnu-parallel" -n 8 fetch
    )
    [[ "$FETCH_CMD" == *"parallel"* ]] || {
        echo "selftest: expected GNU Parallel command preview for fetch" >&2
        exit 1
    }
    [[ "$FETCH_CMD" == *"git -C "{}" fetch"* ]] || {
        echo "selftest: expected fetch preview to run git -C per repo" >&2
        exit 1
    }
    [[ "$FETCH_CMD" != *"--no-optional-locks fetch"* ]] || {
        echo "selftest: fetch should not force --no-optional-locks" >&2
        exit 1
    }

    echo "selftest: ok"
    exit 0
fi
```

- [ ] **Step 2: Run test to verify it fails**

Run: `GIT_ALL_BENCH_SELFTEST=1 script/bench compare`
Expected: FAIL because `bin/git-all-gnu-parallel` does not exist yet and discovery does not include `gnu-parallel`.

### Task 2: Implement The GNU Parallel Baseline Wrapper

**Files:**
- Create: `bin/git-all-gnu-parallel`

- [ ] **Step 1: Write minimal implementation**

Create a Bash wrapper that:
- Parses `-n|--workers`
- Treats remaining args as the git subcommand and its arguments
- Uses pass-through `git "$@"` when running inside a git repository
- Otherwise discovers depth-1 repos by finding `.git` directories one level below the current directory
- Uses `parallel -j "$workers"` to run `git -C "$repo" ...` once per repo
- Uses `--no-optional-locks` only for `status`
- Prints the constructed `parallel` command and exits when `GIT_ALL_GNU_PARALLEL_SELFTEST=1`

```bash
#!/usr/bin/env bash
set -euo pipefail

workers=8

while [[ $# -gt 0 ]]; do
    case "$1" in
        -n|--workers)
            workers="$2"
            shift 2
            ;;
        --)
            shift
            break
            ;;
        *)
            break
            ;;
    esac
done

[[ $# -gt 0 ]] || {
    echo "usage: git-all-gnu-parallel [-n workers] <git-command> [args...]" >&2
    exit 1
}

git_args=("$@")

if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    exec git "${git_args[@]}"
fi

git_prefix=(git -C "{}")
if [[ "${git_args[0]}" == "status" ]]; then
    git_prefix+=(--no-optional-locks)
fi

parallel_cmd=(parallel --will-cite -j "$workers" "${git_prefix[*]} ${git_args[*]}")

if [[ "${GIT_ALL_GNU_PARALLEL_SELFTEST:-}" == "1" ]]; then
    printf '%s\n' "${parallel_cmd[*]}"
    exit 0
fi

find . -mindepth 2 -maxdepth 2 -type d -name .git -print0 |
    parallel -0 --will-cite -j "$workers" 'git -C "{//}" '"${git_args[*]}"
```

- [ ] **Step 2: Run test to verify it passes**

Run: `GIT_ALL_BENCH_SELFTEST=1 script/bench compare`
Expected: PASS with `selftest: ok`

### Task 3: Document The Baseline

**Files:**
- Modify: `docs/dev/benchmarks.md`

- [ ] **Step 1: Update benchmark docs**

Add a short note that `bench compare` now includes a GNU Parallel shell baseline when `bin/git-all-gnu-parallel` is executable.

```markdown
## Baseline

`script/bench compare` also benchmarks `bin/git-all-gnu-parallel`, a shell baseline that discovers the same depth-1 repos and runs one `git -C <repo>` command per repository through GNU Parallel.
```

- [ ] **Step 2: Verify docs and wrapper together**

Run: `GIT_ALL_BENCH_SELFTEST=1 script/bench compare`
Expected: PASS with `selftest: ok`
