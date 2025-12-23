use walkdir::WalkDir;

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
pub async fn open_entity(path: String) -> Result<(), String> {
    std::thread::spawn(move || {
        let _ = open::that(path);
    });
    Ok(())
}

#[derive(serde::Serialize)]
pub struct FileMetadata {
    size: u64,
    created: Option<u64>,
    is_dir: bool,
    readonly: bool,
    mime_type: Option<String>,
}

#[tauri::command]
pub async fn search_files(query: String, path: String) -> Result<Vec<String>, String> {
    let lower_query = query.to_lowercase();

    // Priority extensions
    let priority_exts = vec![
        "mp4", "mkv", "avi", "mov", "webm", // Video
        "png", "jpg", "jpeg", "webp", "gif", "svg", // Image
        "rs", "js", "ts", "vue", "py", "html", "css", "json", "md", "txt", "pdf",
        "csv", // Code/Doc
    ];

    let walker = WalkDir::new(&path).max_depth(10).into_iter();

    // Collect matches
    let mut matches = Vec::new();

    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path();
        let path_str = path.to_string_lossy();

        if path_str.to_lowercase().contains(&lower_query) {
            // Simple scoring
            let mut score = 0;
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                let lower_name = name.to_lowercase();
                if lower_name.starts_with(&lower_query) {
                    score += 50;
                }
                if lower_name == lower_query {
                    score += 100;
                }

                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if priority_exts.contains(&ext.to_lowercase().as_str()) {
                        score += 20;
                    }
                }
            }

            matches.push((path_str.to_string(), score));
            if matches.len() > 200 {
                break;
            } // Hard limit scan
        }
    }

    // Sort by score descending
    matches.sort_by(|a, b| b.1.cmp(&a.1));

    // Take top 50
    let results: Vec<String> = matches.into_iter().take(50).map(|(p, _)| p).collect();

    Ok(results)
}

#[tauri::command]
pub async fn read_file_preview(path: String, max_bytes: Option<usize>) -> Result<String, String> {
    use std::io::Read;
    let limit = max_bytes.unwrap_or(2048);
    let mut file = std::fs::File::open(&path).map_err(|e| e.to_string())?;

    let mut buffer = vec![0; limit];
    let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
    buffer.truncate(n);

    // Check for null bytes to detect binary
    if buffer.contains(&0) {
        return Err("Binary file detected".to_string());
    }

    String::from_utf8(buffer).map_err(|e| format!("Not valid UTF-8: {}", e))
}

#[tauri::command]
pub async fn get_file_metadata(path: String) -> Result<FileMetadata, String> {
    let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;

    let created = metadata
        .created()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs());

    Ok(FileMetadata {
        size: metadata.len(),
        created,
        is_dir: metadata.is_dir(),
        readonly: metadata.permissions().readonly(),
        mime_type: None, // Could use `mime_guess` crate if added, but frontend handles mostly
    })
}

#[tauri::command]
pub async fn get_selection_context() -> Result<String, String> {
    use arboard::Clipboard;

    if let Ok(mut clipboard) = Clipboard::new() {
        if let Ok(text) = clipboard.get_text() {
            if !text.trim().is_empty() {
                return Ok(text);
            }
        }
    }

    Err("No text found in selection or clipboard".to_string())
}

#[tauri::command]
pub async fn copy_to_clipboard(text: String) -> Result<(), String> {
    use arboard::Clipboard;

    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn check_is_executable(path: String) -> Result<bool, String> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Ok(false);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = p.metadata().map_err(|e| e.to_string())?;
        return Ok(metadata.permissions().mode() & 0o111 != 0);
    }
    #[cfg(not(unix))]
    {
        Ok(true) // Assume executable on non-unix for now or just return true
    }
}

#[tauri::command]
pub async fn make_file_executable(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err("File not found".to_string());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = p.metadata().map_err(|e| e.to_string())?;
        let mut perms = metadata.permissions();
        perms.set_mode(perms.mode() | 0o111);
        std::fs::set_permissions(p, perms).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[tokio::test]
    async fn test_read_file_preview_text() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("test_preview.txt");
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, "Hello, World!").unwrap();

        let content = read_file_preview(file_path.to_string_lossy().to_string(), Some(1024))
            .await
            .unwrap();
        assert!(content.contains("Hello, World!"));
    }

    #[tokio::test]
    async fn test_read_file_preview_binary() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("test_preview.bin");
        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(&[0, 1, 2, 3, 4]).unwrap();

        let result = read_file_preview(file_path.to_string_lossy().to_string(), Some(1024)).await;
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), "Binary file detected");
    }

    #[tokio::test]
    async fn test_get_file_metadata() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("test_meta.txt");
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, "Meta test").unwrap();

        let metadata = get_file_metadata(file_path.to_string_lossy().to_string())
            .await
            .unwrap();
        assert!(metadata.size > 0);
        assert!(!metadata.is_dir);
    }
}
