use chardetng::{EncodingDetector, Iso2022JpDetection, Utf8Detection};
use encoding_rs::{Encoding, UTF_8};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileReadResult {
    pub content: String,
    pub encoding: String,
    pub language: String,
    pub had_bom: bool,
}

/// Read a file from disk, auto-detecting its text encoding (UTF-8/16, GBK, etc.)
/// and returning UTF-8 content along with the original encoding label so we can
/// re-save in the same encoding later.
#[tauri::command]
pub async fn read_file(path: String) -> Result<FileReadResult, String> {
    tauri::async_runtime::spawn_blocking(move || read_file_inner(path))
        .await
        .map_err(|e| format!("join: {e}"))?
}

pub fn read_file_inner(path: String) -> Result<FileReadResult, String> {
    let bytes = fs::read(&path).map_err(|e| format!("read failed: {e}"))?;

    // Try BOM first.
    let (encoding, had_bom, body) = if let Some((enc, bom_len)) = sniff_bom(&bytes) {
        (enc, true, &bytes[bom_len..])
    } else {
        // chardetng for everything else.
        let mut detector = EncodingDetector::new(Iso2022JpDetection::Allow);
        detector.feed(&bytes, true);
        let enc = detector.guess(None, Utf8Detection::Allow);
        (enc, false, bytes.as_slice())
    };

    let (cow, _used_enc, had_errors) = encoding.decode_without_bom_handling_and_without_replacement(body)
        .map(|c| (c, encoding, false))
        .unwrap_or_else(|| {
            let (c, used, errs) = encoding.decode(body);
            (c, used, errs)
        });

    if had_errors {
        let lossy = String::from_utf8_lossy(body).into_owned();
        return Ok(FileReadResult {
            content: lossy,
            encoding: encoding.name().to_string(),
            language: detect_language(&path),
            had_bom,
        });
    }

    Ok(FileReadResult {
        content: cow.into_owned(),
        encoding: encoding.name().to_string(),
        language: detect_language(&path),
        had_bom,
    })
}

/// Write a UTF-8 string back to disk in the requested encoding.
#[tauri::command]
pub async fn write_file(
    app: tauri::AppHandle,
    path: String,
    content: String,
    encoding: String,
    _workspace: Option<String>,
) -> Result<(), String> {
    let app_for_write = app.clone();
    tauri::async_runtime::spawn_blocking({
        let path = path.clone();
        let content = content.clone();
        move || {
            #[cfg(target_os = "android")]
            if path.starts_with("content://") {
                let bytes = encode_for_disk(&content, &encoding)?;
                return write_android_content_uri(&app_for_write, &path, &bytes);
            }
            #[cfg(not(target_os = "android"))]
            let _ = &app_for_write;
            write_file_inner(path, content, encoding)
        }
    })
    .await
    .map_err(|e| format!("join: {e}"))??;

    Ok(())
}

pub fn encode_for_disk(content: &str, encoding: &str) -> Result<Vec<u8>, String> {
    let enc = Encoding::for_label(encoding.as_bytes()).unwrap_or(UTF_8);
    if enc == encoding_rs::UTF_16LE || enc == encoding_rs::UTF_16BE {
        let le = enc == encoding_rs::UTF_16LE;
        let mut out = Vec::with_capacity(2 + content.len() * 2);
        out.extend_from_slice(if le { &[0xFF, 0xFE] } else { &[0xFE, 0xFF] });
        for unit in content.encode_utf16() {
            out.extend_from_slice(&if le {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            });
        }
        return Ok(out);
    }
    let (cow, _, had_errors) = enc.encode(content);
    if had_errors {
        return Err(format!(
            "Some characters cannot be represented in {}",
            enc.name()
        ));
    }
    Ok(cow.into_owned())
}

pub fn write_file_inner(path: String, content: String, encoding: String) -> Result<(), String> {
    let bytes = encode_for_disk(&content, &encoding)?;
    fs::write(&path, &bytes).map_err(|e| format!("write failed: {e}"))?;
    super::watcher::mark_self_write(&path);
    Ok(())
}

#[tauri::command]
pub async fn read_binary_file(path: String) -> Result<Vec<u8>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        fs::read(&path).map_err(|e| format!("read failed: {e}"))
    })
    .await
    .map_err(|e| format!("join: {e}"))?
}

#[tauri::command]
pub async fn write_binary_file(
    app: tauri::AppHandle,
    path: String,
    data: Vec<u8>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "android")]
        if path.starts_with("content://") {
            return write_android_content_uri(&app, &path, &data);
        }
        #[cfg(not(target_os = "android"))]
        let _ = &app;
        write_binary_file_inner(path, data)
    })
    .await
    .map_err(|e| format!("join: {e}"))?
}

