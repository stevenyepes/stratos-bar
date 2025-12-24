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

fn calculate_file_score(name: &str, ext: Option<&str>, query: &str) -> i32 {
    let mut score = 0;
    let lower_name = name.to_lowercase();
    let lower_query = query.to_lowercase();

    // Name match bonuses
    if lower_name == lower_query {
        score += 100;
    } else if lower_name.starts_with(&lower_query) {
        score += 50;
    }

    // Extension prioritization
    if let Some(e) = ext {
        let e_lower = e.to_lowercase();
        match e_lower.as_str() {
            // Images (Highest priority)
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" | "bmp" | "tiff" => score += 40,

            // Videos (Second priority)
            "mp4" | "mkv" | "avi" | "mov" | "webm" | "flv" | "wmv" => score += 30,

            // Common documents/code (Third priority)
            "rs" | "js" | "ts" | "vue" | "py" | "html" | "css" | "json" | "md" | "txt" | "pdf"
            | "csv" | "docx" | "xlsx" => score += 20,

            _ => {}
        }
    }

    score
}

#[tauri::command]
pub async fn search_files(query: String, path: String) -> Result<Vec<String>, String> {
    let lower_query = query.to_lowercase();

    let walker = WalkDir::new(&path).max_depth(10).into_iter();

    // Collect matches
    let mut matches = Vec::new();

    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path();
        let path_str = path.to_string_lossy();

        if path_str.to_lowercase().contains(&lower_query) {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let ext = path.extension().and_then(|e| e.to_str());

            let score = calculate_file_score(name, ext, &query);

            matches.push((path_str.to_string(), score));
            if matches.len() > 500 {
                // Increased limit slightly to allow better sorting
                break;
            }
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

#[tauri::command]
pub async fn generate_video_thumbnail(
    app_handle: tauri::AppHandle,
    path: String,
) -> Result<String, String> {
    use std::process::Command;
    use tauri::Manager;

    let cache_dir = app_handle
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?;

    // Create cache dir if it doesn't exist
    if !cache_dir.exists() {
        std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
    }

    // Hash path to get unique filename
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    let filename = format!("{}.jpg", hasher.finish());
    let thumb_path = cache_dir.join(&filename);
    let thumb_path_str = thumb_path.to_string_lossy().to_string();

    if thumb_path.exists() {
        return Ok(thumb_path_str);
    }

    // Run ffmpeg
    // ffmpeg -y -i <input> -ss 00:00:01 -vframes 1 -vf scale=640:-1 <output>
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(&path)
        .arg("-ss")
        .arg("00:00:01")
        .arg("-vframes")
        .arg("1")
        // Scale to reasonable width, keep aspect ratio
        .arg("-vf")
        .arg("scale=640:-1")
        .arg(&thumb_path_str)
        .output()
        .map_err(|e| format!("Failed to execute ffmpeg: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg failed: {}", stderr));
    }

    Ok(thumb_path_str)
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
    #[test]
    fn test_calculate_file_score() {
        // Exact match
        assert_eq!(calculate_file_score("test", None, "test"), 100);

        // Starts with
        assert_eq!(calculate_file_score("test_file", None, "test"), 50);

        // Extensions
        // Image (40)
        assert_eq!(calculate_file_score("pic", Some("png"), "pic"), 140); // 100 + 40
        assert_eq!(calculate_file_score("foo", Some("jpg"), "bar"), 40);

        // Video (30)
        assert_eq!(calculate_file_score("vid", Some("mp4"), "bar"), 30);

        // Common (20)
        assert_eq!(calculate_file_score("doc", Some("txt"), "bar"), 20);

        // Prioritization check
        let img = calculate_file_score("a", Some("png"), "query");
        let vid = calculate_file_score("a", Some("mp4"), "query");
        let doc = calculate_file_score("a", Some("txt"), "query");
        let other = calculate_file_score("a", Some("bin"), "query");

        assert!(img > vid);
        assert!(vid > doc);
        assert!(doc > other);
    }
}
