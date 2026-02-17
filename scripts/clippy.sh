#!/bin/bash
set -euo pipefail

cargo clippy --no-deps --all-targets --all-features -- -Dwarnings

# If not on Linux, re-run clippy targeting Linux for crates with
# cfg(target_os = "linux") gated code that would otherwise be skipped.
if [ "$(uname -s)" != "Linux" ]; then
    LINUX_TARGET="x86_64-unknown-linux-gnu"
    LINUX_PACKAGES=(-p bencher_runner -p bencher_rootfs)

    # A C cross-compiler is needed for transitive native dependencies (e.g. ring).
    CROSS_CC=""
    CROSS_AR=""
    CLEANUP=""
    if command -v x86_64-linux-gnu-gcc &>/dev/null; then
        CROSS_CC="x86_64-linux-gnu-gcc"
        CROSS_AR="x86_64-linux-gnu-ar"
    elif command -v x86_64-unknown-linux-gnu-gcc &>/dev/null; then
        CROSS_CC="x86_64-unknown-linux-gnu-gcc"
        CROSS_AR="x86_64-unknown-linux-gnu-ar"
    elif command -v zig &>/dev/null; then
        # zig uses x86_64-linux-gnu (no "unknown"), but the cc crate adds
        # --target=x86_64-unknown-linux-gnu which zig can't parse.
        # A wrapper script filters out the conflicting flag.
        ZIG_CC_WRAPPER=$(mktemp "${TMPDIR:-/tmp}/zig-cc-wrapper.XXXXXX")
        cat > "$ZIG_CC_WRAPPER" << 'WRAPPER'
#!/bin/bash
args=()
for arg in "$@"; do
    case "$arg" in
        --target=*) ;;
        *) args+=("$arg") ;;
    esac
done
exec zig cc -target x86_64-linux-gnu "${args[@]}"
WRAPPER
        chmod +x "$ZIG_CC_WRAPPER"
        CLEANUP="$ZIG_CC_WRAPPER"
        CROSS_CC="$ZIG_CC_WRAPPER"
        CROSS_AR="zig ar"
    fi

    if [ -z "$CROSS_CC" ]; then
        echo "Warning: skipping cross-target clippy for Linux (no cross-compiler found)"
        echo "  Install zig or a GCC cross-compiler to enable this check"
        exit 0
    fi

    # Ensure the Rust target is installed
    if ! rustup target list --installed | grep -q "$LINUX_TARGET"; then
        rustup target add "$LINUX_TARGET"
    fi

    CC_x86_64_unknown_linux_gnu="$CROSS_CC" \
    AR_x86_64_unknown_linux_gnu="$CROSS_AR" \
    cargo clippy \
        "${LINUX_PACKAGES[@]}" \
        --target "$LINUX_TARGET" \
        --no-deps \
        --all-targets \
        --all-features \
        -- -Dwarnings

    if [ -n "$CLEANUP" ]; then
        rm -f "$CLEANUP"
    fi
fi
