//! Workspace index — shared backbone for v2.0 features (F1 wikilinks/
//! backlinks, F3 tags panel, F6 Bases-style properties view).
//!
//! Scans the active workspace folder for `.md`/`.markdown` files, extracts
//! YAML front matter, wikilinks (`[[X]]`), `#tags`, and headings. Holds
//! everything in-memory; persists a JSON cache per workspace at
//! `<app_data_dir>/index/<hash(folder)>.json` so warm starts are fast.
//!
//! A file watcher (notify) keeps the index live; on debounced changes the
//! affected file is re-scanned and an `solomd://index-updated` event is
//! emitted to the frontend.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::hash::{DefaultHasher, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, RwLock};
use std::time::{Duration, SystemTime};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, Manager};
use walkdir::WalkDir;

// ---------------------------------------------------------------------------
// Public types (match the TS interface in `app/src/lib/workspace-index.ts`).
// ---------------------------------------------------------------------------

/// One `- [ ]` / `- [x]` item, as found in a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRef {
    pub line: u32,
    pub text: String,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    pub path: String,
    pub name: String,
    pub stem: String,
    pub mtime: u64,
    pub size: u64,
    pub frontmatter: serde_json::Value,
    pub wikilinks: Vec<WikilinkRef>,
    pub tags: Vec<String>,
    pub headings: Vec<String>,
    #[serde(default)]
    pub tasks: Vec<TaskRef>,
    pub summary: String,
    pub title: Option<String>,
    #[serde(default)]
    pub relationships: HashMap<String, Vec<String>>,
}