#[cfg(target_os = "android")]
fn write_android_content_uri(
    app: &tauri::AppHandle,
    uri: &str,
    data: &[u8],
) -> Result<(), String> {
    use std::io::Write;
    use std::str::FromStr;
    use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};

    let Ok(fp) = FilePath::from_str(uri);
    let mut opts = OpenOptions::new();
    opts.read(false).write(true).create(true).truncate(true);
    if let Ok(mut file) = app.fs().open(fp, opts) {
        if file.write_all(data).and_then(|()| file.flush()).is_ok() {
            return Ok(());
        }
    }
    super::saf_android::write_content_uri_bytes(uri, data)
}

pub fn write_binary_file_inner(path: String, data: Vec<u8>) -> Result<(), String> {
    if let Some(parent) = Path::new(&path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir failed: {e}"))?;
        }
    }
    fs::write(&path, &data).map_err(|e| format!("write failed: {e}"))?;
    super::watcher::mark_self_write(&path);
    Ok(())
}

#[tauri::command]
pub async fn copy_file(src: String, dst: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || copy_file_inner(src, dst))
        .await
        .map_err(|e| format!("join: {e}"))?
}

pub fn copy_file_inner(src: String, dst: String) -> Result<(), String> {
    if let Some(parent) = Path::new(&dst).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir failed: {e}"))?;
        }
    }
    fs::copy(&src, &dst).map_err(|e| format!("copy failed: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn fs_create_file(path: String, content: Option<String>) -> Result<(), String> {
    let p = Path::new(&path);
    if p.exists() {
        return Err(format!("already exists: {path}"));
    }
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir failed: {e}"))?;
        }
    }
    fs::write(p, content.unwrap_or_default().as_bytes())
        .map_err(|e| format!("create failed: {e}"))
}

#[tauri::command]
pub fn fs_create_dir(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if p.exists() {
        return Err(format!("already exists: {path}"));
    }
    fs::create_dir_all(p).map_err(|e| format!("mkdir failed: {e}"))
}

#[tauri::command]
pub fn fs_delete(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Ok(());
    }
    if p.is_dir() {
        fs::remove_dir_all(p).map_err(|e| format!("delete dir failed: {e}"))
    } else {
        fs::remove_file(p).map_err(|e| format!("delete file failed: {e}"))
    }
}

#[tauri::command]
pub fn webview_runtime_version() -> Option<String> {
    tauri::webview_version().ok()
}

#[tauri::command]
pub fn fs_dir_exists(path: String) -> bool {
    Path::new(&path).is_dir()
}

#[tauri::command]
pub fn fs_rename(from: String, to: String) -> Result<(), String> {
    let from_p = Path::new(&from);
    let to_p = Path::new(&to);
    if !from_p.exists() {
        return Err(format!("source missing: {from}"));
    }
    if to_p.exists() {
        return Err(format!("target already exists: {to}"));
    }

    let from_assets = sibling_assets_dir(from_p);
    let to_assets = sibling_assets_dir(to_p);
    let stems_differ = from_assets
        .as_ref()
        .zip(to_assets.as_ref())
        .map(|(a, b)| a.file_name() != b.file_name())
        .unwrap_or(false);

    fs::rename(from_p, to_p).map_err(|e| format!("rename failed: {e}"))?;

    if let (Some(fa), Some(ta)) = (from_assets, to_assets) {
        if fa.is_dir() && !ta.exists() {
            if let Err(e) = fs::rename(&fa, &ta) {
                eprintln!("[fs_rename] assets folder rename failed: {e}");
            } else if stems_differ && is_markdown_path(to_p) {
                if let Err(e) = rewrite_assets_refs(to_p, &fa, &ta) {
                    eprintln!("[fs_rename] body rewrite failed: {e}");
                }
            }
        }
    }
    Ok(())
}

fn sibling_assets_dir(p: &Path) -> Option<std::path::PathBuf> {
    let stem = p.file_stem()?.to_string_lossy().to_string();
    if stem.is_empty() {
        return None;
    }
    let parent = p.parent()?;
    Some(parent.join(format!("{stem}.assets")))
}

fn is_markdown_path(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()).map(|s| s.to_ascii_lowercase()).as_deref(),
        Some("md") | Some("markdown") | Some("mdown") | Some("mkd")
    )
}

fn rewrite_assets_refs(file: &Path, old_assets: &Path, new_assets: &Path) -> Result<(), String> {
    let old_name = old_assets
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "old assets name missing".to_string())?;
    let new_name = new_assets
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "new assets name missing".to_string())?;
    let bytes = fs::read(file).map_err(|e| format!("read for rewrite: {e}"))?;
    let body = match std::str::from_utf8(&bytes) {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };
    let old_pat = format!("{old_name}/");
    let new_pat = format!("{new_name}/");
    if !body.contains(&old_pat) {
        return Ok(());
    }
    let rewritten = body.replace(&old_pat, &new_pat);
    fs::write(file, rewritten).map_err(|e| format!("write back: {e}"))
}

