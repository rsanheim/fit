# Shared library for fit scripts
# Source this file: source "$(dirname "$0")/lib.sh"

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
BIN_DIR="${FIT_ROOT}/bin"

# Discover all executable fit implementations (full paths)
discover_implementations() {
    local impls=()
    for impl in "${BIN_DIR}"/fit-*; do
        [[ -x "$impl" ]] && impls+=("$impl")
    done
    echo "${impls[@]}"
}

# Discover implementation names (e.g., "rust", "zig")
discover_impl_names() {
    for impl in "${BIN_DIR}"/fit-*; do
        [[ -x "$impl" ]] && basename "$impl" | sed 's/fit-//'
    done
}

# Discover implementation directories (full paths to fit-* dirs)
discover_impl_dirs() {
    for dir in "${FIT_ROOT}/fit-"*; do
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
    echo "${FIT_ROOT}/fit-${impl}"
}

# Get binary output path for implementation type (after release build)
get_binary_path() {
    local impl="$1"
    case "$impl" in
        rust)    echo "${FIT_ROOT}/fit-rust/target/release/fit" ;;
        zig)     echo "${FIT_ROOT}/fit-zig/zig-out/bin/fit" ;;
        crystal) echo "${FIT_ROOT}/fit-crystal/bin/fit" ;;
        *)       return 1 ;;
    esac
}