/// F3 — one inverse (referenced-by) edge resolved over the in-memory index.
#[derive(Debug, Clone, Serialize)]
pub struct ReferencedByRef {
    pub from_path: String,
    pub from_name: String,
    pub via_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikilinkRef {
    pub target: String,
    pub heading: Option<String>,
    pub alias: Option<String>,
    pub line: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct BacklinkRef {
    pub from_path: String,
    pub from_name: String,
    pub line: u32,
    pub context: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TagCount {
    pub tag: String,
    pub count: u32,
    pub files: Vec<String>,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

struct State {
    root: Option<PathBuf>,
    entries: HashMap<PathBuf, IndexEntry>,
    _watcher: Option<RecommendedWatcher>,
}

static STATE: LazyLock<RwLock<State>> = LazyLock::new(|| {
    RwLock::new(State {
        root: None,
        entries: HashMap::new(),
        _watcher: None,
    })
});

static PENDING: LazyLock<Mutex<HashMap<PathBuf, SystemTime>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn workspace_index_init(app: AppHandle, folder: String) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || workspace_index_init_inner(app, folder))
        .await
        .map_err(|e| format!("join: {e}"))?
}

fn workspace_index_init_inner(app: AppHandle, folder: String) -> Result<usize, String> {
    let root = PathBuf::from(&folder);
    if !root.is_dir() {
        return Err(format!("not a directory: {folder}"));
    }

    {
        let mut s = STATE.write().map_err(|e| e.to_string())?;
        s.entries.clear();
        s.root = Some(root.clone());
        s._watcher = None;
    }

    if let Some(cached) = load_cache(&app, &root) {
        let mut s = STATE.write().map_err(|e| e.to_string())?;
        for entry in cached {
            s.entries.insert(PathBuf::from(&entry.path), entry);
        }
    }

    scan_into(&root)?;

    let app_for_watch = app.clone();
    let watcher = make_watcher(app_for_watch, root.clone())?;
    {
        let mut s = STATE.write().map_err(|e| e.to_string())?;
        s._watcher = Some(watcher);
    }

    let count = STATE.read().map_err(|e| e.to_string())?.entries.len();
    save_cache(&app, &root)?;
    let _ = app.emit("solomd://index-updated", &"init");
    Ok(count)
}

#[tauri::command]
pub fn workspace_index_files() -> Result<Vec<IndexEntry>, String> {
    let s = STATE.read().map_err(|e| e.to_string())?;
    let mut v: Vec<IndexEntry> = s.entries.values().cloned().collect();
    v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(v)
}

#[tauri::command]
pub fn workspace_index_backlinks(target: String) -> Result<Vec<BacklinkRef>, String> {
    let s = STATE.read().map_err(|e| e.to_string())?;
    let target_lc = target.to_lowercase();
    let mut out: Vec<BacklinkRef> = Vec::new();
    for entry in s.entries.values() {
        for link in &entry.wikilinks {
            if link.target.to_lowercase() == target_lc {
                let context = read_context(&PathBuf::from(&entry.path), link.line);
                out.push(BacklinkRef {
                    from_path: entry.path.clone(),
                    from_name: entry.name.clone(),
                    line: link.line,
                    context,
                });
            }
        }
    }
    out.sort_by(|a, b| a.from_name.cmp(&b.from_name));
    Ok(out)
}

#[tauri::command]
pub fn workspace_index_referenced_by(target: String) -> Result<Vec<ReferencedByRef>, String> {
    let s = STATE.read().map_err(|e| e.to_string())?;
    let needle = target.trim().to_lowercase();
    if needle.is_empty() {
        return Ok(Vec::new());
    }

    let resolved_path: Option<String> = {
        let by_stem = s
            .entries
            .values()
            .find(|e| e.stem.to_lowercase() == needle);
        let by_title = || {
            s.entries.values().find(|e| {
                e.title
                    .as_ref()
                    .map(|t| t.to_lowercase() == needle)
                    .unwrap_or(false)
            })
        };
        let by_alias = || {
            s.entries.values().find(|e| {
                if let serde_json::Value::Object(map) = &e.frontmatter {
                    if let Some(a) = map.get("aliases").or_else(|| map.get("alias")) {
                        let mut names = Vec::new();
                        collect_alias_names(a, &mut names);
                        return names.iter().any(|n| n.to_lowercase() == needle);
                    }
                }
                false
            })
        };
        by_stem
            .or_else(by_title)
            .or_else(by_alias)
            .map(|e| e.path.clone())
    };

    let mut out: Vec<ReferencedByRef> = Vec::new();
    for entry in s.entries.values() {
        for (via_key, refs) in &entry.relationships {
            for reference in refs {
                let ref_target = parse_ref_target(reference).to_lowercase();
                let matches_needle = ref_target == needle;
                let matches_resolved = match &resolved_path {
                    Some(p) => s
                        .entries
                        .get(&PathBuf::from(p))
                        .map(|te| te.stem.to_lowercase() == ref_target)
                        .unwrap_or(false),
                    None => false,
                };
                if !(matches_needle || matches_resolved) {
                    continue;
                }
                if let Some(p) = &resolved_path {
                    if &entry.path == p {
                        continue;
                    }
                }
                out.push(ReferencedByRef {
                    from_path: entry.path.clone(),
                    from_name: entry.name.clone(),
                    via_key: via_key.clone(),
                });
            }
        }
    }
    out.sort_by(|a, b| {
        a.via_key
            .cmp(&b.via_key)
            .then_with(|| a.from_name.cmp(&b.from_name))
    });
    out.dedup_by(|a, b| a.from_path == b.from_path && a.via_key == b.via_key);
    Ok(out)
}

fn collect_alias_names(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => out.push(s.trim().to_string()),
        serde_json::Value::Array(arr) => {
            for v in arr {
                collect_alias_names(v, out);
            }
        }
        _ => {}
    }
}

#[tauri::command]
pub fn workspace_index_tags() -> Result<Vec<TagCount>, String> {
    let s = STATE.read().map_err(|e| e.to_string())?;
    let mut by_tag: HashMap<String, (u32, Vec<String>)> = HashMap::new();
    for entry in s.entries.values() {
        let mut seen_in_file: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for tag in &entry.tags {
            if seen_in_file.insert(tag) {
                let e = by_tag.entry(tag.clone()).or_insert_with(|| (0, vec![]));
                e.0 += 1;
                e.1.push(entry.path.clone());
            }
        }
    }
    let mut out: Vec<TagCount> = by_tag
        .into_iter()
        .map(|(tag, (count, files))| TagCount { tag, count, files })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.tag.cmp(&b.tag)));
    Ok(out)
}

