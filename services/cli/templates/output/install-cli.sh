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

set -u

APP_NAME=bencher
APP_VERSION="${BENCHER_VERSION:-0.5.3}"
ARTIFACT_DOWNLOAD_URL="${INSTALLER_DOWNLOAD_URL:-https://bencher.dev/download/$APP_VERSION}"
PRINT_VERBOSE=${INSTALLER_PRINT_VERBOSE:-0}
PRINT_QUIET=${INSTALLER_PRINT_QUIET:-0}
NO_MODIFY_PATH=${INSTALLER_NO_MODIFY_PATH:-0}

# glibc provided by our Ubuntu 22.04 runners;
# in the future, we should actually record which glibc was on the runner,
# and inject that into the script.
BUILDER_GLIBC_MAJOR="2"
BUILDER_GLIBC_SERIES="35"

usage() {
    # print help (this cat/EOF stuff is a "heredoc" string)
    cat <<EOF
install-cli.sh

Bencher CLI v$APP_VERSION Installer

This script detects what platform you're on and fetches an appropriate archive from https://bencher.dev.
It then unpacks the binaries and installs them to \$CARGO_HOME/bin (\$HOME/.cargo/bin).
If \$CARGO_HOME does not exist, it falls back to creating it.
It will then add that dir to PATH by adding the appropriate line to your shell profiles.

USAGE:
    install-cli.sh [OPTIONS]

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
    need_cmd tar
    need_cmd which
    need_cmd grep
    need_cmd cat

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
                            # user wants to skip the prompt --
                            # we don't need /dev/tty
                            PRINT_VERBOSE=1
                            ;;
                        q)
                            # user wants to skip the prompt --
                            # we don't need /dev/tty
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
    local _zip_ext
    local _bins
    local _bin

    # Lookup what to download/unpack based on platform
    case "$_arch" in 
        "x86_64-unknown-linux-gnu")
            _artifact_name="$APP_NAME-v${APP_VERSION}-linux-x86-64"
            _zip_ext=""
            _bins="bencher"
            _bin="bencher"
            ;;
        "aarch64-unknown-linux-gnu")
            _artifact_name="$APP_NAME-v${APP_VERSION}-linux-arm-64"
            _zip_ext=""
            _bins="bencher"
            _bin="bencher"
            ;;
        "x86_64-apple-darwin")
            _artifact_name="$APP_NAME-v${APP_VERSION}-macos-x86-64"
            _zip_ext=""
            _bins="bencher"
            _bin="bencher"
            ;;
        "aarch64-apple-darwin")
            _artifact_name="$APP_NAME-v${APP_VERSION}-macos-arm-64"
            _zip_ext=""
            _bins="bencher"
            _bin="bencher"
            ;;
        *)
            err "There isn't a package for $_arch"
            err "If you would like for there to be, please open an issue on GitHub:"
            err "https://github.com/bencherdev/bencher/issues"
            ;;
    esac

    # download the archive
    local _url="$ARTIFACT_DOWNLOAD_URL/$_artifact_name"
    local _dir
    if ! _dir="$(ensure mktemp -d)"; then
        # Because the previous command ran in a subshell, we must manually
        # propagate exit status.
        exit 1
    fi
    local _file="$_dir/input$_zip_ext"

    say "Downloading Bencher CLI ($APP_NAME v$APP_VERSION)" 1>&2
    say_verbose "  From: $_url" 1>&2
    say_verbose "  To:   $_file" 1>&2

    ensure mkdir -p "$_dir"

    if ! downloader "$_url" "$_file"; then
      say "Failed to download Bencher CLI from $_url"
      say "This may be a network error, or it may be an issue on our end."
      say "If so please, open an issue on GitHub:"
      say "https://github.com/bencherdev/bencher/issues"
      exit 1
    fi

    # unpack the archive
    case "$_zip_ext" in
        ".zip")
            ensure unzip -q "$_file" -d "$_dir"
            ;;

        ".tar."*)
            ensure tar xf "$_file" --strip-components 1 -C "$_dir"
            ;;
        "")
            say_verbose "Installing single binary: $_bin"
            cp "$_file" "$_dir/$_bin"
            _bins="$_bin"
            ;;
        *)
            err "Unknown archive format: $_zip_ext"
            ;;
    esac

    install "$_dir" "$_bins" "$@"
    local _retval=$?

    ignore rm -rf "$_dir"

    return "$_retval"
}