#[tauri::command]
pub fn fs_move(from: String, to: String) -> Result<(), String> {
    fs_move_inner(from, to)
}

pub fn fs_move_inner(from: String, to: String) -> Result<(), String> {
    let from_p = Path::new(&from);
    let to_p = Path::new(&to);
    if !from_p.exists() {
        return Err(format!("source missing: {from}"));
    }
    let to_parent = to_p
        .parent()
        .ok_or_else(|| "destination has no parent folder".to_string())?;
    if !to_parent.is_dir() {
        return Err(format!("destination folder missing: {}", to_parent.display()));
    }
    if to_p.exists() {
        return Err(format!("target already exists: {to}"));
    }
    if from_p.is_dir() && is_self_or_descendant(from_p, to_parent) {
        return Err("a folder cannot be moved inside itself".to_string());
    }

    let from_assets = sibling_assets_dir(from_p);
    let to_assets = sibling_assets_dir(to_p);

    move_tree(from_p, to_p)?;

    let mut moved_roots: Vec<Vec<String>> = vec![abs_segments(from_p)];
    if let (Some(fa), Some(ta)) = (from_assets, to_assets) {
        if fa.is_dir() && !ta.exists() {
            match move_tree(&fa, &ta) {
                Ok(()) => moved_roots.push(abs_segments(&fa)),
                Err(e) => eprintln!("[fs_move] assets folder move failed: {e}"),
            }
        }
    }

    relink_moved_tree(to_p, from_p, &moved_roots);
    Ok(())
}

fn is_self_or_descendant(outer: &Path, inner: &Path) -> bool {
    let c_outer = fs::canonicalize(outer).unwrap_or_else(|_| outer.to_path_buf());
    let c_inner = fs::canonicalize(inner).unwrap_or_else(|_| inner.to_path_buf());
    c_inner.starts_with(&c_outer)
}

fn move_tree(from: &Path, to: &Path) -> Result<(), String> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(e) => {
            if !matches!(e.raw_os_error(), Some(18) | Some(17)) {
                return Err(format!("move failed: {e}"));
            }
            copy_tree(from, to).map_err(|e| format!("cross-device copy failed: {e}"))?;
            let removed = if from.is_dir() {
                fs::remove_dir_all(from)
            } else {
                fs::remove_file(from)
            };
            removed.map_err(|e| {
                format!("copied to the new location, but the original could not be removed: {e}")
            })
        }
    }
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copy_tree(&entry.path(), &to.join(entry.file_name()))?;
        }
    } else {
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(from, to)?;
    }
    Ok(())
}

pub fn abs_segments(p: &Path) -> Vec<String> {
    p.components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
        .collect()
}

pub fn lexical_resolve(base: &[String], target: &str) -> Option<Vec<String>> {
    let mut out = base.to_vec();
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop()?;
            }
            s => out.push(s.to_string()),
        }
    }
    Some(out)
}

