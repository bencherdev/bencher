#!/bin/sh
# shellcheck shell=dash
#
# Copyright © 2022-present Bencher
# Licensed under the Apache License, Version 2.0 (https://opensource.org/licenses/Apache-2.0/)
# or MIT License (https://opensource.org/licenses/MIT/) at your discretion.

if [ "$KSH_VERSION" = 'Version JM 93t+ 2010-03-05' ]; then
    # The version of ksh93 that ships with many illumos systems does not
    # support the "local" extension.  Print a message rather than fail in
    # subtle ways later on:
    echo 'this installer does not work with this ksh93 version; please try bash!' >&2
    exit 1
fi

set -uo pipefail

APP_NAME=runner
APP_VERSION="${BENCHER_RUNNER_VERSION:-0.5.10}"
ARTIFACT_DOWNLOAD_URL="${INSTALLER_DOWNLOAD_URL:-https://bencher.dev/download/$APP_VERSION}"
PRINT_VERBOSE=${INSTALLER_PRINT_VERBOSE:-0}
PRINT_QUIET=${INSTALLER_PRINT_QUIET:-0}
NO_MODIFY_PATH=${INSTALLER_NO_MODIFY_PATH:-0}

usage() {
    # print help (this cat/EOF stuff is a "heredoc" string)
    cat <<EOF
install-runner.sh

Bencher Runner v$APP_VERSION Installer

This script detects what platform you're on and fetches an appropriate binary from https://bencher.dev.
It installs the binary to \$CARGO_HOME/bin (\$HOME/.cargo/bin).
If \$CARGO_HOME does not exist, it falls back to creating it.
It will then add that dir to PATH by adding the appropriate line to your shell profiles.

NOTE: Bencher Runner only supports Linux (x86_64 and aarch64).

USAGE:
    install-runner.sh [OPTIONS]

OPTIONS:
    -v, --verbose
            Enable verbose output

    -q, --quiet
            Disable progress output

        --no-modify-path
            Don't configure the PATH environment variable

    -h, --help
            Print help information
EOF
}

download_binary_and_run_installer() {
    downloader --check
    need_cmd uname
    need_cmd mktemp
    need_cmd chmod
    need_cmd mkdir
    need_cmd rm
    need_cmd cp

    for arg in "$@"; do
        case "$arg" in
            --help)
                usage
                exit 0
                ;;
            --quiet)
                PRINT_QUIET=1
                ;;
            --verbose)
                PRINT_VERBOSE=1
                ;;
            --no-modify-path)
                NO_MODIFY_PATH=1
                ;;
            *)
                OPTIND=1
                if [ "${arg%%--*}" = "" ]; then
                    err "Unknown option: $arg"
                fi
                while getopts :hvq sub_arg "$arg"; do
                    case "$sub_arg" in
                        h)
                            usage
                            exit 0
                            ;;
                        v)
                            PRINT_VERBOSE=1
                            ;;
                        q)
                            PRINT_QUIET=1
                            ;;
                        *)
                            err "Unknown option: -$OPTARG"
                            ;;
                        esac
                done
                ;;
        esac
    done

    get_architecture || return 1
    local _arch="$RETVAL"
    assert_nz "$_arch" "arch"

    local _artifact_name
    local _bin

    # Lookup what to download based on platform
    case "$_arch" in
        "x86_64")
            _artifact_name="$APP_NAME-v${APP_VERSION}-linux-x86-64"
            _bin="runner"
            ;;
        "aarch64")
            _artifact_name="$APP_NAME-v${APP_VERSION}-linux-arm-64"
            _bin="runner"
            ;;
        *)
            err "Bencher Runner only supports Linux x86_64 and aarch64"
            err "Detected architecture: $_arch"
            err "If you need support for another platform, please open an issue on GitHub:"
            err "https://github.com/bencherdev/bencher/issues"
            exit 1
            ;;
    esac

    # download the binary
    local _url="$ARTIFACT_DOWNLOAD_URL/$_artifact_name"
    local _dir
    if ! _dir="$(ensure mktemp -d)"; then
        exit 1
    fi
    local _file="$_dir/$_bin"

    say "Downloading Bencher Runner ($APP_NAME v$APP_VERSION)" 1>&2
    say_verbose "  From: $_url" 1>&2
    say_verbose "  To:   $_file" 1>&2

    ensure mkdir -p "$_dir"

    if ! downloader "$_url" "$_file"; then
      say "Failed to download Bencher Runner from $_url"
      say "This may be a network error, or it may be an issue on our end."
      say "If so please, open an issue on GitHub:"
      say "https://github.com/bencherdev/bencher/issues"
      exit 1
    fi

    verify_checksum "$_url" "$_file"

    install "$_dir" "$_bin" "$@"
    local _retval=$?

    ignore rm -rf "$_dir"

    return "$_retval"
}

