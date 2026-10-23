use std::fs;
use std::io::ErrorKind;
use std::path::Path;

/// Whether the `.jsonl` extension is required.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionCheck {
    Required,
    Skip,
}

/// Accept `path` when it points to an existing file.
///
/// The extension is checked only when `extension` is [`ExtensionCheck::Required`].
pub fn validate_jsonl_file(path: &str, extension: ExtensionCheck) -> Result<(), String> {
    let path = Path::new(path);

    if extension == ExtensionCheck::Required && !has_jsonl_extension(path) {
        return Err(format!("expected a .jsonl file, got '{}'", path.display()));
    }

    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            return Err(format!("file not found: '{}'", path.display()));
        }
        Err(err) => return Err(format!("cannot read '{}': {err}", path.display())),
    };

    if !metadata.is_file() {
        return Err(format!("path is not a file: '{}'", path.display()));
    }

    Ok(())
}

/// Return true when the final extension is `jsonl`, ignoring ASCII case.
fn has_jsonl_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("jsonl"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after the unix epoch")
            .as_nanos();
        env::temp_dir().join(format!("varynth-{nanos}-{name}"))
    }

    #[test]
    fn rejects_a_missing_jsonl_file() {
        let path = temp_path("missing.jsonl");
        let message =
            validate_jsonl_file(&path.to_string_lossy(), ExtensionCheck::Required).unwrap_err();
        assert!(message.contains("file not found"), "{message}");
    }

    #[test]
    fn rejects_a_non_jsonl_file() {
        let path = temp_path("notes.txt");
        fs::write(&path, "not jsonl\n").expect("temp file should be writable");
        let message =
            validate_jsonl_file(&path.to_string_lossy(), ExtensionCheck::Required).unwrap_err();
        let _ = fs::remove_file(&path);
        assert!(message.contains("expected a .jsonl file"), "{message}");
    }

    #[test]
    fn accepts_any_extension_when_the_check_is_skipped() {
        let path = temp_path("notes.txt");
        fs::write(&path, "{\"ok\":true}\n").expect("temp file should be writable");
        let result = validate_jsonl_file(&path.to_string_lossy(), ExtensionCheck::Skip);
        let _ = fs::remove_file(&path);
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn rejects_a_directory_with_a_jsonl_name() {
        let path = temp_path("dir.jsonl");
        fs::create_dir(&path).expect("temp directory should be creatable");
        let message =
            validate_jsonl_file(&path.to_string_lossy(), ExtensionCheck::Required).unwrap_err();
        let _ = fs::remove_dir(&path);
        assert!(message.contains("path is not a file"), "{message}");
    }

    #[test]
    fn accepts_an_existing_jsonl_file() {
        let path = temp_path("events.jsonl");
        fs::write(&path, "{\"ok\":true}\n").expect("temp file should be writable");
        let result = validate_jsonl_file(&path.to_string_lossy(), ExtensionCheck::Required);
        let _ = fs::remove_file(&path);
        assert!(result.is_ok(), "{result:?}");
    }
}
