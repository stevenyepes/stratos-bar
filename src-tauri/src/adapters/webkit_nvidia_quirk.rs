/// Resolved GPU vendor for the current machine. Built by a detector from
/// sysfs; the decision function only ever sees this enum, never raw sysfs data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuVendor {
    Nvidia,
    Other,
}

/// Resolved windowing session type. Built by a detector from
/// GDK_BACKEND / XDG_SESSION_TYPE / WAYLAND_DISPLAY / DISPLAY; the decision
/// function only ever sees this enum, never raw env strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionType {
    X11,
    Wayland,
    Unknown,
}

/// Fully-resolved inputs to the quirk decision. Every field here is the
/// output of some detector, never read live inside `decide_quirk`.
#[derive(Debug, Clone, Copy)]
pub struct QuirkInput {
    pub gpu_vendor: GpuVendor,
    pub session_type: SessionType,
    /// True when HYPRLAND_INSTANCE_SIGNATURE (or equivalent) indicates Hyprland.
    pub is_hyprland: bool,
    /// True when the dma-buf based egl-wayland2 path is available
    /// (NVIDIA driver >= 560). Only meaningful when gpu_vendor == Nvidia
    /// and session_type == Wayland; the decision function does not care
    /// how this was derived.
    pub egl_wayland2_present: bool,
}

/// What `main()` should do to its own process environment as a result of
/// the decision. Exactly one variant is ever produced by `decide_quirk`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuirkAction {
    None,
    SetVar {
        key: &'static str,
        value: &'static str,
    },
}

/// Pure decision function: NVIDIA? -> session type -> (Wayland: Hyprland? -> egl-wayland2? -> else).
pub fn decide_quirk(input: &QuirkInput) -> QuirkAction {
    if input.gpu_vendor != GpuVendor::Nvidia {
        return QuirkAction::None;
    }

    match input.session_type {
        SessionType::X11 => QuirkAction::SetVar {
            key: "WEBKIT_DISABLE_DMABUF_RENDERER",
            value: "1",
        },
        SessionType::Wayland => {
            if input.is_hyprland {
                QuirkAction::SetVar {
                    key: "WEBKIT_DISABLE_DMABUF_RENDERER",
                    value: "1",
                }
            } else if input.egl_wayland2_present {
                QuirkAction::None
            } else {
                QuirkAction::SetVar {
                    key: "__NV_DISABLE_EXPLICIT_SYNC",
                    value: "1",
                }
            }
        }
        SessionType::Unknown => QuirkAction::None,
    }
}

/// Scans `/sys/class/drm/*` for an NVIDIA GPU. Fails open to `GpuVendor::Other`
/// on any I/O error or unexpected sysfs layout.
fn detect_gpu_vendor() -> GpuVendor {
    let entries = match std::fs::read_dir("/sys/class/drm") {
        Ok(entries) => entries,
        Err(_) => return GpuVendor::Other,
    };

    for entry in entries.flatten() {
        let device_dir = entry.path().join("device");

        let vendor_is_nvidia = std::fs::read_to_string(device_dir.join("vendor"))
            .map(|s| s.trim().eq_ignore_ascii_case("0x10de"))
            .unwrap_or(false);
        if !vendor_is_nvidia {
            continue;
        }

        let is_boot_gpu = std::fs::read_to_string(device_dir.join("boot_vga"))
            .or_else(|_| std::fs::read_to_string(device_dir.join("boot_display")))
            .map(|s| s.trim() == "1")
            .unwrap_or(false);

        let driver_is_nvidia = std::fs::read_link(device_dir.join("driver"))
            .ok()
            .and_then(|target| target.file_name().map(|f| f.to_string_lossy().into_owned()))
            .map(|name| name == "nvidia")
            .unwrap_or(false);

        if is_boot_gpu || driver_is_nvidia {
            return GpuVendor::Nvidia;
        }
    }

    GpuVendor::Other
}

/// Resolves the windowing session type from environment variables, in order:
/// `GDK_BACKEND` -> `XDG_SESSION_TYPE` -> `WAYLAND_DISPLAY`/`DISPLAY` presence.
/// Fails open to `SessionType::Unknown` if none resolve.
fn detect_session_type() -> SessionType {
    if let Ok(backend) = std::env::var("GDK_BACKEND") {
        let backend = backend.to_ascii_lowercase();
        if backend.contains("wayland") {
            return SessionType::Wayland;
        }
        if backend.contains("x11") {
            return SessionType::X11;
        }
    }

    if let Ok(session_type) = std::env::var("XDG_SESSION_TYPE") {
        let session_type = session_type.to_ascii_lowercase();
        if session_type == "wayland" {
            return SessionType::Wayland;
        }
        if session_type == "x11" {
            return SessionType::X11;
        }
    }

    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        return SessionType::Wayland;
    }
    if std::env::var("DISPLAY").is_ok() {
        return SessionType::X11;
    }

    SessionType::Unknown
}

