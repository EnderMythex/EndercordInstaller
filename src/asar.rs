// SPDX-License-Identifier: GPL-3.0-or-later
// Writes the tiny loader app.asar. Ported from
// https://github.com/GeopJr/asar-cr/blob/cd7695b7c913bf921d9fb6600eaeb1400e3ba225/src/asar-cr/pack.cr#L61

use std::fs;
use std::io;
use std::path::Path;

use crate::constants::PACKAGE_JSON;

pub fn build_loader(target: &Path) -> Vec<u8> {
    let index_js = format!("require({})", serde_json::to_string(&target.to_string_lossy()).unwrap());

    let mut header = format!(
        r#"{{"files":{{"index.js":{{"size":{},"offset":"0"}},"package.json":{{"size":{},"offset":"{}"}}}}}}"#,
        index_js.len(),
        PACKAGE_JSON.len(),
        index_js.len()
    );

    let header_len = header.len() as u32;
    let aligned = (header_len + 3) & !3;
    header.push_str(&"0".repeat((aligned - header_len) as usize));

    let mut out = Vec::with_capacity(16 + header.len() + index_js.len() + PACKAGE_JSON.len());
    for n in [4, aligned + 8, aligned + 4, header_len] {
        out.extend_from_slice(&n.to_le_bytes());
    }
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(index_js.as_bytes());
    out.extend_from_slice(PACKAGE_JSON.as_bytes());
    out
}

pub fn write_loader(out_file: &Path, target: &Path) -> io::Result<()> {
    fs::write(out_file, build_loader(target))
}

/// True when the asar at `path` is a loader written by us (or the Vencord/Equicord installers).
/// EnderCord itself re-patches Discord updates with an `app.asar` folder holding an index.js instead of a file.
pub fn is_loader(path: &Path) -> io::Result<bool> {
    if path.is_dir() {
        return Ok(fs::read_to_string(path.join("index.js"))?.contains("require("));
    }
    if fs::metadata(path)?.len() > 128 * 1024 {
        return Ok(false);
    }
    let bytes = fs::read(path)?;
    let contains = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
    Ok(contains(PACKAGE_JSON.as_bytes()) && contains(b"require("))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_matches_go_installer_layout() {
        let bytes = build_loader(Path::new("/tmp/endercord.asar"));
        let u32_at = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());

        assert_eq!(u32_at(0), 4);
        let header_len = u32_at(12) as usize;
        let header = std::str::from_utf8(&bytes[16..16 + header_len]).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(header).unwrap();
        assert_eq!(parsed["files"]["index.js"]["offset"], "0");

        let data_start = 8 + u32_at(4) as usize;
        let index_len = parsed["files"]["index.js"]["size"].as_u64().unwrap() as usize;
        assert_eq!(
            &bytes[data_start..data_start + index_len],
            br#"require("/tmp/endercord.asar")"#
        );
        assert!(bytes.ends_with(PACKAGE_JSON.as_bytes()));
    }

    #[test]
    fn detects_folder_loader() {
        let dir = std::env::temp_dir().join(format!("endercord-shim-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("index.js"), r#"require("/x/endercord.asar/patcher.js");"#).unwrap();
        assert!(is_loader(&dir).unwrap());
        fs::remove_dir_all(dir).unwrap();
    }
}