pub fn lexical_relative(base: &[String], target: &[String]) -> String {
    let common = base
        .iter()
        .zip(target.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let mut segs: Vec<String> = vec!["..".to_string(); base.len() - common];
    segs.extend(target[common..].iter().cloned());
    if segs.is_empty() {
        ".".to_string()
    } else {
        segs.join("/")
    }
}

fn is_external_target(t: &str) -> bool {
    if t.is_empty() || t.starts_with('#') || t.starts_with('/') || t.starts_with('\\') {
        return true;
    }
    match t.find(':') {
        None => false,
        Some(i) => {
            let scheme = &t[..i];
            !scheme.is_empty()
                && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
        }
    }
}

fn shift_target(
    t: &str,
    old_dir: &[String],
    new_dir: &[String],
    moved_roots: &[Vec<String>],
) -> Option<String> {
    if is_external_target(t) {
        return None;
    }
    let cut = t.find(['#', '?']).unwrap_or(t.len());
    let (path, suffix) = t.split_at(cut);
    if path.is_empty() {
        return None;
    }
    let resolved = lexical_resolve(old_dir, path)?;
    if moved_roots.iter().any(|r| resolved.starts_with(r)) {
        return None;
    }
    let shifted = format!("{}{}", lexical_relative(new_dir, &resolved), suffix);
    if shifted == t {
        None
    } else {
        Some(shifted)
    }
}

fn destination_range(payload: &str) -> Option<(usize, usize)> {
    let lead = payload.len() - payload.trim_start().len();
    let rest = &payload[lead..];
    if let Some(stripped) = rest.strip_prefix('<') {
        let close = stripped.find('>')?;
        return Some((lead + 1, lead + 1 + close));
    }
    let end = rest
        .find(|c: char| c.is_whitespace())
        .unwrap_or(rest.len());
    if end == 0 {
        None
    } else {
        Some((lead, lead + end))
    }
}

fn rewrite_line(
    line: &str,
    old_dir: &[String],
    new_dir: &[String],
    moved_roots: &[Vec<String>],
) -> Option<String> {
    let mut edits: Vec<(usize, usize, String)> = Vec::new();

    let consider = |start: usize, end: usize, edits: &mut Vec<(usize, usize, String)>| {
        if line[..start].matches('`').count() % 2 == 1 {
            return;
        }
        if let Some(next) = shift_target(&line[start..end], old_dir, new_dir, moved_roots) {
            edits.push((start, end, next));
        }
    };

    let mut cursor = 0;
    while let Some(open) = line[cursor..].find("](") {
        let abs_open = cursor + open;
        let payload_start = abs_open + 2;
        if let Some(close) = line[payload_start..].find(')') {
            let payload_end = payload_start + close;
            let payload = &line[payload_start..payload_end];
            if let Some((s, e)) = destination_range(payload) {
                consider(payload_start + s, payload_start + e, &mut edits);
            }
            cursor = payload_end + 1;
        } else {
            break;
        }
    }

    for attr in ["src=\"", "href=\""] {
        let mut cur = 0;
        while let Some(pos) = line[cur..].find(attr) {
            let val_start = cur + pos + attr.len();
            if let Some(val_end_rel) = line[val_start..].find('"') {
                let val_end = val_start + val_end_rel;
                consider(val_start, val_end, &mut edits);
                cur = val_end + 1;
            } else {
                break;
            }
        }
    }

    let trimmed = line.trim_start();
    if trimmed.starts_with('[') {
        if let Some(bracket_end) = trimmed.find("]:") {
            let after_colon = &trimmed[bracket_end + 2..];
            let target_lead = after_colon.len() - after_colon.trim_start().len();
            let target_part = after_colon.trim_start();
            let target_len = target_part.find(|c: char| c.is_whitespace()).unwrap_or(target_part.len());
            if target_len > 0 {
                let start_in_line = (line.len() - trimmed.len()) + bracket_end + 2 + target_lead;
                let end_in_line = start_in_line + target_len;
                consider(start_in_line, end_in_line, &mut edits);
            }
        }
    }

    if edits.is_empty() {
        return None;
    }

    edits.sort_by_key(|(s, _, _)| *s);
    let mut out = String::with_capacity(line.len());
    let mut cur = 0;
    for (s, e, replacement) in edits {
        if s < cur {
            continue;
        }
        out.push_str(&line[cur..s]);
        out.push_str(&replacement);
        cur = e;
    }
    out.push_str(&line[cur..]);
    Some(out)
}

pub fn rewrite_links_after_move(
    body: &str,
    old_dir: &[String],
    new_dir: &[String],
    moved_roots: &[Vec<String>],
) -> Option<String> {
    let mut out = String::with_capacity(body.len());
    let mut in_fence = false;
    let mut changed = false;
    for line in body.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            out.push_str(line);
            continue;
        }
        if in_fence {
            out.push_str(line);
            continue;
        }
        match rewrite_line(line, old_dir, new_dir, moved_roots) {
            Some(next) => {
                changed = true;
                out.push_str(&next);
            }
            None => out.push_str(line),
        }
    }
    if changed {
        Some(out)
    } else {
        None
    }
}

fn relink_moved_tree(new_root: &Path, old_root: &Path, moved_roots: &[Vec<String>]) {
    let files: Vec<std::path::PathBuf> = if new_root.is_dir() {
        walkdir::WalkDir::new(new_root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file() && is_markdown_path(e.path()))
            .map(|e| e.path().to_path_buf())
            .collect()
    } else if is_markdown_path(new_root) {
        vec![new_root.to_path_buf()]
    } else {
        Vec::new()
    };

    for file in files {
        let old_file = match file.strip_prefix(new_root) {
            Ok(rel) if !rel.as_os_str().is_empty() => old_root.join(rel),
            _ => old_root.to_path_buf(),
        };
        let (Some(od), Some(nd)) = (old_file.parent(), file.parent()) else {
            continue;
        };
        let old_dir = abs_segments(od);
        let new_dir = abs_segments(nd);
        if old_dir == new_dir {
            continue;
        }
        let Ok(bytes) = fs::read(&file) else { continue };
        let Ok(body) = std::str::from_utf8(&bytes) else {
            continue;
        };
        if let Some(next) = rewrite_links_after_move(body, &old_dir, &new_dir, moved_roots) {
            if let Err(e) = fs::write(&file, next) {
                eprintln!("[fs_move] link rewrite write-back failed for {}: {e}", file.display());
            }
        }
    }
}

#[tauri::command]
pub async fn fs_list_dirs(root: String, show_hidden: Option<bool>) -> Result<Vec<String>, String> {
    let show_hidden = show_hidden.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || fs_list_dirs_inner(root, show_hidden))
        .await
        .map_err(|e| format!("join: {e}"))?
}