#[tauri::command]
pub fn workspace_index_resolve(name: String) -> Result<Option<String>, String> {
    let s = STATE.read().map_err(|e| e.to_string())?;
    let needle = name.trim();
    if needle.is_empty() {
        return Ok(None);
    }
    let needle_lc = needle.to_lowercase();
    for entry in s.entries.values() {
        if entry.stem.to_lowercase() == needle_lc {
            return Ok(Some(entry.path.clone()));
        }
    }
    for entry in s.entries.values() {
        if entry
            .title
            .as_ref()
            .map(|t| t.to_lowercase() == needle_lc)
            .unwrap_or(false)
        {
            return Ok(Some(entry.path.clone()));
        }
    }
    for entry in s.entries.values() {
        if let serde_json::Value::Object(map) = &entry.frontmatter {
            if let Some(a) = map.get("aliases").or_else(|| map.get("alias")) {
                let mut names = Vec::new();
                collect_alias_names(a, &mut names);
                if names.iter().any(|n| n.to_lowercase() == needle_lc) {
                    return Ok(Some(entry.path.clone()));
                }
            }
        }
    }
    {
        let needle_norm = needle_lc.replace('\\', "/");
        let needle_no_ext = needle_norm
            .strip_suffix(".md")
            .unwrap_or(&needle_norm)
            .to_string();
        for entry in s.entries.values() {
            let path_norm = entry.path.replace('\\', "/").to_lowercase();
            if path_norm.ends_with(&format!("/{}", needle_norm))
                || path_norm.ends_with(&format!("/{}", needle_no_ext))
                || path_norm.ends_with(&format!("/{}", format!("{}.md", needle_no_ext)))
            {
                return Ok(Some(entry.path.clone()));
            }
        }
    }
    for entry in s.entries.values() {
        if entry.stem.to_lowercase().contains(&needle_lc) {
            return Ok(Some(entry.path.clone()));
        }
    }
    Ok(None)
}

#[tauri::command]
pub async fn workspace_index_rescan(app: AppHandle) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || workspace_index_rescan_inner(app))
        .await
        .map_err(|e| format!("join: {e}"))?
}

fn workspace_index_rescan_inner(app: AppHandle) -> Result<usize, String> {
    let root = {
        let s = STATE.read().map_err(|e| e.to_string())?;
        match &s.root {
            Some(r) => r.clone(),
            None => return Err("workspace not initialized".into()),
        }
    };
    {
        let mut s = STATE.write().map_err(|e| e.to_string())?;
        s.entries.clear();
    }
    scan_into(&root)?;
    save_cache(&app, &root)?;
    let _ = app.emit("solomd://index-updated", &"rescan");
    let count = STATE.read().map_err(|e| e.to_string())?.entries.len();
    Ok(count)
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

fn scan_into(root: &Path) -> Result<(), String> {
    let mut new_entries: HashMap<PathBuf, IndexEntry> = HashMap::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let lower = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();
        if lower != "md" && lower != "markdown" && lower != "mdown" {
            continue;
        }
        if let Ok(idx) = scan_file(path) {
            new_entries.insert(path.to_path_buf(), idx);
        }
    }
    let mut s = STATE.write().map_err(|e| e.to_string())?;
    s.entries = new_entries;
    Ok(())
}

fn scan_file(path: &Path) -> Result<IndexEntry, String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    let (frontmatter, body) = split_front_matter(&raw);
    let frontmatter_json: serde_json::Value = match frontmatter {
        Some(fm) => parse_frontmatter_yaml(&fm),
        None => serde_json::Value::Null,
    };

    let wikilinks = extract_wikilinks(body, body_line_offset(&raw, body));
    let mut tags = extract_body_tags(body);
    if let serde_json::Value::Object(map) = &frontmatter_json {
        if let Some(t) = map.get("tags") {
            collect_yaml_tags(t, &mut tags);
        }
    }
    tags.sort();
    tags.dedup();

    let headings = extract_headings(body);
    let tasks = extract_tasks(&raw);
    let title = extract_title(&frontmatter_json, &headings);
    let summary = extract_summary(body);
    let relationships = extract_relationships(&frontmatter_json);

    Ok(IndexEntry {
        path: path.to_string_lossy().to_string(),
        name,
        stem,
        mtime,
        size: meta.len(),
        frontmatter: frontmatter_json,
        wikilinks,
        tags,
        headings,
        tasks,
        summary,
        title,
        relationships,
    })
}