/// True when `HYPRLAND_INSTANCE_SIGNATURE` is set, consistent with the
/// Hyprland check in `adapters/linux_window_service.rs`.
fn detect_is_hyprland() -> bool {
    std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()
}

/// True when `/sys/module/nvidia/version` parses to a driver major version
/// of 560 or higher (the dma-buf based egl-wayland2 path). Fails open to
/// `false` on any I/O error or unparseable content.
fn detect_egl_wayland2_present() -> bool {
    let contents = match std::fs::read_to_string("/sys/module/nvidia/version") {
        Ok(contents) => contents,
        Err(_) => return false,
    };

    let major_version = contents
        .split_whitespace()
        .next()
        .and_then(|token| token.split('.').next())
        .and_then(|major| major.parse::<u32>().ok());

    matches!(major_version, Some(version) if version >= 560)
}

/// Resolves all real-world inputs to `decide_quirk`. The only function in
/// this module (besides `apply_quirk`) permitted to touch real sysfs/env.
pub fn detect_quirk_input() -> QuirkInput {
    QuirkInput {
        gpu_vendor: detect_gpu_vendor(),
        session_type: detect_session_type(),
        is_hyprland: detect_is_hyprland(),
        egl_wayland2_present: detect_egl_wayland2_present(),
    }
}

/// Applies the decided `QuirkAction` to the live process environment. The
/// only function in this module permitted to call `std::env::set_var`.
pub fn apply_quirk(action: QuirkAction) {
    match action {
        QuirkAction::None => {}
        QuirkAction::SetVar { key, value } => {
            std::env::set_var(key, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_nvidia_sets_nothing() {
        let input = QuirkInput {
            gpu_vendor: GpuVendor::Other,
            session_type: SessionType::Wayland,
            is_hyprland: true,
            egl_wayland2_present: true,
        };
        assert_eq!(decide_quirk(&input), QuirkAction::None);
    }

    #[test]
    fn nvidia_x11_disables_dmabuf() {
        let input = QuirkInput {
            gpu_vendor: GpuVendor::Nvidia,
            session_type: SessionType::X11,
            is_hyprland: false,
            egl_wayland2_present: false,
        };
        assert_eq!(
            decide_quirk(&input),
            QuirkAction::SetVar {
                key: "WEBKIT_DISABLE_DMABUF_RENDERER",
                value: "1",
            }
        );
    }

    #[test]
    fn nvidia_wayland_hyprland_disables_dmabuf() {
        let input = QuirkInput {
            gpu_vendor: GpuVendor::Nvidia,
            session_type: SessionType::Wayland,
            is_hyprland: true,
            egl_wayland2_present: false,
        };
        assert_eq!(
            decide_quirk(&input),
            QuirkAction::SetVar {
                key: "WEBKIT_DISABLE_DMABUF_RENDERER",
                value: "1",
            }
        );
    }

    #[test]
    fn nvidia_wayland_egl_wayland2_sets_nothing() {
        let input = QuirkInput {
            gpu_vendor: GpuVendor::Nvidia,
            session_type: SessionType::Wayland,
            is_hyprland: false,
            egl_wayland2_present: true,
        };
        assert_eq!(decide_quirk(&input), QuirkAction::None);
    }

    #[test]
    fn nvidia_wayland_other_disables_explicit_sync() {
        let input = QuirkInput {
            gpu_vendor: GpuVendor::Nvidia,
            session_type: SessionType::Wayland,
            is_hyprland: false,
            egl_wayland2_present: false,
        };
        assert_eq!(
            decide_quirk(&input),
            QuirkAction::SetVar {
                key: "__NV_DISABLE_EXPLICIT_SYNC",
                value: "1",
            }
        );
    }

    #[test]
    fn nvidia_wayland_hyprland_and_egl_wayland2_prefers_dmabuf() {
        let input = QuirkInput {
            gpu_vendor: GpuVendor::Nvidia,
            session_type: SessionType::Wayland,
            is_hyprland: true,
            egl_wayland2_present: true,
        };
        assert_eq!(
            decide_quirk(&input),
            QuirkAction::SetVar {
                key: "WEBKIT_DISABLE_DMABUF_RENDERER",
                value: "1",
            }
        );
    }
}
