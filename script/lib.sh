# Shared library for nit benchmark scripts
# Source this file: source "$(dirname "$0")/lib.sh"

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${SCRIPT_DIR}/../bin"

# Hyperfine defaults
WARMUP=2
MIN_RUNS=5

# Discover all executable nit implementations (full paths)
discover_implementations() {
    local impls=()
    for impl in "${BIN_DIR}"/nit-*; do
        [[ -x "$impl" ]] && impls+=("$impl")
    done
    echo "${impls[@]}"
}

# Discover implementation names (e.g., "rust", "zig")
discover_impl_names() {
    for impl in "${BIN_DIR}"/nit-*; do
        [[ -x "$impl" ]] && basename "$impl" | sed 's/nit-//'
    done
}

# Show help message
# Usage: show_help "script-name" "description" "usage text"
show_help() {
    local script_name="$1"
    local description="$2"
    local usage="$3"
    echo "$script_name - $description"
    echo
    echo "Usage: $usage"
}
