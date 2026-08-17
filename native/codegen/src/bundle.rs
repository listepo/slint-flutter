//! Pack the compiled `.slint` graph into the blob generated wrappers embed.
//!
//! A generated `.slint.dart` must not read `.slint` from disk: the file may not
//! ship with the application at all. So every file the compiler touched goes
//! into the wrapper — imports rewritten to a virtual root and `@image-url`
//! inlined as data URIs — as one gzip+base64 string. The runtime hands that
//! string back to the interpreter, which compiles it without a filesystem.
//!
//! `rust/compiled.rs` is the other half: it decodes this and instantiates.

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use i_slint_compiler::object_tree::Document;
use i_slint_compiler::pathutils;
use i_slint_compiler::typeloader::TypeLoader;

/// Bumped whenever the blob layout changes; the runtime refuses anything else.
const MODULE_VERSION: u32 = 1;
const VIRTUAL_ROOT: &str = "/slint-aot";

struct BundledFile {
    path: PathBuf,
    source: String,
    imports: Vec<(String, PathBuf)>,
}

/// Pack the compiled document graph into the blob generated wrappers embed.
pub fn bundle(main: &Document, loader: &TypeLoader, style: Option<&str>) -> Result<String, String> {
    let mut files = Vec::new();
    collect_document(&mut files, main)?;
    for path in loader.all_files() {
        if let Some(document) = loader.get_document(path) {
            collect_document(&mut files, document)?;
        }
    }
    if files.is_empty() {
        return Err("The Slint compiler produced no files to embed".into());
    }

    let root = common_directory(files.iter().map(|file| file.path.as_path()));
    let mapping = files
        .iter()
        .map(|file| (file.path.clone(), virtual_path(&file.path, &root)))
        .collect::<HashMap<_, _>>();

    let mut bundled = BTreeMap::new();
    for file in &files {
        let virtual_url = mapping.get(&file.path).expect("every bundled file is mapped").clone();
        let mut source = file.source.clone();
        source = rewrite_imports(&source, &file.imports, &mapping);
        source = rewrite_image_urls(&source, file.path.parent().unwrap_or(&file.path))?;
        bundled.insert(virtual_url, source);
    }

    let main_path = document_path(main)
        .ok_or_else(|| "Cannot determine the path of the main Slint file".to_string())?;
    let main = mapping
        .get(&main_path)
        .cloned()
        .ok_or_else(|| "The main Slint file was not bundled".to_string())?;

    let assets = collect_font_assets(&files, &mapping)?;

    let mut json = serde_json::json!({
        "v": MODULE_VERSION,
        "style": style,
        "main": main,
        "files": bundled,
    });
    if !assets.is_empty() {
        json["assets"] = serde_json::Value::Object(
            assets
                .into_iter()
                .map(|(path, data)| (path, serde_json::Value::String(data)))
                .collect(),
        );
    }
    encode_json(&json)
}

fn collect_document(files: &mut Vec<BundledFile>, document: &Document) -> Result<(), String> {
    let path = document_path(document).ok_or_else(|| "A Slint document has no path".to_string())?;
    if is_builtin(&path) || files.iter().any(|file| file.path == path) {
        return Ok(());
    }
    let source = document
        .node
        .as_ref()
        .and_then(|node| node.source_file.source().map(str::to_string))
        .or_else(|| std::fs::read_to_string(&path).ok())
        .ok_or_else(|| format!("Cannot read {}", path.display()))?;
    let imports = document
        .imports
        .iter()
        .map(|import| {
            (import.import_uri_token.text().to_string(), PathBuf::from(import.file.as_str()))
        })
        .collect();
    files.push(BundledFile { path, source, imports });
    Ok(())
}

fn document_path(document: &Document) -> Option<PathBuf> {
    document.node.as_ref().map(|node| pathutils::clean_path(node.source_file.path()))
}

fn is_builtin(path: &Path) -> bool {
    path.to_string_lossy().starts_with("builtin:")
}

fn common_directory<'a>(paths: impl Iterator<Item = &'a Path>) -> PathBuf {
    let mut directories =
        paths.filter_map(|path| path.parent().map(Path::to_path_buf)).collect::<Vec<_>>();
    let Some(mut prefix) = directories.pop() else {
        return PathBuf::from(".");
    };
    for directory in directories {
        while !directory.starts_with(&prefix) {
            match prefix.parent() {
                Some(parent) if parent != prefix => prefix = parent.to_path_buf(),
                _ => return PathBuf::from("."),
            }
        }
    }
    prefix
}

fn virtual_path(real: &Path, root: &Path) -> String {
    let relative = pathdiff::diff_paths(real, root).unwrap_or_else(|| {
        PathBuf::from(real.file_name().unwrap_or_else(|| std::ffi::OsStr::new("component.slint")))
    });
    let relative = relative.to_string_lossy().replace('\\', "/");
    format!("{VIRTUAL_ROOT}/{relative}")
}