pub fn fs_list_dirs_inner(root: String, show_hidden: bool) -> Result<Vec<String>, String> {
    const CAP: usize = 20_000;
    const SKIP: &[&str] = &["node_modules", "target", "dist", "_assets", ".git"];
    let root_p = Path::new(&root);
    if !root_p.is_dir() {
        return Err(format!("not a folder: {root}"));
    }
    let mut out: Vec<String> = Vec::new();
    for entry in walkdir::WalkDir::new(root_p)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            let name = e.file_name().to_string_lossy().to_string();
            (show_hidden || !name.starts_with('.'))
                && !SKIP.contains(&name.as_str())
                && !name.ends_with(".assets")
        })
        .filter_map(|e| e.ok())
    {
        if entry.depth() == 0 || !entry.file_type().is_dir() {
            continue;
        }
        if let Ok(rel) = entry.path().strip_prefix(root_p) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
        if out.len() >= CAP {
            break;
        }
    }
    out.sort();
    Ok(out)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExtensionCount {
    pub ext: String,
    pub count: usize,
}

#[tauri::command]
pub async fn fs_list_extensions(
    root: String,
    show_hidden: Option<bool>,
) -> Result<Vec<ExtensionCount>, String> {
    let show_hidden = show_hidden.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || fs_list_extensions_inner(root, show_hidden))
        .await
        .map_err(|e| format!("join: {e}"))?
}

pub fn fs_list_extensions_inner(
    root: String,
    show_hidden: bool,
) -> Result<Vec<ExtensionCount>, String> {
    let root_p = Path::new(&root);
    if !root_p.is_dir() {
        return Err(format!("not a folder: {root}"));
    }
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for entry in filtered_walk(root_p, show_hidden) {
        if !entry.file_type().is_file() {
            continue;
        }
        *counts.entry(entry_extension(entry.path())).or_insert(0) += 1;
    }
    let mut out: Vec<ExtensionCount> = counts
        .into_iter()
        .map(|(ext, count)| ExtensionCount { ext, count })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.ext.cmp(&b.ext)));
    Ok(out)
}

#[tauri::command]
pub async fn fs_dirs_with_extensions(
    root: String,
    exts: Vec<String>,
    show_hidden: Option<bool>,
) -> Result<Vec<String>, String> {
    let show_hidden = show_hidden.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || {
        fs_dirs_with_extensions_inner(root, exts, show_hidden)
    })
    .await
    .map_err(|e| format!("join: {e}"))?
}

pub fn fs_dirs_with_extensions_inner(
    root: String,
    exts: Vec<String>,
    show_hidden: bool,
) -> Result<Vec<String>, String> {
    let root_p = Path::new(&root);
    if !root_p.is_dir() {
        return Err(format!("not a folder: {root}"));
    }
    let wanted: std::collections::HashSet<String> =
        exts.into_iter().map(|e| e.trim_start_matches('.').to_ascii_lowercase()).collect();
    let mut hits: std::collections::HashSet<String> = std::collections::HashSet::new();
    for entry in filtered_walk(root_p, show_hidden) {
        if !entry.file_type().is_file() || !wanted.contains(&entry_extension(entry.path())) {
            continue;
        }
        let mut dir = entry.path().parent();
        while let Some(d) = dir {
            if d == root_p {
                break;
            }
            match d.strip_prefix(root_p) {
                Ok(rel) => {
                    let key = rel.to_string_lossy().replace('\\', "/");
                    if !hits.insert(key) {
                        break;
                    }
                }
                Err(_) => break,
            }
            dir = d.parent();
        }
    }
    let mut out: Vec<String> = hits.into_iter().collect();
    out.sort();
    Ok(out)
}

fn filtered_walk(root: &Path, show_hidden: bool) -> impl Iterator<Item = walkdir::DirEntry> {
    const SKIP: &[&str] = &["node_modules", "target", "dist", ".git"];
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(move |e| {
            if e.depth() == 0 {
                return true;
            }
            let name = e.file_name().to_string_lossy().to_string();
            (show_hidden || !name.starts_with('.')) && !SKIP.contains(&name.as_str())
        })
        .filter_map(|e| e.ok())
        .take(200_000)
}

fn entry_extension(p: &Path) -> String {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default()
}

fn sniff_bom(bytes: &[u8]) -> Option<(&'static Encoding, usize)> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        Some((encoding_rs::UTF_8, 3))
    } else if bytes.starts_with(&[0xFF, 0xFE]) {
        Some((encoding_rs::UTF_16LE, 2))
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        Some((encoding_rs::UTF_16BE, 2))
    } else {
        None
    }
}

fn detect_language(path: &str) -> String {
    let ext = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "md" | "markdown" | "mdown" | "mkd" => "markdown".to_string(),
        _ => "plaintext".to_string(),
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub modified: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub created: Option<u64>,
}

