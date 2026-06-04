#!/bin/bash
# Build and install stratos-bar from source via makepkg.
# Performs pre-flight checks for required tools and packages so failures
# happen in seconds, not 5 minutes into a Tauri build.

set -e

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

green "pre-flight OK. Building..."
exec makepkg -si
