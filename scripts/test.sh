#!/bin/bash
set -euo pipefail

if [ "${1:-}" != "--linux-only" ]; then
    RUST_BACKTRACE=1 cargo nextest run --all-features --no-capture --profile ci
    RUST_BACKTRACE=1 cargo test --doc --all-features
fi

# If not on Linux, cross-compile tests for crates with
# cfg(target_os = "linux") gated code that would otherwise be skipped.
# The binaries cannot run on a non-Linux host, so we only build them (--no-run).
if [ "$(uname -s)" != "Linux" ]; then
    LINUX_TARGET="x86_64-unknown-linux-gnu"
    LINUX_PACKAGES=(-p bencher_init -p bencher_rootfs -p bencher_runner -p bencher_runner_cli)

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
        echo "Warning: skipping cross-target test build for Linux (no cross-compiler found)"
        echo "  Install zig or a GCC cross-compiler to enable this check"
        exit 0
    fi

    # Ensure the Rust target is installed
    if ! rustup target list --installed | grep -q "$LINUX_TARGET"; then
        rustup target add "$LINUX_TARGET"
    fi

    # Override the mold linker from .cargo/config.toml since mold is not
    # available on macOS. Use the cross-compiler directly as the linker.
    CC_x86_64_unknown_linux_gnu="$CROSS_CC" \
    AR_x86_64_unknown_linux_gnu="$CROSS_AR" \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="$CROSS_CC" \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-Csymbol-mangling-version=v0" \
    RUST_BACKTRACE=1 cargo test \
        "${LINUX_PACKAGES[@]}" \
        --target "$LINUX_TARGET" \
        --all-features \
        --no-run

    if [ -n "$CLEANUP" ]; then
        rm -f "$CLEANUP"
    fi
fi