fn epoch_ms(t: std::io::Result<std::time::SystemTime>) -> Option<u64> {
    t.ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
}

pub fn entry_times(meta: &fs::Metadata) -> (Option<u64>, Option<u64>) {
    let modified = epoch_ms(meta.modified());
    let created = epoch_ms(meta.created()).or(modified);
    (modified, created)
}

pub fn list_dir_inner(path: String, show_hidden: bool) -> Result<Vec<DirEntry>, String> {
    list_dir_with(path, show_hidden, false)
}

pub fn list_dir_with(path: String, show_hidden: bool, with_times: bool) -> Result<Vec<DirEntry>, String> {
    let read = fs::read_dir(&path).map_err(|e| format!("read_dir failed: {e}"))?;
    const HARD_CAP: usize = 10_000;
    let mut entries: Vec<DirEntry> = Vec::new();
    let mut truncated = false;
    for e in read.flatten() {
        if entries.len() >= HARD_CAP {
            truncated = true;
            break;
        }
        let name = e.file_name().to_string_lossy().to_string();
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        let is_dir = match e.file_type() {
            Ok(t) => t.is_dir(),
            Err(_) => continue,
        };
        let (modified, created) = if with_times {
            match e.metadata() {
                Ok(m) => entry_times(&m),
                Err(_) => (None, None),
            }
        } else {
            (None, None)
        };
        entries.push(DirEntry {
            name,
            path: e.path().to_string_lossy().to_string(),
            is_dir,
            modified,
            created,
        });
    }
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    if truncated {
        entries.push(DirEntry {
            name: "__solomd_truncated__".into(),
            path: String::new(),
            is_dir: false,
            modified: None,
            created: None,
        });
    }
    Ok(entries)
}

#[tauri::command]
pub async fn list_dir(
    path: String,
    show_hidden: Option<bool>,
    with_times: Option<bool>,
) -> Result<Vec<DirEntry>, String> {
    let show_hidden = show_hidden.unwrap_or(false);
    let with_times = with_times.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || list_dir_with(path, show_hidden, with_times))
        .await
        .map_err(|e| format!("join: {e}"))?
}

// ---------------------------------------------------------------------------
// Frontmatter property editing
// ---------------------------------------------------------------------------

struct BlockLayout {
    nl: &'static str,
    had_block: bool,
    open: String,
    close: String,
    inner_lines: Vec<String>,
    body: String,
}

struct KeySpan {
    key: String,
    start: usize,
    end: usize,
}

