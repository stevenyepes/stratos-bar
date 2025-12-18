# Build Instructions

## Prerequisites
- Node.js and npm
- Rust and Cargo
- Tauri CLI

## Building the AppImage

To build the AppImage for Linux, use the following command. Note that `NO_STRIP=true` is required to avoid issues with `linuxdeploy` on some systems.

```bash
NO_STRIP=true npm run tauri build
```

## Troubleshooting

### Empty Window / Black Screen
If you experience an empty or black window on launch, it is likely a WebKitGTK hardware acceleration issue.
This project includes a fix in `src-tauri/src/main.rs` that automatically sets `WEBKIT_DISABLE_DMABUF_RENDERER=1`.

### Build Failures
If the build fails during the bundling step with errors about `strip`, ensure you are using the `NO_STRIP=true` environment variable as shown above.
