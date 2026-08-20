#!/bin/sh
# Install toktally and toktally-api.
#
# From a checkout:
#   ./scripts/install.sh
#   PREFIX=$HOME/.local ./scripts/install.sh
#
# From the web:
#   curl -fsSL https://raw.githubusercontent.com/mintychochip/token-usage/master/scripts/install.sh | sh
set -eu

REPO_URL="${TOKTALLY_REPO:-${TOKEN_USAGE_REPO:-https://github.com/mintychochip/token-usage.git}}"

usage() {
    cat <<'EOF'
Usage: install.sh [options]

Install toktally and toktally-api into PREFIX/bin, and copy
host wrappers to PREFIX/share/toktally/plugins.

  --prefix DIR     Install prefix (default: $HOME/.local, or $PREFIX)
  --src DIR        Source checkout to build (default: detect or PREFIX/src/toktally)
  --skip-build     Copy binaries from TOKTALLY_BIN_DIR instead of cargo
  -h, --help       Show this help

Environment:
  PREFIX, TOKTALLY_SRC, TOKTALLY_BIN_DIR, TOKTALLY_SKIP_BUILD,
  TOKTALLY_REPO
EOF
}

die() {
    echo "install.sh: $*" >&2
    exit 1
}

is_checkout() {
    [ -f "$1/Cargo.toml" ] && [ -d "$1/crates/cli" ] && [ -d "$1/plugins" ]
}

PREFIX="${PREFIX:-${HOME}/.local}"
SRC="${TOKTALLY_SRC:-${TOKEN_USAGE_SRC:-}}"
SKIP_BUILD="${TOKTALLY_SKIP_BUILD:-${TOKEN_USAGE_SKIP_BUILD:-}}"

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

[ -n "$PREFIX" ] || die "PREFIX is empty"
BINDIR="${PREFIX}/bin"
SHAREDIR="${PREFIX}/share/toktally"

resolve_src() {
    if [ -n "$SRC" ]; then
        echo "$SRC"
        return
    fi
    if is_checkout "$(pwd)"; then
        pwd
        return
    fi
    script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" 2>/dev/null && pwd) || script_dir=""
    if [ -n "$script_dir" ] && is_checkout "$(dirname -- "$script_dir")"; then
        dirname -- "$script_dir"
        return
    fi
    echo "${PREFIX}/src/toktally"
}

SRC=$(resolve_src)

ensure_src() {
    if is_checkout "$SRC"; then
        return
    fi
    command -v git >/dev/null 2>&1 || die "git is required to fetch $REPO_URL"
    mkdir -p "$(dirname -- "$SRC")"
    if [ -d "$SRC/.git" ]; then
        git -C "$SRC" fetch --quiet origin
        git -C "$SRC" pull --ff-only --quiet
    else
        git clone --depth 1 "$REPO_URL" "$SRC"
    fi
    is_checkout "$SRC" || die "clone at $SRC is not a toktally checkout"
}

ensure_src

BIN_DIR="${TOKTALLY_BIN_DIR:-${TOKEN_USAGE_BIN_DIR:-}}"
if [ -z "$SKIP_BUILD" ]; then
    command -v cargo >/dev/null 2>&1 || die "cargo is required (https://rustup.rs)"
    (cd "$SRC" && cargo build --release -p toktally-cli --bins)
    BIN_DIR="${SRC}/target/release"
else
    [ -n "$BIN_DIR" ] || die "TOKTALLY_SKIP_BUILD requires TOKTALLY_BIN_DIR"
fi

[ -x "$BIN_DIR/toktally" ] || die "missing $BIN_DIR/toktally"
[ -x "$BIN_DIR/toktally-api" ] || die "missing $BIN_DIR/toktally-api"

mkdir -p "$BINDIR" "$SHAREDIR"
cp "$BIN_DIR/toktally" "$BINDIR/toktally"
cp "$BIN_DIR/toktally-api" "$BINDIR/toktally-api"
chmod 0755 "$BINDIR/toktally" "$BINDIR/toktally-api"

rm -rf "$SHAREDIR/plugins"
cp -R "$SRC/plugins" "$SHAREDIR/plugins"
cp "$SRC/scripts/install.sh" "$SHAREDIR/install.sh"
cp "$SRC/scripts/update.sh" "$SHAREDIR/update.sh"
chmod 0755 "$SHAREDIR/install.sh" "$SHAREDIR/update.sh"

{
    printf 'PREFIX=%s\n' "$PREFIX"
    printf 'SRC=%s\n' "$SRC"
} >"$SHAREDIR/install.conf"

echo "installed toktally and toktally-api to $BINDIR"
echo "plugins copied to $SHAREDIR/plugins"
case ":$PATH:" in
    *":$BINDIR:"*) ;;
    *)
        echo "add $BINDIR to PATH to run the tools from any directory"
        ;;
esac
