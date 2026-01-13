# Release Process

## Overview

fit is distributed via Homebrew tap using prebuilt binaries. This approach is language-agnostic - the formula downloads architecture-specific binaries regardless of whether they were built with Rust, Zig, or Crystal.

## Architecture

```
rsanheim/fit                    rsanheim/homebrew-tap
├── .github/workflows/          ├── Formula/
│   └── release.yml  ──────────►│   └── fit.rb
└── script/                     └── .github/workflows/
    ├── release                     └── lint.yml
    └── update-homebrew
```

**Artifact naming** (language-agnostic):
```
fit-{version}-darwin-arm64.tar.gz
fit-{version}-darwin-x86_64.tar.gz
```

## Release Steps

### 1. Create Release

From the fit repo:

```bash
# Dry-run first
script/release --dry-run 0.4.0

# Create the release
script/release 0.4.0
```

This will:
* Update version in `fit-rust/Cargo.toml`
* Commit the version bump
* Create and push a `v0.4.0` tag
* GitHub Actions builds binaries and creates a GitHub Release

### 2. Update Homebrew Tap

After GitHub Actions completes (~5 min):

```bash
# Dry-run to see SHA256 hashes
script/update-homebrew --dry-run 0.4.0

# Update the formula
script/update-homebrew 0.4.0

# Commit and push the tap
cd ~/src/rsanheim/homebrew-tap
git diff  # verify changes
git add -A && git commit -m "fit 0.4.0" && git push
```

### 3. Verify Installation

```bash
brew update
brew upgrade fit  # or: brew install rsanheim/tap/fit
fit --version
```

## Local Testing

Before releasing, test the formula locally:

```bash
cd ~/src/rsanheim/homebrew-tap
brew audit --strict Formula/fit.rb
brew style Formula/fit.rb
```

## Changing Implementation Language

When switching from Rust to Zig (or Crystal):

| Component | Changes | No Changes |
|-----------|---------|------------|
| release.yml | `cargo build` → `zig build` | Artifact names, upload step |
| script/release | Cargo.toml → build.zig | Tag/push logic |
| Formula/fit.rb | None | Downloads same artifacts |

The formula never knows what language built the binary.

## TODO

### Make fit repo public

#### Audit for sensitive content

**Secrets and Credentials:**
* [x] Search for API keys, tokens, passwords: `git log -p --all -S "password\|secret\|token\|api_key\|credential"`
* [x] Check for AWS/GCP/Azure credential patterns (e.g., `AKIA`, `azure_`, service account JSON)
* [x] Verify no `.env` files in history: `git log --all --diff-filter=A --name-only | grep -i env`
* [x] Search for private key headers: `git log -p --all -S "BEGIN.*PRIVATE KEY"`

**Personal Information:**
* [x] Search for hardcoded home paths: `git log -p --all -S "/Users/\|/home/"`
* [x] Check for email addresses in code (not commits): `grep -r "@" --include="*.rs" --include="*.zig" --include="*.cr"`
* [x] Look for internal hostnames or IP addresses

**Private Dependencies:**
* [x] Verify Cargo.toml uses only public crates (no private git URLs)
* [x] Verify shard.yml uses only public shards
* [x] Check for private git URLs in build configs (build.zig.zon, etc.)

**Configuration Files:**
* [x] Confirm `.gitignore` covers local config files
* [x] Add `.claude/settings.local.json` to `.gitignore`
* [x] Verify no IDE configs with personal paths are tracked

#### Review git history

**File History Analysis:**
* [x] List deleted files: `git log --all --diff-filter=D --name-only --oneline`
* [x] Check for sensitive file patterns (`.env`, `credentials`, `secrets`, `*.pem`)
* [x] Verify no config files with secrets were ever tracked

**Commit Message Review:**
* [x] Scan messages for sensitive keywords: `git log --all --oneline | grep -iE "secret|password|token|key"`
* [x] Check for internal project or private repo references

**Binary Artifacts:**

Early commits include `nit-crystal/bin/nit` (543KB) and `nit-crystal/bin/nit.dwarf` (1MB) - ~1.5MB total bloat.

*Decision: Skip cleanup* - The cost outweighs the benefit:
* 13 PRs exist (11 merged, 2 open) - rewriting would orphan all commit references
* Open PRs (#3, #13) would break and need manual rebasing
* 1.5MB is negligible for a CLI tool repo
* No security risk - binaries contain no sensitive data

If cleanup is ever needed, use [git-filter-repo](https://github.com/newren/git-filter-repo) (the modern, git-recommended replacement for BFG):

```bash
# Install
brew install git-filter-repo

# Create fresh mirror clone (required)
git clone --mirror git@github.com:rsanheim/fit.git fit-cleanup
cd fit-cleanup

# Remove the files
git filter-repo --invert-paths \
  --path nit-crystal/bin/nit \
  --path nit-crystal/bin/nit.dwarf

# Re-add origin (filter-repo removes it) and force push
git remote add origin git@github.com:rsanheim/fit.git
git push origin --force --all
git push origin --force --tags
```

Warning: This rewrites all commit SHAs, breaks PR references, and requires all clones to be re-fetched.

#### Finalize for public release

* [ ] Add LICENSE file (MIT)
* [ ] Review and clean up documentation
* [ ] Update README with project overview and usage
* [ ] Make repo public on GitHub

### Homebrew distribution

* [ ] Create `rsanheim/homebrew-tap` repository on GitHub
* [ ] Push homebrew-tap initial commit
* [ ] Merge `release-workflow` branch in fit repo
* [ ] Create first release (`script/release 0.3.0` or bump to 0.4.0)
* [ ] Update tap with real SHA256 hashes (`script/update-homebrew`)
* [ ] Test installation: `brew tap rsanheim/tap && brew install fit`