fn rewrite_imports(
    source: &str,
    imports: &[(String, PathBuf)],
    mapping: &HashMap<PathBuf, String>,
) -> String {
    let mut result = source.to_string();
    for (quoted, resolved) in imports {
        let resolved = pathutils::clean_path(resolved);
        if is_builtin(&resolved) {
            continue;
        }
        let Some(virtual_url) = mapping.get(&resolved) else {
            continue;
        };
        let replacement = format!("\"{virtual_url}\"");
        if result.contains(quoted) {
            result = result.replacen(quoted, &replacement, 1);
        }
    }
    result
}

fn rewrite_image_urls(source: &str, source_dir: &Path) -> Result<String, String> {
    let mut replacements = Vec::new();
    let mut search_from = 0;
    while let Some(relative) = source[search_from..].find("@image-url") {
        let start = search_from + relative;
        let after_name = start + "@image-url".len();
        let rest = source[after_name..].trim_start();
        let skipped = source[after_name..].len() - rest.len();
        if !rest.starts_with('(') {
            search_from = after_name;
            continue;
        }
        let after_paren = rest[1..].trim_start();
        let skipped_paren = rest[1..].len() - after_paren.len();
        let quote_index = after_name + skipped + 1 + skipped_paren;
        let Some(quote) = after_paren.chars().next() else {
            break;
        };
        if quote != '"' && quote != '\'' {
            search_from = quote_index + quote.len_utf8();
            continue;
        }
        let content_start = quote_index + quote.len_utf8();
        let Some(end) = source[content_start..].find(quote) else {
            break;
        };
        let path = &source[content_start..content_start + end];
        if !path.starts_with("data:") && !path.starts_with("builtin:") {
            replacements.push((content_start, content_start + end, path.to_string()));
        }
        search_from = content_start + end + quote.len_utf8();
    }

    let mut result = source.to_string();
    for (start, end, path) in replacements.into_iter().rev() {
        let resolved = if Path::new(&path).is_absolute() {
            PathBuf::from(&path)
        } else {
            source_dir.join(&path)
        };
        let Ok(bytes) = std::fs::read(&resolved) else {
            continue;
        };
        let uri = data_uri(&resolved, &bytes);
        result.replace_range(start..end, &uri);
    }
    Ok(result)
}

fn data_uri(path: &Path, bytes: &[u8]) -> String {
    let mime = match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("bmp") => "image/bmp",
        _ => "application/octet-stream",
    };
    format!("data:{mime};base64,{}", STANDARD.encode(bytes))
}

fn is_font_import(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".ttf") || lower.ends_with(".ttc") || lower.ends_with(".otf")
}

fn extract_font_imports(source: &str) -> Vec<String> {
    let mut imports = Vec::new();
    for line in source.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("import ") else {
            continue;
        };
        let rest = rest.trim();
        if rest.starts_with('{') {
            continue;
        }
        let Some((path, _)) = rest.split_once(';') else {
            continue;
        };
        let path = path.trim();
        let path = path
            .strip_prefix('"')
            .and_then(|p| p.strip_suffix('"'))
            .or_else(|| path.strip_prefix('\'').and_then(|p| p.strip_suffix('\'')))
            .unwrap_or(path);
        if is_font_import(path) {
            imports.push(path.to_string());
        }
    }
    imports
}

fn virtual_font_path(virtual_slint: &str, import: &str) -> String {
    let slint_path = Path::new(virtual_slint);
    let parent = slint_path.parent().unwrap_or(Path::new(VIRTUAL_ROOT));
    pathutils::join(parent, Path::new(import))
        .unwrap_or_else(|| parent.join(import))
        .to_string_lossy()
        .replace('\\', "/")
}

fn collect_font_assets(
    files: &[BundledFile],
    mapping: &HashMap<PathBuf, String>,
) -> Result<BTreeMap<String, String>, String> {
    let mut assets = BTreeMap::new();
    for file in files {
        let Some(virtual_slint) = mapping.get(&file.path) else {
            continue;
        };
        let source_dir = file.path.parent().unwrap_or(&file.path);
        for import in extract_font_imports(&file.source) {
            let resolved = if Path::new(&import).is_absolute() {
                PathBuf::from(&import)
            } else {
                pathutils::join(source_dir, Path::new(&import))
                    .unwrap_or_else(|| source_dir.join(&import))
            };
            let bytes = std::fs::read(&resolved).map_err(|error| {
                format!("Cannot read font {}: {error}", resolved.display())
            })?;
            let virtual_path = virtual_font_path(virtual_slint, &import);
            assets.insert(virtual_path, STANDARD.encode(bytes));
        }
    }
    Ok(assets)
}

fn encode_json(value: &serde_json::Value) -> Result<String, String> {
    let json = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&json).map_err(|error| error.to_string())?;
    let compressed = encoder.finish().map_err(|error| error.to_string())?;
    Ok(STANDARD.encode(compressed))
}
