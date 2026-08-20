#!/bin/sh
# Update an existing toktally install by refreshing the source tree and
# re-running install.sh.
#
#   ./scripts/update.sh
#   PREFIX=$HOME/.local ./scripts/update.sh
set -eu

usage() {
    cat <<'EOF'
Usage: update.sh [options]

Pull the source checkout (unless TOKTALLY_SKIP_PULL=1) and reinstall
into the same prefix.

  --prefix DIR     Install prefix (default: $PREFIX or last install.conf)
  --src DIR        Source checkout (default: last install.conf or detect)
  --skip-build     Passed through to install.sh
  -h, --help       Show this help
EOF
}

die() {
    echo "update.sh: $*" >&2
    exit 1
}

is_checkout() {
    [ -f "$1/Cargo.toml" ] && [ -d "$1/crates/cli" ] && [ -d "$1/plugins" ]
}

PREFIX="${PREFIX:-}"
SRC="${TOKTALLY_SRC:-${TOKEN_USAGE_SRC:-}}"
SKIP_BUILD="${TOKTALLY_SKIP_BUILD:-${TOKEN_USAGE_SKIP_BUILD:-}}"
INSTALL_ARGS=""

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix)
            [ $# -ge 2 ] || die "missing value for --prefix"
            PREFIX="$2"
            shift 2
            ;;
        --prefix=*)
            PREFIX="${1#*=}"
            shift
            ;;
        --src)
            [ $# -ge 2 ] || die "missing value for --src"
            SRC="$2"
            shift 2
            ;;
        --src=*)
            SRC="${1#*=}"
            shift
            ;;
        --skip-build)
            SKIP_BUILD=1
            shift
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            die "unknown option: $1"
            ;;
    esac
done

if [ -z "$PREFIX" ]; then
    if [ -f "${HOME}/.local/share/toktally/install.conf" ]; then
        PREFIX=$(sed -n 's/^PREFIX=//p' "${HOME}/.local/share/toktally/install.conf" | head -1)
    else
        PREFIX="${HOME}/.local"
    fi
fi

CONF="${PREFIX}/share/toktally/install.conf"
if [ -z "$SRC" ] && [ -f "$CONF" ]; then
    SRC=$(sed -n 's/^SRC=//p' "$CONF" | head -1)
fi

if [ -z "$SRC" ]; then
    script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" 2>/dev/null && pwd) || script_dir=""
    if [ -n "$script_dir" ] && is_checkout "$(dirname -- "$script_dir")"; then
        SRC=$(dirname -- "$script_dir")
    fi
fi

[ -n "$SRC" ] || die "cannot find a source checkout; pass --src or TOKTALLY_SRC"

if [ -z "${TOKTALLY_SKIP_PULL:-${TOKEN_USAGE_SKIP_PULL:-}}" ] && [ -d "$SRC/.git" ]; then
    command -v git >/dev/null 2>&1 || die "git is required to update $SRC"
    if git -C "$SRC" remote get-url origin >/dev/null 2>&1; then
        git -C "$SRC" pull --ff-only
    fi
fi

INSTALLER=""
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" 2>/dev/null && pwd) || script_dir=""
if [ -n "$script_dir" ] && [ -x "$script_dir/install.sh" ]; then
    INSTALLER="$script_dir/install.sh"
elif [ -x "${PREFIX}/share/toktally/install.sh" ]; then
    INSTALLER="${PREFIX}/share/toktally/install.sh"
elif [ -x "$SRC/scripts/install.sh" ]; then
    INSTALLER="$SRC/scripts/install.sh"
else
    die "cannot find install.sh"
fi

if [ -n "$SKIP_BUILD" ]; then
    INSTALL_ARGS="$INSTALL_ARGS --skip-build"
fi

# shellcheck disable=SC2086
exec "$INSTALLER" --prefix "$PREFIX" --src "$SRC" $INSTALL_ARGS
