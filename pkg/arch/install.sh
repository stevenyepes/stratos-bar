#!/bin/bash
# Build and install stratos-bar from source via makepkg.
# Performs pre-flight checks for required tools and packages so failures
# happen in seconds, not 5 minutes into a Tauri build.

set -e

# makepkg reads PKGBUILD from the working directory, and the documented
# invocation is `./pkg/arch/install.sh` from the repo root -- which would look
# for a PKGBUILD that is not there. Anchor to this script's own directory so the
# script works from anywhere.
cd "$(dirname "$(readlink -f "$0")")"

red()    { printf '\033[31m%s\033[0m\n' "$*"; }
green()  { printf '\033[32m%s\033[0m\n' "$*"; }
yellow() { printf '\033[33m%s\033[0m\n' "$*"; }

missing_pkgs=()
for cmd in makepkg pacman npm cargo node git; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        red "missing command: $cmd"
        exit 1
    fi
done

# Runtime libs the built binary links against.
for pkg in webkit2gtk-4.1 gtk3 libappindicator-gtk3; do
    if ! pacman -Qi "$pkg" &>/dev/null; then
        missing_pkgs+=("$pkg")
    fi
done

if [ ${#missing_pkgs[@]} -gt 0 ]; then
    yellow "missing runtime packages: ${missing_pkgs[*]}"
    yellow "install with: sudo pacman -S --needed ${missing_pkgs[*]}"
    exit 1
fi

# makepkg keeps its bare clone and extracted working copy between runs, and a
# working copy follows refs/remotes/origin/HEAD -- a ref git writes once, at
# clone time, that no later fetch updates. One first created while the repo sat
# on a topic branch keeps building that branch after it is merged and deleted:
# it fetches the new commits, ignores them, and the test suite passes because it
# runs against the stale tree. Pinning a #branch fragment in the PKGBUILD would
# fix it here and break CI's detached pull_request checkout, so the guarantee is
# made by always starting from a fresh clone. These are build outputs only; all
# three are gitignored.
rm -rf ./stratos-bar ./src ./pkg

green "pre-flight OK. Building..."
exec makepkg -si
