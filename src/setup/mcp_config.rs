//! Write MCP server configuration to agent config files.

use serde_json::{Map, Value, json};
use std::io::{self, Write};
use std::path::Path;
use std::{fs, io::ErrorKind};

/// Write or update an MCP config file to include the `rust-doctor` server.
///
/// Reads the existing JSON (if any), merges a `rust-doctor` entry into
/// `mcpServers`, and writes atomically via temp file + rename to avoid
/// corrupting the user's config on crash.
pub fn write_mcp_config(path: &Path, command: &str, args: &[String]) -> io::Result<()> {
    // Read existing config or start with an empty object (race-free: no exists() check)
    let mut config = match fs::read_to_string(path) {
        Ok(content) => {
            serde_json::from_str::<Value>(&content).unwrap_or_else(|_| Value::Object(Map::new()))
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Value::Object(Map::new()),
        Err(e) => return Err(e),
    };

    // Ensure the top-level object and mcpServers key exist
    let obj = config
        .as_object_mut()
        .ok_or_else(|| io::Error::other("config file is not a JSON object"))?;
    if !obj.contains_key("mcpServers") {
        obj.insert("mcpServers".into(), json!({}));
    }

    // Insert or overwrite the rust-doctor server entry
    let servers = obj
        .get_mut("mcpServers")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| io::Error::other("mcpServers is not a JSON object"))?;

    servers.insert(
        "rust-doctor".into(),
        json!({
            "command": command,
            "args": args,
        }),
    );

    // Ensure parent directory exists (needed for both temp file and final path)
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    // Write atomically: temp file in same directory → rename (same-filesystem = atomic)
    let output = serde_json::to_string_pretty(&config).map_err(io::Error::other)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(output.as_bytes())?;
    tmp.write_all(b"\n")?;
    tmp.flush()?;
    tmp.persist(path).map_err(|e| e.error)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn writes_new_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");

        write_mcp_config(&path, "rust-doctor", &["--mcp".into()]).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let json: Value = serde_json::from_str(&content).unwrap();
        assert_eq!(json["mcpServers"]["rust-doctor"]["command"], "rust-doctor");
        assert_eq!(json["mcpServers"]["rust-doctor"]["args"][0], "--mcp");
    }

    #[test]
    fn preserves_existing_servers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(
            &path,
            r#"{"mcpServers":{"other-tool":{"command":"other"}}}"#,
        )
        .unwrap();

        write_mcp_config(
            &path,
            "npx",
            &["-y".into(), "rust-doctor@latest".into(), "--mcp".into()],
        )
        .unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let json: Value = serde_json::from_str(&content).unwrap();
        assert!(json["mcpServers"]["other-tool"].is_object());
        assert_eq!(json["mcpServers"]["rust-doctor"]["command"], "npx");
    }

    #[test]
    fn creates_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path: PathBuf = dir.path().join("nested").join("dir").join("mcp.json");

        write_mcp_config(&path, "rust-doctor", &["--mcp".into()]).unwrap();
        assert!(path.exists());
    }
}
