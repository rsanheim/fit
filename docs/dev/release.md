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

Remaining tasks to complete Homebrew distribution:

* [ ] Create `rsanheim/homebrew-tap` repository on GitHub
* [ ] Push homebrew-tap initial commit
* [ ] Merge `release-workflow` branch in fit repo
* [ ] Create first release (`script/release 0.3.0` or bump to 0.4.0)
* [ ] Update tap with real SHA256 hashes (`script/update-homebrew`)
* [ ] Test installation: `brew tap rsanheim/tap && brew install fit`
* [ ] Add LICENSE file to fit repo (MIT)