fn top_level_key_of(line: &str) -> Option<String> {
    if line.is_empty() || line.starts_with([' ', '\t', '#', '-']) {
        return None;
    }
    let bytes = line.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b':' if !in_single && !in_double => {
                let next = bytes.get(i + 1);
                if next.is_none() || next == Some(&b' ') || next == Some(&b'\t') {
                    return Some(line[..i].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn unquote_key(raw: &str) -> String {
    let t = raw.trim();
    if t.len() >= 2
        && ((t.starts_with('"') && t.ends_with('"'))
            || (t.starts_with('\'') && t.ends_with('\'')))
    {
        return t[1..t.len() - 1].to_string();
    }
    t.to_string()
}

fn split_block(raw: &str) -> Result<BlockLayout, String> {
    let starts_lf = raw.starts_with("---\n");
    let starts_crlf = raw.starts_with("---\r\n");
    if !starts_lf && !starts_crlf {
        return Ok(BlockLayout {
            nl: if raw.contains("\r\n") { "\r\n" } else { "\n" },
            had_block: false,
            open: String::new(),
            close: String::new(),
            inner_lines: Vec::new(),
            body: raw.to_string(),
        });
    }
    let nl: &'static str = if starts_crlf { "\r\n" } else { "\n" };
    let open_len = if starts_crlf { 5 } else { 4 };
    let rest = &raw[open_len..];

    let mut offset = 0usize;
    let mut close_start: Option<usize> = None;
    let mut close_len = 0usize;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed == "---" || trimmed == "..." {
            close_start = Some(offset);
            close_len = line.len();
            break;
        }
        offset += line.len();
    }
    let Some(close_start) = close_start else {
        return Ok(BlockLayout {
            nl,
            had_block: false,
            open: String::new(),
            close: String::new(),
            inner_lines: Vec::new(),
            body: raw.to_string(),
        });
    };
    let yaml_src = &rest[..close_start];
    let close = rest[close_start..close_start + close_len].to_string();
    let body = rest[close_start + close_len..].to_string();

    let inner_lines: Vec<String> = if yaml_src.is_empty() {
        Vec::new()
    } else {
        yaml_src
            .strip_suffix('\n')
            .unwrap_or(yaml_src)
            .split('\n')
            .map(|l| l.trim_end_matches('\r').to_string())
            .collect()
    };

    Ok(BlockLayout {
        nl,
        had_block: true,
        open: raw[..open_len].to_string(),
        close,
        inner_lines,
        body,
    })
}

fn key_spans(inner: &[String]) -> Vec<KeySpan> {
    let mut spans: Vec<KeySpan> = Vec::new();
    let mut i = 0;
    while i < inner.len() {
        if let Some(raw_key) = top_level_key_of(&inner[i]) {
            let start = i;
            let mut end = i + 1;
            while end < inner.len() {
                let l = &inner[end];
                let is_continuation = l.is_empty()
                    || l.starts_with([' ', '\t'])
                    || l.starts_with('-');
                if l.is_empty() {
                    let next_real = inner[end + 1..]
                        .iter()
                        .find(|x| !x.is_empty());
                    let follows = next_real
                        .map(|x| x.starts_with([' ', '\t', '-']))
                        .unwrap_or(false);
                    if !follows {
                        break;
                    }
                    end += 1;
                } else if is_continuation {
                    end += 1;
                } else {
                    break;
                }
            }
            spans.push(KeySpan {
                key: unquote_key(&raw_key),
                start,
                end,
            });
            i = end;
        } else {
            i += 1;
        }
    }
    spans
}

fn render_entry(key: &str, value: &serde_json::Value) -> Result<Vec<String>, String> {
    match value {
        serde_json::Value::Null => Ok(vec![format!("{key}: null")]),
        serde_json::Value::Bool(b) => Ok(vec![format!("{key}: {b}")]),
        serde_json::Value::Number(n) => Ok(vec![format!("{key}: {n}")]),
        serde_json::Value::String(s) => {
            if s.contains('\n') {
                let mut lines = vec![format!("{key}: |")];
                for l in s.lines() {
                    lines.push(format!("  {l}"));
                }
                Ok(lines)
            } else if s.contains(':') || s.contains('#') || s.starts_with('@') || s.starts_with('[') {
                let escaped = s.replace('"', "\\\"");
                Ok(vec![format!("{key}: \"{escaped}\"")])
            } else {
                Ok(vec![format!("{key}: {s}")])
            }
        }
        serde_json::Value::Array(arr) => {
            let mut lines = vec![format!("{key}:")];
            for item in arr {
                match item {
                    serde_json::Value::String(s) => lines.push(format!("  - {s}")),
                    _ => lines.push(format!("  - {item}")),
                }
            }
            Ok(lines)
        }
        serde_json::Value::Object(obj) => {
            let mut lines = vec![format!("{key}:")];
            for (k, v) in obj {
                lines.push(format!("  {k}: {v}"));
            }
            Ok(lines)
        }
    }
}

fn rebuild_block(layout: &BlockLayout) -> String {
    if layout.had_block && layout.inner_lines.is_empty() {
        return layout.body.clone();
    }
    let mut out = String::new();
    if layout.had_block {
        out.push_str(&layout.open);
        out.push_str(&layout.inner_lines.join(layout.nl));
        out.push_str(layout.nl);
        out.push_str(&layout.close);
        out.push_str(&layout.body);
        return out;
    }
    out.push_str("---");
    out.push_str(layout.nl);
    out.push_str(&layout.inner_lines.join(layout.nl));
    out.push_str(layout.nl);
    out.push_str("---");
    out.push_str(layout.nl);
    if !layout.body.is_empty() {
        out.push_str(layout.body.trim_start_matches(['\n', '\r']));
    }
    out
}

pub fn set_frontmatter_property_str(
    raw: &str,
    key: &str,
    value: &serde_json::Value,
) -> Result<String, String> {
    if key.trim().is_empty() {
        return Err("frontmatter key must not be empty".into());
    }
    let mut layout = split_block(raw)?;
    let entry = render_entry(key, value)?;
    let spans = key_spans(&layout.inner_lines);

    if let Some(span) = spans.iter().find(|s| s.key == key) {
        layout
            .inner_lines
            .splice(span.start..span.end, entry.into_iter());
    } else {
        layout.inner_lines.extend(entry);
    }
    Ok(rebuild_block(&layout))
}

pub fn delete_frontmatter_property_str(raw: &str, key: &str) -> Result<String, String> {
    let mut layout = split_block(raw)?;
    let spans = key_spans(&layout.inner_lines);
    if let Some(span) = spans.iter().find(|s| s.key == key) {
        layout.inner_lines.drain(span.start..span.end);
        if key_spans(&layout.inner_lines).is_empty() {
            layout.inner_lines.clear();
        }
    }
    Ok(rebuild_block(&layout))
}

#[tauri::command]
pub async fn update_frontmatter_property(
    path: String,
    key: String,
    value: serde_json::Value,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let next = set_frontmatter_property_str(&raw, &key, &value)?;
        fs::write(&path, &next).map_err(|e| e.to_string())?;
        Ok(next)
    })
    .await
    .map_err(|e| format!("join: {e}"))?
}

#[tauri::command]
pub async fn delete_frontmatter_property(
    path: String,
    key: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let next = delete_frontmatter_property_str(&raw, &key)?;
        fs::write(&path, &next).map_err(|e| e.to_string())?;
        Ok(next)
    })
    .await
    .map_err(|e| format!("join: {e}"))?
}

