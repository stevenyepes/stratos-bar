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
If you experience an empty or black window on launch, it is likely a WebKitGTK hardware acceleration issue affecting NVIDIA GPUs.
`src-tauri/src/adapters/webkit_nvidia_quirk.rs` detects your GPU vendor and session type at startup and conditionally sets one environment variable before WebKit initializes, following this matrix:

| GPU | Session | Hyprland? | egl-wayland2 present (driver ≥ 560)? | Action |
|---|---|---|---|---|
| Non-NVIDIA | any | — | — | none |
| NVIDIA | X11 | — | — | `WEBKIT_DISABLE_DMABUF_RENDERER=1` |
| NVIDIA | Wayland | yes | — | `WEBKIT_DISABLE_DMABUF_RENDERER=1` |
| NVIDIA | Wayland | no | yes | none |
| NVIDIA | Wayland | no | no | `__NV_DISABLE_EXPLICIT_SYNC=1` |
| NVIDIA | session type undetermined | — | — | none |

Detection fails open: if the GPU vendor or session type can't be determined, no variable is set, so a misdetection never makes things worse than the unpatched behavior.

This is a permanent workaround, not a stopgap awaiting an upstream fix. [WebKit bug 262607](https://bugs.webkit.org/show_bug.cgi?id=262607), which proposed handling this NVIDIA/dma-buf interaction upstream, is RESOLVED WONTFIX. Don't delete this detection as obsolete — there is no upstream change coming that would make it unnecessary.

If the detection gets it wrong on your machine — you still see a black window, or the app sets a variable your setup doesn't need — export the relevant variable yourself before launching (`WEBKIT_DISABLE_DMABUF_RENDERER=1` or `__NV_DISABLE_EXPLICIT_SYNC=1`, depending on which path matches your setup) and file an issue with your GPU and session details so the matrix can be corrected.

### Build Failures
If the build fails during the bundling step with errors about `strip`, ensure you are using the `NO_STRIP=true` environment variable as shown above.