# See discussion of late-bound vs early-bound for why we use single-quotes with env vars
# shellcheck disable=SC2016
install() {
    local _install_dir
    local _env_script_path
    local _install_dir_expr
    local _env_script_path_expr

    # first try CARGO_HOME, then fallback to HOME
    if [ -n "${CARGO_HOME:-}" ]; then
        _install_dir="$CARGO_HOME/bin"
        _env_script_path="$CARGO_HOME/env"
        if [ -n "${HOME:-}" ]; then
            if [ "$HOME/.cargo/bin" = "$_install_dir" ]; then
                _install_dir_expr='$HOME/.cargo/bin'
                _env_script_path_expr='$HOME/.cargo/env'
            else
                _install_dir_expr="$_install_dir"
                _env_script_path_expr="$_env_script_path"
            fi
        else
            _install_dir_expr="$_install_dir"
            _env_script_path_expr="$_env_script_path"
        fi
    elif [ -n "${HOME:-}" ]; then
        _install_dir="$HOME/.cargo/bin"
        _env_script_path="$HOME/.cargo/env"
        _install_dir_expr='$HOME/.cargo/bin'
        _env_script_path_expr='$HOME/.cargo/env'
    else
        err "Home not found: Could not find your CARGO_HOME or HOME dir to install binaries"
    fi
    say "Installing Bencher Runner to $_install_dir"
    ensure mkdir -p "$_install_dir"

    # copy the binary to the install dir
    local _src_dir="$1"
    local _bin_name="$2"
    local _bin="$_src_dir/$_bin_name"
    ensure cp "$_bin" "$_install_dir"
    ensure chmod +x "$_install_dir/$_bin_name"
    say_verbose "Installed: $_bin_name"

    say "Bencher Runner installed!"

    if [ "0" = "$NO_MODIFY_PATH" ]; then
        add_install_dir_to_path "$_install_dir_expr" "$_env_script_path" "$_env_script_path_expr" ".profile"
        exit1=$?
        add_install_dir_to_path "$_install_dir_expr" "$_env_script_path" "$_env_script_path_expr" ".bash_profile .bash_login .bashrc"
        exit2=$?
        add_install_dir_to_path "$_install_dir_expr" "$_env_script_path" "$_env_script_path_expr" ".zshrc .zshenv"
        exit3=$?

        if [ "${exit1:-0}" = 1 ] || [ "${exit2:-0}" = 1 ] || [ "${exit3:-0}" = 1 ]; then
            say ""
            say "To add $_install_dir_expr to your PATH, either restart your shell or run:"
            say ""
            say "    source $_env_script_path_expr"
        fi
    fi
}

print_home_for_script() {
    local script="$1"

    local _home
    case "$script" in
        .zsh*)
            if [ -n "${ZDOTDIR:-}" ]; then
                _home="$ZDOTDIR"
            else
                _home="$HOME"
            fi
            ;;
        *)
            _home="$HOME"
            ;;
    esac

    echo "$_home"
}

add_install_dir_to_path() {
    local _install_dir_expr="$1"
    local _env_script_path="$2"
    local _env_script_path_expr="$3"
    local _rcfiles="$4"

    if [ -n "${HOME:-}" ]; then
        local _target
        local _home

        for _rcfile_relative in $_rcfiles; do
            _home="$(print_home_for_script "$_rcfile_relative")"
            local _rcfile="$_home/$_rcfile_relative"

            if [ -f "$_rcfile" ]; then
                _target="$_rcfile"
                break
            fi
        done

        if [ -z "${_target:-}" ]; then
            local _rcfile_relative
            _rcfile_relative="$(echo "$_rcfiles" | awk '{ print $1 }')"
            _home="$(print_home_for_script "$_rcfile_relative")"
            _target="$_home/$_rcfile_relative"
        fi

        local _robust_line=". \"$_env_script_path_expr\""
        local _pretty_line="source \"$_env_script_path_expr\""

        if [ ! -f "$_env_script_path" ]; then
            say_verbose "creating $_env_script_path"
            write_env_script "$_install_dir_expr" "$_env_script_path"
        else
            say_verbose "$_env_script_path already exists"
        fi

        if ! grep -F "$_robust_line" "$_target" > /dev/null 2>/dev/null && \
           ! grep -F "$_pretty_line" "$_target" > /dev/null 2>/dev/null
        then
            if [ -f "$_env_script_path" ]; then
                say_verbose "Adding $_robust_line to $_target"
                ensure echo "$_robust_line" >> "$_target"
                return 1
            fi
        else
            say_verbose "$_install_dir already in PATH"
        fi
    fi
}

