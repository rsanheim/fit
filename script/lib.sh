# Shared library for git-all scripts
# Source this file: source "$(dirname "$0")/lib.sh"

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GIT_ALL_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
BIN_DIR="${GIT_ALL_ROOT}/bin"

# Discover all executable git-all implementations (full paths)
discover_implementations() {
    local impls=()
    for impl in "${BIN_DIR}"/git-all-*; do
        [[ -x "$impl" ]] && impls+=("$impl")
    done
    echo "${impls[@]}"
}

# Discover implementation names (e.g., "rust", "zig")
discover_impl_names() {
    for impl in "${BIN_DIR}"/git-all-*; do
        [[ -x "$impl" ]] && basename "$impl" | sed 's/git-all-//'
    done
}

# Discover implementation directories (full paths to rust/, zig/, crystal/ dirs)
discover_impl_dirs() {
    for lang in rust zig crystal; do
        local dir="${GIT_ALL_ROOT}/${lang}"
        [[ -d "$dir" ]] && echo "$dir"
    done
}

# Get build command for implementation type
# Usage: get_build_cmd <impl> [dev]
# If second arg is "dev", returns debug/dev build command
get_build_cmd() {
    local impl="$1"
    local mode="${2:-release}"
    case "$impl" in
        rust)
            if [[ "$mode" == "dev" ]]; then
                echo "cargo build"
            else
                echo "cargo build --release"
            fi
            ;;
        zig)
            if [[ "$mode" == "dev" ]]; then
                echo "zig build"
            else
                echo "zig build --release=fast"
            fi
            ;;
        crystal)
            if [[ "$mode" == "dev" ]]; then
                echo "shards build"
            else
                echo "shards build --release"
            fi
            ;;
        *)       return 1 ;;
    esac
}

# Get test command for implementation type
get_test_cmd() {
    local impl="$1"
    case "$impl" in
        rust)    echo "cargo test" ;;
        zig)     echo "zig build test" ;;
        crystal) echo "crystal spec" ;;
        *)       return 1 ;;
    esac
}

# Get implementation directory for type
get_impl_dir() {
    local impl="$1"
    echo "${GIT_ALL_ROOT}/${impl}"
}

# Get binary output path for implementation type (after release build)
get_binary_path() {
    local impl="$1"
    case "$impl" in
        rust)    echo "${GIT_ALL_ROOT}/rust/target/release/git-all" ;;
        zig)     echo "${GIT_ALL_ROOT}/zig/zig-out/bin/git-all" ;;
        crystal) echo "${GIT_ALL_ROOT}/crystal/bin/git-all" ;;
        *)       return 1 ;;
    esac
}
