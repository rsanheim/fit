# Shared library for nit benchmark scripts
# Source this file: source "$(dirname "$0")/lib.sh"

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${SCRIPT_DIR}/../bin"

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