// ---------------------------------------------------------------------------
// Frontmatter & Relationships
// ---------------------------------------------------------------------------

const RESERVED_RELATIONSHIP_KEYS: &[&str] = &[
    "title",
    "aliases",
    "alias",
    "tags",
    "tag",
    "status",
    "date",
    "created",
    "modified",
    "updated",
    "icon",
    "color",
    "colour",
    "cssclass",
    "cssclasses",
    "publish",
    "permalink",
    "inbox",
];

fn is_reserved_key(key: &str) -> bool {
    let k = key.trim();
    if k.is_empty() || k.starts_with('_') {
        return true;
    }
    let lc = k.to_lowercase();
    RESERVED_RELATIONSHIP_KEYS.iter().any(|r| *r == lc)
}

fn find_wikilinks(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        let after_start = &rest[start + 2..];
        if let Some(end) = after_start.find("]]") {
            let inner = &after_start[..end];
            if !inner.contains('\n') && !inner.contains('[') && !inner.contains(']') && !inner.trim().is_empty() {
                out.push(inner);
            }
            rest = &after_start[end + 2..];
        } else {
            break;
        }
    }
    out
}

fn refs_in_str(text: &str, out: &mut Vec<String>) {
    for inner in find_wikilinks(text) {
        let target_raw = match inner.split_once('|') {
            Some((t, _)) => t.trim().to_string(),
            None => inner.trim().to_string(),
        };
        let target = match target_raw.split_once('#') {
            Some((t, _)) => t.trim().to_string(),
            None => target_raw,
        };
        if !target.is_empty() {
            out.push(format!("[[{target}]]"));
        }
    }
}

fn collect_refs(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => refs_in_str(s, out),
        serde_json::Value::Array(arr) => {
            for v in arr {
                collect_refs(v, out);
            }
        }
        _ => {}
    }
}

fn extract_relationships(fm: &serde_json::Value) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    if let serde_json::Value::Object(map) = fm {
        for (key, val) in map {
            if is_reserved_key(key) {
                continue;
            }
            let mut refs = Vec::new();
            collect_refs(val, &mut refs);
            if !refs.is_empty() {
                refs.sort();
                refs.dedup();
                out.insert(key.clone(), refs);
            }
        }
    }
    out
}

fn parse_ref_target(reference: &str) -> String {
    let mut v = Vec::new();
    refs_in_str(reference, &mut v);
    if let Some(first) = v.first() {
        return first
            .trim_start_matches("[[")
            .trim_end_matches("]]")
            .to_string();
    }
    reference.trim().to_string()
}

fn split_front_matter(raw: &str) -> (Option<String>, &str) {
    let trimmed = raw.trim_start_matches('\u{feff}');
    if !trimmed.starts_with("---") {
        return (None, raw);
    }
    let after_first = match trimmed.find('\n') {
        Some(i) => &trimmed[i + 1..],
        None => return (None, raw),
    };
    if let Some(end) = after_first.find("\n---") {
        let yaml = &after_first[..end];
        let rest_offset_in_after = end + "\n---".len();
        let rest = &after_first[rest_offset_in_after..];
        let rest = rest.strip_prefix('\n').unwrap_or(rest);
        return (Some(yaml.to_string()), rest);
    }
    (None, raw)
}

pub(crate) fn body_line_offset(raw: &str, body: &str) -> u32 {
    let start = body.as_ptr() as usize;
    let base = raw.as_ptr() as usize;
    if start < base || start > base + raw.len() {
        return 0;
    }
    raw.as_bytes()[..start - base]
        .iter()
        .filter(|&&b| b == b'\n')
        .count() as u32
}