write_env_script() {
    local _install_dir_expr="$1"
    local _env_script_path="$2"
    ensure cat <<EOF > "$_env_script_path"
#!/bin/sh
# add binaries to PATH if they aren't added yet
# affix colons on either side of \$PATH to simplify matching
case ":\${PATH}:" in
    *:"$_install_dir_expr":*)
        ;;
    *)
        # Prepending path in case a system-installed binary needs to be overridden
        export PATH="$_install_dir_expr:\$PATH"
        ;;
esac
EOF
}

get_architecture() {
    local _ostype
    local _cputype
    _ostype="$(uname -s)"
    _cputype="$(uname -m)"

    # Bencher Runner only supports Linux
    if [ "$_ostype" != "Linux" ]; then
        err "Bencher Runner only supports Linux"
        err "Detected OS: $_ostype"
        exit 1
    fi

    case "$_cputype" in
        x86_64 | x86-64 | x64 | amd64)
            _cputype=x86_64
            ;;
        aarch64 | arm64)
            _cputype=aarch64
            ;;
        *)
            err "Bencher Runner only supports x86_64 and aarch64"
            err "Detected CPU: $_cputype"
            exit 1
            ;;
    esac

    RETVAL="$_cputype"
}

say() {
    if [ "0" = "$PRINT_QUIET" ]; then
        echo "$1"
    fi
}

say_verbose() {
    if [ "1" = "$PRINT_VERBOSE" ]; then
        echo "$1"
    fi
}

err() {
    if [ "0" = "$PRINT_QUIET" ]; then
        local red
        local reset
        red=$(tput setaf 1 2>/dev/null || echo '')
        reset=$(tput sgr0 2>/dev/null || echo '')
        say "${red}ERROR${reset}: $1" >&2
    fi
    exit 1
}

need_cmd() {
    if ! check_cmd "$1"
    then err "Command not found: '$1'"
    fi
}

check_cmd() {
    command -v "$1" > /dev/null 2>&1
    return $?
}

assert_nz() {
    if [ -z "$1" ]; then err "assert_nz $2"; fi
}

ensure() {
    if ! "$@"; then err "Command failed: $*"; fi
}

ignore() {
    "$@"
}

verify_checksum() {
    local _url="$1"
    local _file="$2"
    local _checksum_url="${_url}.sha256"
    local _checksum_file
    _checksum_file="$(mktemp)" || return 0

    # Determine which sha256 tool is available
    local _sha_cmd=""
    if check_cmd sha256sum; then
        _sha_cmd="sha256sum"
    elif check_cmd shasum; then
        _sha_cmd="shasum -a 256"
    else
        say_verbose "No sha256sum or shasum found, skipping checksum verification"
        rm -f "$_checksum_file"
        return 0
    fi

    # Download the checksum file (best-effort)
    if ! downloader "$_checksum_url" "$_checksum_file" 2>/dev/null; then
        say_verbose "No checksum file available at $_checksum_url, skipping verification"
        rm -f "$_checksum_file"
        return 0
    fi

    local _expected
    _expected="$(awk '{print $1}' < "$_checksum_file")"
    rm -f "$_checksum_file"

    if [ -z "$_expected" ]; then
        say_verbose "Checksum file was empty, skipping verification"
        return 0
    fi

    local _actual
    _actual="$($_sha_cmd "$_file" | awk '{print $1}')"

    if [ "$_expected" != "$_actual" ]; then
        err "Checksum verification failed!
  Expected: $_expected
  Actual:   $_actual
  File:     $_file
  This could indicate a corrupted download or a tampered binary."
    fi

    say_verbose "Checksum verified: $_actual"
}

downloader() {
    if check_cmd curl
    then _dld=curl
    elif check_cmd wget
    then _dld=wget
    else _dld='curl or wget'
    fi

    if [ "$1" = --check ]
    then need_cmd "$_dld"
    elif [ "$_dld" = curl ]
    then curl -sSfL "$1" -o "$2"
    elif [ "$_dld" = wget ]
    then wget "$1" -O "$2"
    else err "Unknown downloader"
    fi
}

download_binary_and_run_installer "$@" || exit 1