#[cfg(test)]
mod list_dir_times_tests {
    use super::*;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn now_ms() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
    }

    #[test]
    fn times_only_when_asked() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.md"), "a").unwrap();
        let p = dir.path().to_string_lossy().to_string();

        let plain = list_dir_with(p.clone(), false, false).unwrap();
        assert_eq!(plain.len(), 1);
        assert!(plain[0].modified.is_none() && plain[0].created.is_none());

        let timed = list_dir_with(p, false, true).unwrap();
        let m = timed[0].modified.expect("modified");
        let c = timed[0].created.expect("created (or modified fallback)");
        let now = now_ms();
        assert!(m <= now + 1_000 && now - m < 60_000, "modified {m} vs now {now}");
        assert!(c <= now + 1_000 && now - c < 60_000, "created {c} vs now {now}");
    }

    #[test]
    fn modified_tracks_later_writes() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old.md");
        let new = dir.path().join("new.md");
        fs::write(&old, "o").unwrap();
        fs::write(&new, "n").unwrap();
        let hour_ago = SystemTime::now() - Duration::from_secs(3600);
        fs::File::options().write(true).open(&old).unwrap().set_modified(hour_ago).unwrap();
        let entries = list_dir_with(dir.path().to_string_lossy().to_string(), false, true).unwrap();
        let get = |n: &str| entries.iter().find(|e| e.name == n).unwrap().modified.unwrap();
        assert!(get("new.md") > get("old.md") + 3_000_000);
    }

    #[test]
    fn serialization_omits_absent_times() {
        let e = DirEntry { name: "x".into(), path: "/x".into(), is_dir: false, modified: None, created: None };
        let json = serde_json::to_string(&e).unwrap();
        assert!(!json.contains("modified") && !json.contains("created"), "{json}");
    }
}

#[cfg(test)]
mod encoding_round_trip_tests {
    use super::*;

    fn round_trip(encoding: &str, original: &[u8]) -> (FileReadResult, Vec<u8>) {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f.txt");
        fs::write(&p, original).unwrap();
        let path = p.to_string_lossy().to_string();
        let read = read_file_inner(path.clone()).unwrap();
        assert_eq!(read.encoding, encoding);
        write_file_inner(path.clone(), read.content.clone(), read.encoding.clone()).unwrap();
        let reread = read_file_inner(path).unwrap();
        assert_eq!(reread.encoding, encoding);
        assert_eq!(reread.content, read.content);
        (read, fs::read(&p).unwrap())
    }

    fn utf16(text: &str, le: bool) -> Vec<u8> {
        let mut bytes = if le { vec![0xFF, 0xFE] } else { vec![0xFE, 0xFF] };
        for u in text.encode_utf16() {
            bytes.extend_from_slice(&if le { u.to_le_bytes() } else { u.to_be_bytes() });
        }
        bytes
    }

    #[test]
    fn utf16le_with_bom_round_trips() {
        let text = "UTF-16 文件\nline 2 😀";
        let bytes = utf16(text, true);
        let (read, disk) = round_trip("UTF-16LE", &bytes);
        assert!(read.had_bom);
        assert_eq!(read.content, text);
        assert_eq!(disk, bytes);
    }

    #[test]
    fn utf16be_with_bom_round_trips() {
        let text = "UTF-16 文件\nline 2 😀";
        let bytes = utf16(text, false);
        let (read, disk) = round_trip("UTF-16BE", &bytes);
        assert!(read.had_bom);
        assert_eq!(read.content, text);
        assert_eq!(disk, bytes);
    }

    #[test]
    fn gbk_round_trips() {
        let text = "这是一个中文文件，用来测试 GBK 编码的读取和保存。\n第二行内容也是中文。";
        let (bytes, _, _) = encoding_rs::GBK.encode(text);
        let (read, disk) = round_trip("GBK", &bytes);
        assert_eq!(read.content, text);
        assert_eq!(disk, bytes.as_ref());
    }

    #[test]
    fn big5_round_trips() {
        let text = "這是一個繁體中文檔案，用來測試大五碼的讀取與儲存。\n第二行內容也是繁體中文。";
        let (bytes, _, _) = encoding_rs::BIG5.encode(text);
        let (read, disk) = round_trip("Big5", &bytes);
        assert_eq!(read.content, text);
        assert_eq!(disk, bytes.as_ref());
    }
}