fn extract_wikilinks(body: &str, line_offset: u32) -> Vec<WikilinkRef> {
    let mut out = Vec::new();
    for (line_idx, line) in body.lines().enumerate() {
        for inner in find_wikilinks(line) {
            let (target_raw, alias) = match inner.split_once('|') {
                Some((t, a)) => (t.trim().to_string(), Some(a.trim().to_string())),
                None => (inner.trim().to_string(), None),
            };
            let (target, heading) = match target_raw.split_once('#') {
                Some((t, h)) => (t.trim().to_string(), Some(h.trim().to_string())),
                None => (target_raw, None),
            };
            if target.is_empty() {
                continue;
            }
            out.push(WikilinkRef {
                target,
                heading,
                alias,
                line: line_offset + (line_idx as u32) + 1,
            });
        }
    }
    out
}

fn extract_body_tags(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let stripped_line = strip_inline_code(line);
        let chars: Vec<char> = stripped_line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            let preceded_ok = i == 0 || chars[i - 1].is_whitespace();
            if c == '#' && preceded_ok {
                if i + 1 < chars.len() && chars[i + 1].is_alphanumeric() {
                    let mut j = i + 1;
                    while j < chars.len()
                        && (chars[j].is_alphanumeric()
                            || chars[j] == '_'
                            || chars[j] == '/'
                            || chars[j] == '-')
                    {
                        j += 1;
                    }
                    let tag: String = chars[i + 1..j].iter().collect();
                    if !tag.chars().all(|c| c.is_ascii_digit()) {
                        out.push(tag);
                    }
                    i = j;
                    continue;
                }
            }
            i += 1;
        }
    }
    out
}

fn strip_inline_code(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    for ch in line.chars() {
        if ch == '`' {
            in_code = !in_code;
            out.push(' ');
        } else if in_code {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

fn collect_yaml_tags(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => {
            for piece in s.split(|c: char| c == ',' || c.is_whitespace()) {
                let t = piece.trim().trim_start_matches('#');
                if !t.is_empty() {
                    out.push(t.to_string());
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                collect_yaml_tags(v, out);
            }
        }
        _ => {}
    }
}

pub fn parse_frontmatter_yaml(src: &str) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    let mut current_key: Option<String> = None;
    let mut current_list: Option<Vec<serde_json::Value>> = None;

    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(item_str) = trimmed.strip_prefix('-') {
            let item_val = item_str.trim();
            if let Some(list) = &mut current_list {
                list.push(parse_yaml_scalar(item_val));
                continue;
            }
        }

        if let (Some(k), Some(list)) = (current_key.take(), current_list.take()) {
            map.insert(k, serde_json::Value::Array(list));
        }

        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().trim_matches(['\'', '"']).to_string();
            if key.is_empty() {
                continue;
            }
            let val = v.trim();
            if val.is_empty() {
                current_key = Some(key);
                current_list = Some(Vec::new());
            } else if val.starts_with('[') && val.ends_with(']') {
                let inner = &val[1..val.len() - 1];
                let items: Vec<serde_json::Value> = inner
                    .split(',')
                    .map(|s| parse_yaml_scalar(s.trim()))
                    .filter(|v| match v {
                        serde_json::Value::String(s) => !s.is_empty(),
                        _ => true,
                    })
                    .collect();
                map.insert(key, serde_json::Value::Array(items));
            } else {
                map.insert(key, parse_yaml_scalar(val));
            }
        }
    }

    if let (Some(k), Some(list)) = (current_key, current_list) {
        map.insert(k, serde_json::Value::Array(list));
    }

    serde_json::Value::Object(map)
}

fn parse_yaml_scalar(val: &str) -> serde_json::Value {
    let unquoted = val.trim_matches(['\'', '"']);
    if unquoted.eq_ignore_ascii_case("true") {
        serde_json::Value::Bool(true)
    } else if unquoted.eq_ignore_ascii_case("false") {
        serde_json::Value::Bool(false)
    } else if unquoted.eq_ignore_ascii_case("null") || unquoted == "~" {
        serde_json::Value::Null
    } else if let Ok(n) = unquoted.parse::<i64>() {
        serde_json::Value::Number(n.into())
    } else if let Ok(f) = unquoted.parse::<f64>() {
        if let Some(num) = serde_json::Number::from_f64(f) {
            serde_json::Value::Number(num)
        } else {
            serde_json::Value::String(unquoted.to_string())
        }
    } else {
        serde_json::Value::String(unquoted.to_string())
    }
}

