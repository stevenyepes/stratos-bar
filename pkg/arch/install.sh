#!/bin/bash

# Exit on error
set -e

echo "Installing dependencies..."
# Ensure base-devel is installed (needed for makepkg)
# This might ask for sudo password
# sudo pacman -S --needed base-devel git webkit2gtk-4.1 gtk3 libappindicator-gtk3

echo "Building and installing StratosBar..."
makepkg -si
