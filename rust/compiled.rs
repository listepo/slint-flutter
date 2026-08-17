//! Build-time compilation units for generated Dart wrappers.
//!
//! Generated `.slint.dart` files must not read `.slint` from disk. This module
//! packs every file the compiler needed — rewritten so imports and `@image-url`
//! resolve without a filesystem — into a gzip+base64 blob. `load()` passes that
//! blob to [`instantiate`], which compiles it through the interpreter once per
//! process and then creates instances.

use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use i_slint_compiler::object_tree::Document;
use i_slint_compiler::pathutils;
use i_slint_compiler::typeloader::TypeLoader;
use slint_interpreter::{CompilationResult, Compiler, ComponentDefinition, ComponentInstance};

const MODULE_VERSION: u32 = 1;
const VIRTUAL_ROOT: &str = "/slint-aot";

thread_local! {
    static MODULES: std::cell::RefCell<HashMap<u64, CompilationResult>> =
        std::cell::RefCell::new(HashMap::new());
}

struct BundledFile {
    path: PathBuf,
    source: String,
    imports: Vec<(String, PathBuf)>,
}

/// Pack the compiled document graph into the blob generated wrappers embed.
pub(crate) fn bundle(
    main: &Document,
    loader: &TypeLoader,
    style: Option<&str>,
) -> Result<String, String> {
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

    let json = serde_json::json!({
        "v": MODULE_VERSION,
        "style": style,
        "main": main,
        "files": bundled,
    });
    encode_json(&json)
}

/// Compile [blob] if needed and instantiate [component], or the last exported
/// component when [component] is empty.
pub(crate) fn instantiate(
    blob: &str,
    component: Option<&str>,
) -> Result<ComponentInstance, String> {
    let definition = definition(blob, component)?;
    definition.create().map_err(|error| error.to_string())
}

fn definition(blob: &str, component: Option<&str>) -> Result<ComponentDefinition, String> {
    let hash = blob_hash(blob);
    MODULES.with(|modules| {
        let mut modules = modules.borrow_mut();
        if !modules.contains_key(&hash) {
            modules.insert(hash, compile_blob(blob)?);
        }
        pick_component(&modules[&hash], component)
    })
}

fn compile_blob(blob: &str) -> Result<CompilationResult, String> {
    let module = decode_module(blob)?;
    let version = module.get("v").and_then(|value| value.as_u64()).unwrap_or(0);
    if version != MODULE_VERSION as u64 {
        return Err(format!("unsupported compiled Slint module version {version}"));
    }
    let main = module
        .get("main")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "compiled module is missing main".to_string())?
        .to_string();
    let files = module
        .get("files")
        .and_then(|value| value.as_object())
        .ok_or_else(|| "compiled module is missing files".to_string())?
        .iter()
        .map(|(path, source)| {
            source
                .as_str()
                .map(|source| (normalize_key(path), source.to_string()))
                .ok_or_else(|| format!("compiled module file {path} is not a string"))
        })
        .collect::<Result<HashMap<_, _>, _>>()?;
    let main_source = files
        .get(&normalize_key(&main))
        .cloned()
        .ok_or_else(|| "compiled module is missing its main file".to_string())?;
    let files = Rc::new(files);

    let mut compiler = Compiler::default();
    if let Some(style) =
        module.get("style").and_then(|value| value.as_str()).filter(|s| !s.is_empty())
    {
        compiler.set_style(style.to_string());
    }
    let loaded = files.clone();
    compiler.set_file_loader(move |path| {
        let requested = normalize_key(&path.to_string_lossy());
        let loaded = loaded.clone();
        Box::pin(async move { lookup_file(&loaded, &requested).map(Ok) })
    });

    let result = spin_on::spin_on(compiler.build_from_source(main_source, PathBuf::from(main)));
    if result.has_errors() {
        let messages = result
            .diagnostics()
            .map(|diagnostic| diagnostic.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        return Err(messages);
    }
    Ok(result)
}

fn pick_component(
    result: &CompilationResult,
    component: Option<&str>,
) -> Result<ComponentDefinition, String> {
    match component.filter(|name| !name.is_empty()) {
        Some(name) => result.component(name).ok_or_else(|| {
            let names = result.component_names().collect::<Vec<_>>().join(", ");
            format!("no component named {name:?}; the module exports [{names}]")
        }),
        None => result
            .components()
            .last()
            .ok_or_else(|| "the compiled module exports no instantiable component".into()),
    }
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

fn encode_json(value: &serde_json::Value) -> Result<String, String> {
    let json = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&json).map_err(|error| error.to_string())?;
    let compressed = encoder.finish().map_err(|error| error.to_string())?;
    Ok(STANDARD.encode(compressed))
}

pub(crate) fn decode_module(blob: &str) -> Result<serde_json::Value, String> {
    let compressed = STANDARD.decode(blob.trim()).map_err(|error| error.to_string())?;
    let json = flate2::read::GzDecoder::new(compressed.as_slice());
    serde_json::from_reader(json).map_err(|error| error.to_string())
}

fn blob_hash(blob: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    blob.hash(&mut hasher);
    hasher.finish()
}

fn normalize_key(path: &str) -> String {
    path.replace('\\', "/")
}

fn lookup_file<'a>(files: &'a HashMap<String, String>, requested: &str) -> Option<String> {
    if let Some(source) = files.get(requested) {
        return Some(source.clone());
    }
    files.iter().find_map(|(path, source)| {
        (requested.ends_with(path) || path.ends_with(requested)).then(|| source.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_files(main: &str, files: BTreeMap<&str, &str>) -> String {
        encode_json(&serde_json::json!({
            "v": MODULE_VERSION,
            "main": main,
            "files": files,
        }))
        .unwrap()
    }

    #[test]
    fn a_bundled_import_compiles_without_touching_the_filesystem() {
        let blob = encode_files(
            "/slint-aot/app.slint",
            BTreeMap::from([
                (
                    "/slint-aot/app.slint",
                    r#"import { Shared } from "/slint-aot/shared.slint";
                    export component App inherits Shared { }"#,
                ),
                (
                    "/slint-aot/shared.slint",
                    "export component Shared { in-out property <int> n: 3; }",
                ),
            ]),
        );
        let result = compile_blob(&blob).unwrap();
        assert!(result.component("App").is_some());
    }

    #[test]
    fn a_relative_import_resolves_under_the_virtual_root() {
        let blob = encode_files(
            "/slint-aot/app.slint",
            BTreeMap::from([
                (
                    "/slint-aot/app.slint",
                    r#"import { Shared } from "shared.slint";
                    export component App inherits Shared { }"#,
                ),
                (
                    "/slint-aot/shared.slint",
                    "export component Shared { in-out property <int> n: 4; }",
                ),
            ]),
        );
        let result = compile_blob(&blob).unwrap();
        assert!(result.component("App").is_some());
    }
}