fn extract_headings(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for line in body.lines() {
        let trimmed_start = line.trim_start();
        if trimmed_start.starts_with("```") || trimmed_start.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let hashes = line.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&hashes) {
            let rest = &line[hashes..];
            if rest.starts_with(' ') || rest.starts_with('\t') {
                let heading = rest.trim();
                if !heading.is_empty() {
                    out.push(heading.to_string());
                }
            }
        }
    }
    out
}

fn parse_task_line(line: &str) -> Option<(bool, String)> {
    let trimmed = line.trim_start();
    let bullet_rest = if let Some(r) = trimmed.strip_prefix("- ") {
        r
    } else if let Some(r) = trimmed.strip_prefix("* ") {
        r
    } else if let Some(r) = trimmed.strip_prefix("+ ") {
        r
    } else {
        return None;
    };
    let bullet_trimmed = bullet_rest.trim_start();
    let (done, after_box) = if let Some(r) = bullet_trimmed.strip_prefix("[ ] ") {
        (false, r)
    } else if let Some(r) = bullet_trimmed.strip_prefix("[x] ") {
        (true, r)
    } else if let Some(r) = bullet_trimmed.strip_prefix("[X] ") {
        (true, r)
    } else {
        return None;
    };
    let text = after_box.trim();
    if text.is_empty() {
        None
    } else {
        Some((done, text.to_string()))
    }
}

fn extract_tasks(raw: &str) -> Vec<TaskRef> {
    let mut out = Vec::new();
    let mut in_fence = false;
    let mut in_front_matter = false;
    for (idx, line) in raw.lines().enumerate() {
        let trimmed = line.trim_start();
        if idx == 0 && line.trim_end() == "---" {
            in_front_matter = true;
            continue;
        }
        if in_front_matter {
            if line.trim_end() == "---" || line.trim_end() == "..." {
                in_front_matter = false;
            }
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some((done, text)) = parse_task_line(line) {
            out.push(TaskRef {
                line: (idx as u32) + 1,
                text,
                done,
            });
        }
    }
    out
}

fn extract_title(fm: &serde_json::Value, headings: &[String]) -> Option<String> {
    if let serde_json::Value::Object(map) = fm {
        if let Some(serde_json::Value::String(t)) = map.get("title") {
            let trimmed = t.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    headings.first().cloned()
}

fn extract_summary(body: &str) -> String {
    let mut in_fence = false;
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let cleaned: String = trimmed.chars().take(200).collect();
        return cleaned;
    }
    String::new()
}

// ---------------------------------------------------------------------------
// Watcher
// ---------------------------------------------------------------------------

fn make_watcher(app: AppHandle, root: PathBuf) -> Result<RecommendedWatcher, String> {
    let app_for_handler = app.clone();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        if let Ok(ev) = res {
            handle_event(&app_for_handler, ev);
        }
    })
    .map_err(|e| e.to_string())?;
    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|e| e.to_string())?;
    Ok(watcher)
}