# See discussion of late-bound vs early-bound for why we use single-quotes with env vars
# shellcheck disable=SC2016
install() {
    # This code needs to both compute certain paths for itself to write to, and
    # also write them to shell/rc files so that they can look them up to e.g.
    # add them to PATH. This requires an active distinction between paths
    # and expressions that can compute them.
    #
    # The distinction lies in when we want env-vars to be evaluated. For instance
    # if we determine that we want to install to $HOME/.myapp, which do we add
    # to e.g. $HOME/.profile:
    #
    # * early-bound: export PATH="/home/myuser/.myapp:$PATH"
    # * late-bound:  export PATH="$HOME/.myapp:$PATH"
    #
    # In this case most people would prefer the late-bound version, but in other
    # cases the early-bound version might be a better idea. In particular when using
    # other env-vars than $HOME, they are more likely to be only set temporarily
    # for the duration of this install script, so it's more advisable to erase their
    # existence with early-bounding.
    #
    # This distinction is handled by "double-quotes" (early) vs 'single-quotes' (late).
    #
    # This script has a few different variants, the most complex one being the
    # CARGO_HOME version which attempts to install things to Cargo's bin dir,
    # potentially setting up a minimal version if the user hasn't ever installed Cargo.
    #
    # In this case we need to:
    #
    # * Install to $HOME/.cargo/bin/
    # * Create a shell script at $HOME/.cargo/env that:
    #   * Checks if $HOME/.cargo/bin/ is on PATH
    #   * and if not prepends it to PATH
    # * Edits $HOME/.profile to run $HOME/.cargo/env (if the line doesn't exist)
    #
    # To do this we need these 4 values:

    # The actual path we're going to install to
    local _install_dir
    # Path to the an shell script that adds install_dir to PATH
    local _env_script_path
    # Potentially-late-bound version of install_dir to write env_script
    local _install_dir_expr
    # Potentially-late-bound version of env_script_path to write to rcfiles like $HOME/.profile
    local _env_script_path_expr

    # first try CARGO_HOME, then fallback to HOME
    if [ -n "${CARGO_HOME:-}" ]; then
        _install_dir="$CARGO_HOME/bin"
        _env_script_path="$CARGO_HOME/env"
        # If CARGO_HOME was set but it ended up being the default $HOME-based path,
        # then keep things late-bound. Otherwise bake the value for safety.
        # This is what rustup does, and accurately reproducing it is useful.
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
    say "Installing Bencher CLI to $_install_dir"
    ensure mkdir -p "$_install_dir"

    # copy all the binaries to the install dir
    local _src_dir="$1"
    local _bins="$2"
    for _bin_name in $_bins; do
        local _bin="$_src_dir/$_bin_name"
        ensure cp "$_bin" "$_install_dir"
        # unzip seems to need this chmod
        ensure chmod +x "$_install_dir/$_bin_name"
        say_verbose "Installed: $_bin_name"
    done

    say "🐰 Bencher CLI installed!"

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
        # zsh has a special ZDOTDIR directory, which if set
        # should be considered instead of $HOME
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
    # Edit rcfiles ($HOME/.profile) to add install_dir to $PATH
    #
    # We do this slightly indirectly by creating an "env" shell script which checks if install_dir
    # is on $PATH already, and prepends it if not. The actual line we then add to rcfiles
    # is to just source that script. This allows us to blast it into lots of different rcfiles and
    # have it run multiple times without causing problems. It's also specifically compatible
    # with the system rustup uses, so that we don't conflict with it.
    local _install_dir_expr="$1"
    local _env_script_path="$2"
    local _env_script_path_expr="$3"
    local _rcfiles="$4"

    if [ -n "${HOME:-}" ]; then
        local _target
        local _home

        # Find the first file in the array that exists and choose
        # that as our target to write to
        for _rcfile_relative in $_rcfiles; do
            _home="$(print_home_for_script "$_rcfile_relative")"
            local _rcfile="$_home/$_rcfile_relative"

            if [ -f "$_rcfile" ]; then
                _target="$_rcfile"
                break
            fi
        done

        # If we didn't find anything, pick the first entry in the
        # list as the default to create and write to
        if [ -z "${_target:-}" ]; then
            local _rcfile_relative
            _rcfile_relative="$(echo "$_rcfiles" | awk '{ print $1 }')"
            _home="$(print_home_for_script "$_rcfile_relative")"
            _target="$_home/$_rcfile_relative"
        fi

        # `source x` is an alias for `. x`, and the latter is more portable/actually-posix.
        # This apparently comes up a lot on freebsd. It's easy enough to always add
        # the more robust line to rcfiles, but when telling the user to apply the change
        # to their current shell ". x" is pretty easy to misread/miscopy, so we use the
        # prettier "source x" line there. Hopefully people with Weird Shells are aware
        # this is a thing and know to tweak it (or just restart their shell).
        local _robust_line=". \"$_env_script_path_expr\""
        local _pretty_line="source \"$_env_script_path_expr\""

        # Add the env script if it doesn't already exist
        if [ ! -f "$_env_script_path" ]; then
            say_verbose "creating $_env_script_path"
            write_env_script "$_install_dir_expr" "$_env_script_path"
        else
            say_verbose "$_env_script_path already exists"
        fi

        # Check if the line is already in the rcfile
        # grep: 0 if matched, 1 if no match, and 2 if an error occurred
        #
        # Ideally we could use quiet grep (-q), but that makes "match" and "error"
        # have the same behaviour, when we want "no match" and "error" to be the same
        # (on error we want to create the file, which >> conveniently does)
        #
        # We search for both kinds of line here just to do the right thing in more cases.
        if ! grep -F "$_robust_line" "$_target" > /dev/null 2>/dev/null && \
           ! grep -F "$_pretty_line" "$_target" > /dev/null 2>/dev/null
        then
            # If the script now exists, add the line to source it to the rcfile
            # (This will also create the rcfile if it doesn't exist)
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
    # write this env script to the given path (this cat/EOF stuff is a "heredoc" string)
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

check_proc() {
    # Check for /proc by looking for the /proc/self/exe link
    # This is only run on Linux
    if ! test -L /proc/self/exe ; then
        err "proc not found: Unable to find /proc/self/exe.  Is /proc mounted?  Installation cannot proceed without /proc."
    fi
}

get_bitness() {
    need_cmd head
    # Architecture detection without dependencies beyond coreutils.
    # ELF files start out "\x7fELF", and the following byte is
    #   0x01 for 32-bit and
    #   0x02 for 64-bit.
    # The printf builtin on some shells like dash only supports octal
    # escape sequences, so we use those.
    local _current_exe_head
    _current_exe_head=$(head -c 5 /proc/self/exe )
    if [ "$_current_exe_head" = "$(printf '\177ELF\001')" ]; then
        echo 32
    elif [ "$_current_exe_head" = "$(printf '\177ELF\002')" ]; then
        echo 64
    else
        err "Unknown platform bitness"
    fi
}

is_host_amd64_elf() {
    need_cmd head
    need_cmd tail
    # ELF e_machine detection without dependencies beyond coreutils.
    # Two-byte field at offset 0x12 indicates the CPU,
    # but we're interested in it being 0x3E to indicate amd64, or not that.
    local _current_exe_machine
    _current_exe_machine=$(head -c 19 /proc/self/exe | tail -c 1)
    [ "$_current_exe_machine" = "$(printf '\076')" ]
}

get_endianness() {
    local cputype=$1
    local suffix_eb=$2
    local suffix_el=$3

    # detect endianness without od/hexdump, like get_bitness() does.
    need_cmd head
    need_cmd tail

    local _current_exe_endianness
    _current_exe_endianness="$(head -c 6 /proc/self/exe | tail -c 1)"
    if [ "$_current_exe_endianness" = "$(printf '\001')" ]; then
        echo "${cputype}${suffix_el}"
    elif [ "$_current_exe_endianness" = "$(printf '\002')" ]; then
        echo "${cputype}${suffix_eb}"
    else
        err "Unknown platform endianness"
    fi
}

get_architecture() {
    local _ostype
    local _cputype
    _ostype="$(uname -s)"
    _cputype="$(uname -m)"
    local _clibtype="gnu"
    local _local_glibc

    if [ "$_ostype" = Linux ]; then
        if [ "$(uname -o)" = Android ]; then
            _ostype=Android
        fi
        if ldd --version 2>&1 | grep -q 'musl'; then
            _clibtype="musl-dynamic"
        # glibc, but is it a compatible glibc?
        else
            # Parsing version out from line 1 like:
            # ldd (Ubuntu GLIBC 2.35-0ubuntu3.1) 2.35
            _local_glibc="$(ldd --version | awk -F' ' '{ if (FNR<=1) print $NF }')"

            if [ "$(echo "${_local_glibc}" | awk -F. '{ print $1 }')" = $BUILDER_GLIBC_MAJOR ] && [ "$(echo "${_local_glibc}" | awk -F. '{ print $2 }')" -ge $BUILDER_GLIBC_SERIES ]; then
                _clibtype="gnu"
            else
                _clibtype="musl-static"
            fi
        fi
    fi

    if [ "$_ostype" = Darwin ] && [ "$_cputype" = i386 ]; then
        # Darwin `uname -m` lies
        if sysctl hw.optional.x86_64 | grep -q ': 1'; then
            _cputype=x86_64
        fi
    fi

    if [ "$_ostype" = SunOS ]; then
        # Both Solaris and illumos presently announce as "SunOS" in "uname -s"
        # so use "uname -o" to disambiguate.  We use the full path to the
        # system uname in case the user has coreutils uname first in PATH,
        # which has historically sometimes printed the wrong value here.
        if [ "$(/usr/bin/uname -o)" = illumos ]; then
            _ostype=illumos
        fi

        # illumos systems have multi-arch userlands, and "uname -m" reports the
        # machine hardware name; e.g., "i86pc" on both 32- and 64-bit x86
        # systems.  Check for the native (widest) instruction set on the
        # running kernel:
        if [ "$_cputype" = i86pc ]; then
            _cputype="$(isainfo -n)"
        fi
    fi

    case "$_ostype" in

        Android)
            _ostype=linux-android
            ;;

        Linux)
            check_proc
            _ostype=unknown-linux-$_clibtype
            _bitness=$(get_bitness)
            ;;

        FreeBSD)
            _ostype=unknown-freebsd
            ;;

        NetBSD)
            _ostype=unknown-netbsd
            ;;

        DragonFly)
            _ostype=unknown-dragonfly
            ;;

        Darwin)
            _ostype=apple-darwin
            ;;

        illumos)
            _ostype=unknown-illumos
            ;;

        MINGW* | MSYS* | CYGWIN* | Windows_NT)
            _ostype=pc-windows-gnu
            ;;

        *)
            err "Unrecognized OS type: $_ostype"
            ;;

    esac

    case "$_cputype" in

        i386 | i486 | i686 | i786 | x86)
            _cputype=i686
            ;;

        xscale | arm)
            _cputype=arm
            if [ "$_ostype" = "linux-android" ]; then
                _ostype=linux-androideabi
            fi
            ;;

        armv6l)
            _cputype=arm
            if [ "$_ostype" = "linux-android" ]; then
                _ostype=linux-androideabi
            else
                _ostype="${_ostype}eabihf"
            fi
            ;;

        armv7l | armv8l)
            _cputype=armv7
            if [ "$_ostype" = "linux-android" ]; then
                _ostype=linux-androideabi
            else
                _ostype="${_ostype}eabihf"
            fi
            ;;

        aarch64 | arm64)
            _cputype=aarch64
            ;;

        x86_64 | x86-64 | x64 | amd64)
            _cputype=x86_64
            ;;

        mips)
            _cputype=$(get_endianness mips '' el)
            ;;

        mips64)
            if [ "$_bitness" -eq 64 ]; then
                # only n64 ABI is supported for now
                _ostype="${_ostype}abi64"
                _cputype=$(get_endianness mips64 '' el)
            fi
            ;;

        ppc)
            _cputype=powerpc
            ;;

        ppc64)
            _cputype=powerpc64
            ;;

        ppc64le)
            _cputype=powerpc64le
            ;;

        s390x)
            _cputype=s390x
            ;;
        riscv64)
            _cputype=riscv64gc
            ;;
        loongarch64)
            _cputype=loongarch64
            ;;
        *)
            err "Unknown CPU type: $_cputype"

    esac

    # Detect 64-bit linux with 32-bit userland
    if [ "${_ostype}" = unknown-linux-gnu ] && [ "${_bitness}" -eq 32 ]; then
        case $_cputype in
            x86_64)
                # 32-bit executable for amd64 = x32
                if is_host_amd64_elf; then {
                    err "Unsuppored Userland: Linux x32"
                }; else
                    _cputype=i686
                fi
                ;;
            mips64)
                _cputype=$(get_endianness mips '' el)
                ;;
            powerpc64)
                _cputype=powerpc
                ;;
            aarch64)
                _cputype=armv7
                if [ "$_ostype" = "linux-android" ]; then
                    _ostype=linux-androideabi
                else
                    _ostype="${_ostype}eabihf"
                fi
                ;;
            riscv64gc)
                err "Unsuppored Userland: riscv64 32-bit"
                ;;
        esac
    fi

    # treat armv7 systems without neon as plain arm
    if [ "$_ostype" = "unknown-linux-gnueabihf" ] && [ "$_cputype" = armv7 ]; then
        if ensure grep '^Features' /proc/cpuinfo | grep -q -v neon; then
            # At least one processor does not have NEON.
            _cputype=arm
        fi
    fi

    _arch="${_cputype}-${_ostype}"

    RETVAL="$_arch"
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

# Run a command that should never fail. If the command fails execution
# will immediately terminate with an error showing the failing
# command.
ensure() {
    if ! "$@"; then err "Command failed: $*"; fi
}

# This is just for indicating that commands' results are being
# intentionally ignored. Usually, because it's being executed
# as part of error handling.
ignore() {
    "$@"
}

# This wraps curl or wget. Try curl first, if not installed,
# use wget instead.
downloader() {
    if check_cmd curl
    then _dld=curl
    elif check_cmd wget
    then _dld=wget
    else _dld='curl or wget' # to be used in error message of need_cmd
    fi

    if [ "$1" = --check ]
    then need_cmd "$_dld"
    elif [ "$_dld" = curl ]
    then curl -sSfL "$1" -o "$2"
    elif [ "$_dld" = wget ]
    then wget "$1" -O "$2"
    else err "Unknown downloader"   # should not reach here
    fi
}

download_binary_and_run_installer "$@" || exit 1
