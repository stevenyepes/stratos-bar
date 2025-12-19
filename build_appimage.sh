#!/bin/bash

# Build the AppImage with the necessary environment variables
echo "Building StratosBar AppImage..."

# NO_STRIP=true is required to prevent linuxdeploy from failing on some systems
NO_STRIP=true npm run tauri build

if [ $? -eq 0 ]; then
    echo "----------------------------------------------------------------"
    echo "Build Successful!"
    echo "AppImage location:"
    echo "./src-tauri/target/release/bundle/appimage/stratos-bar_0.1.0_amd64.AppImage"
    echo "----------------------------------------------------------------"
else
    echo "Build Failed!"
    exit 1
fi
