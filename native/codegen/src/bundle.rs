//! Pack the compiled `.slint` graph into the blob generated wrappers embed.
//!
//! A generated `.slint.dart` must not read `.slint` from disk: the file may not
//! ship with the application at all. So every file the compiler touched goes
//! into the wrapper — imports rewritten to a virtual root, relative `@image-url`
//! and font references bundled once as assets — as one gzip+base64 string. The
//! runtime hands that string back to the interpreter, which compiles it without
//! a filesystem.
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
        // Absolute image paths cannot ship with the application, so they are
        // inlined as data URIs. Relative `@image-url`s are left in place and
        // bundled once as assets below: the runtime materializes them next to
        // the sources, and the interpreter resolves the reference from there —
        // each image once, however many times it is referenced.
        source = inline_absolute_image_urls(&source);
        bundled.insert(virtual_url, source);
    }

    let main_path = document_path(main)
        .ok_or_else(|| "Cannot determine the path of the main Slint file".to_string())?;
    let main = mapping
        .get(&main_path)
        .cloned()
        .ok_or_else(|| "The main Slint file was not bundled".to_string())?;

    let mut assets = collect_font_assets(&files, &mapping)?;
    collect_image_assets(&files, &mapping, &mut assets)?;

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

/// One `@image-url("…")` reference, with the range of the quoted path inside
/// the source it was scanned from.
struct ImageUrl {
    start: usize,
    end: usize,
    path: String,
}

/// Scan for every `@image-url("…")` reference in [source], skipping `data:` and
/// `builtin:` URIs that need no embedding. The ranges index [source] itself.
fn find_image_urls(source: &str) -> Vec<ImageUrl> {
    let mut urls = Vec::new();
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
            // The scanner works in byte offsets on a UTF-8 string; this guards
            // the arithmetic (`content_start + end` is measured from a slice,
            // so a mistake here would slice a wrong, possibly non-boundary
            // range). Only checked in debug builds.
            debug_assert_eq!(
                &source[content_start..content_start + end],
                path,
                "@image-url range does not round-trip"
            );
            urls.push(ImageUrl {
                start: content_start,
                end: content_start + end,
                path: path.to_string(),
            });
        }
        search_from = content_start + end + quote.len_utf8();
    }
    urls
}

/// An absolute `@image-url` path cannot ship with the application, so inline it
/// as a data URI (the previous behaviour). Relative references are left alone:
/// they are bundled once as assets and resolved by the interpreter against the
/// materialized tree at load time.
fn inline_absolute_image_urls(source: &str) -> String {
    let mut result = source.to_string();
    for url in find_image_urls(&result).into_iter().rev() {
        if !Path::new(&url.path).is_absolute() {
            continue;
        }
        let Ok(bytes) = std::fs::read(&url.path) else {
            continue;
        };
        // Ranges were captured from `result`; replacements run in reverse so
        // earlier indices stay valid. This catches any drift if that ever stops
        // being true (e.g. a replacement that shifts the bytes under a pending
        // range).
        debug_assert!(url.start < url.end && url.end <= result.len());
        debug_assert_eq!(&result[url.start..url.end], url.path);
        result.replace_range(url.start..url.end, &data_uri(Path::new(&url.path), &bytes));
    }
    result
}

/// Bundle every relative `@image-url` once, keyed by its virtual path so the
/// runtime materializes a single copy however many times the image is used.
fn collect_image_assets(
    files: &[BundledFile],
    mapping: &HashMap<PathBuf, String>,
    assets: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    for file in files {
        let Some(virtual_slint) = mapping.get(&file.path) else {
            continue;
        };
        let source_dir = file.path.parent().unwrap_or(&file.path);
        for url in find_image_urls(&file.source) {
            let path = &url.path;
            if path.starts_with("data:")
                || path.starts_with("builtin:")
                || Path::new(path).is_absolute()
            {
                continue;
            }
            let resolved = pathutils::join(source_dir, Path::new(path))
                .unwrap_or_else(|| source_dir.join(path));
            let virtual_path = virtual_resource_path(virtual_slint, path);
            // The runtime materializes assets under the virtual root and the
            // interpreter resolves the reference from the same tree; a key
            // outside it (e.g. an absolute path leaking through) would put the
            // file where nothing looks for it.
            debug_assert!(
                virtual_path.starts_with(VIRTUAL_ROOT),
                "image {path} escaped the virtual root: {virtual_path}"
            );
            // A missing image is left as a plain reference, as before: it may
            // be decorative, and the old data-URI pass silently skipped it too.
            let Ok(bytes) = std::fs::read(&resolved) else {
                continue;
            };
            let encoded = STANDARD.encode(bytes);
            if let Some(existing) = assets.get(&virtual_path) {
                // The same key means the same file: repeated references dedup
                // to identical bytes. Two different files sharing a key (say an
                // image colliding with a font of the same name) is a bug that
                // would silently drop one of them.
                debug_assert_eq!(
                    existing, &encoded,
                    "two different resources resolve to {virtual_path}"
                );
                continue;
            }
            assets.insert(virtual_path, encoded);
        }
    }
    Ok(())
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

/// The virtual path a resource referenced from the virtual `.slint` file
/// [virtual_slint] resolves to — the location the runtime materializes it at.
fn virtual_resource_path(virtual_slint: &str, resource: &str) -> String {
    let slint_path = Path::new(virtual_slint);
    let parent = slint_path.parent().unwrap_or(Path::new(VIRTUAL_ROOT));
    pathutils::join(parent, Path::new(resource))
        .unwrap_or_else(|| parent.join(resource))
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
            let bytes = std::fs::read(&resolved)
                .map_err(|error| format!("Cannot read font {}: {error}", resolved.display()))?;
            let virtual_path = virtual_resource_path(virtual_slint, &import);
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