fn handle_event(app: &AppHandle, event: Event) {
    let interesting = matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    );
    if !interesting {
        return;
    }
    let now = SystemTime::now();
    let mut to_rescan: Vec<PathBuf> = Vec::new();
    {
        let mut pending = match PENDING.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
        for path in event.paths {
            let lower = path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();
            if lower != "md" && lower != "markdown" && lower != "mdown" {
                continue;
            }
            pending.insert(path.clone(), now);
            to_rescan.push(path);
        }
    }
    if to_rescan.is_empty() {
        return;
    }
    let app_for_thread = app.clone();
    let paths = to_rescan.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        let now2 = SystemTime::now();
        let still_pending: Vec<PathBuf> = {
            let pending = match PENDING.lock() {
                Ok(p) => p,
                Err(_) => return,
            };
            paths
                .into_iter()
                .filter(|p| {
                    pending
                        .get(p)
                        .map(|t| now2.duration_since(*t).map(|d| d.as_millis() >= 180).unwrap_or(true))
                        .unwrap_or(false)
                })
                .collect()
        };
        if still_pending.is_empty() {
            return;
        }
        let mut changed = false;
        for path in &still_pending {
            if path.is_file() {
                if let Ok(idx) = scan_file(path) {
                    if let Ok(mut s) = STATE.write() {
                        s.entries.insert(path.clone(), idx);
                        changed = true;
                    }
                }
            } else {
                if let Ok(mut s) = STATE.write() {
                    if s.entries.remove(path).is_some() {
                        changed = true;
                    }
                }
            }
        }
        if let Ok(mut pending) = PENDING.lock() {
            for p in &still_pending {
                pending.remove(p);
            }
        }
        if changed {
            let root = STATE.read().ok().and_then(|s| s.root.clone());
            if let Some(root) = root {
                let _ = save_cache(&app_for_thread, &root);
            }
            let _ = app_for_thread.emit("solomd://index-updated", &"watch");
        }
    });
}

// ---------------------------------------------------------------------------
// Cache (JSON on disk per workspace)
// ---------------------------------------------------------------------------

fn cache_path(app: &AppHandle, root: &Path) -> Option<PathBuf> {
    let app_data = app.path().app_data_dir().ok()?;
    let mut hasher = DefaultHasher::new();
    hasher.write(root.to_string_lossy().as_bytes());
    let hash = format!("{:016x}", hasher.finish());
    let dir = app_data.join("index");
    let _ = fs::create_dir_all(&dir);
    Some(dir.join(format!("{}.json", hash)))
}

fn save_cache(app: &AppHandle, root: &Path) -> Result<(), String> {
    let path = match cache_path(app, root) {
        Some(p) => p,
        None => return Ok(()),
    };
    let s = STATE.read().map_err(|e| e.to_string())?;
    let entries: Vec<&IndexEntry> = s.entries.values().collect();
    let json = serde_json::to_vec_pretty(&entries).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

fn load_cache(app: &AppHandle, root: &Path) -> Option<Vec<IndexEntry>> {
    let path = cache_path(app, root)?;
    if !path.exists() {
        return None;
    }
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn read_context(path: &Path, line_no: u32) -> Vec<String> {
    let raw = match fs::read_to_string(path) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let lines: Vec<&str> = raw.lines().collect();
    let i = (line_no as usize).saturating_sub(1);
    let prev = if i > 0 { lines.get(i - 1) } else { None };
    let cur = lines.get(i);
    let next = lines.get(i + 1);
    [prev, cur, next]
        .iter()
        .filter_map(|x| x.map(|s| s.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_inline_code_basic() {
        let stripped = strip_inline_code("purple bg `#1A1033` + text `#9B7EE0`");
        assert!(!stripped.contains('#'), "inline code should be stripped: {:?}", stripped);
    }

    fn fm(yaml: &str) -> serde_json::Value {
        parse_frontmatter_yaml(yaml)
    }

    #[test]
    fn relationships_scalar_and_array() {
        let v = fm("belongs_to: \"[[b]]\"\ncites:\n  - \"[[a]]\"\n  - \"[[c]]\"");
        let rel = extract_relationships(&v);
        assert_eq!(rel.get("belongs_to").unwrap(), &vec!["[[b]]".to_string()]);
        assert_eq!(
            rel.get("cites").unwrap(),
            &vec!["[[a]]".to_string(), "[[c]]".to_string()]
        );
    }

    #[test]
    fn parse_ref_target_strips_brackets() {
        assert_eq!(parse_ref_target("[[b]]"), "b");
        assert_eq!(parse_ref_target("[[a|alias]]"), "a");
        assert_eq!(parse_ref_target("[[a#h]]"), "a");
        assert_eq!(parse_ref_target("plain"), "plain");
    }
}
