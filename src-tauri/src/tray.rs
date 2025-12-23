use crate::utils::toggle_main_window;

// Reuse PaletteTray as is.
pub struct PaletteTray {
    pub handle: tauri::AppHandle,
}

impl ksni::Tray for PaletteTray {
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let mut icons = Vec::new();

        let sizes = vec![
            (include_bytes!("../icons/tray-icon-22.png").to_vec(), 22),
            (include_bytes!("../icons/tray-icon-32.png").to_vec(), 32),
            (include_bytes!("../icons/tray-icon-48.png").to_vec(), 48),
        ];

        for (data, _expected_size) in sizes {
            if let Ok(image) = image::load_from_memory(&data) {
                let rgba = image.to_rgba8();
                let width = rgba.width() as i32;
                let height = rgba.height() as i32;
                let raw_rgba = rgba.into_raw();

                let mut argb = Vec::with_capacity(raw_rgba.len());
                for chunk in raw_rgba.chunks(4) {
                    if chunk.len() == 4 {
                        argb.push(chunk[3]); // A
                        argb.push(chunk[0]); // R
                        argb.push(chunk[1]); // G
                        argb.push(chunk[2]); // B
                    }
                }

                icons.push(ksni::Icon {
                    width,
                    height,
                    data: argb,
                });
            }
        }

        if !icons.is_empty() {
            icons
        } else {
            if let Some(img) = self.handle.default_window_icon() {
                vec![ksni::Icon {
                    width: img.width() as i32,
                    height: img.height() as i32,
                    data: img.rgba().to_vec(),
                }]
            } else {
                vec![]
            }
        }
    }

    fn id(&self) -> String {
        "stratos-bar".to_string()
    }

    fn title(&self) -> String {
        "stratos-bar".to_string()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        toggle_main_window(&self.handle);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: "Show".into(),
                activate: Box::new(|this: &mut Self| {
                    toggle_main_window(&this.handle);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|this: &mut Self| {
                    this.handle.exit(0);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}
