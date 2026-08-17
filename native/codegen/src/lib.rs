//! Generate-time compiler: `.slint` in, Dart out.
//!
//! This crate holds the Slint compiler so the runtime library doesn't have to
//! expose it, and it emits exactly one artifact per input: a `.slint.dart`
//! wrapper carrying the typed API and the compiled module. Nothing it produces
//! needs a Rust, C or C++ toolchain — an application only ever compiles Dart.
//!
//! `package:slint_generator` drives it through the `slint-dart-generate`
//! binary; the runtime instantiates the module with `slint_dart_load_compiled`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::generator::OutputFormat;

pub mod bundle;
pub mod dart;

/// What one `.slint` file generated: Dart source, or an error, plus everything
/// `build_runner` needs to decide when to run again.
pub struct Generation {
    pub dart: Option<String>,
    pub error: Option<String>,
    pub dependencies: BTreeSet<String>,
    pub diagnostics: Vec<serde_json::Value>,
}

#[derive(Debug, Default)]
pub struct Options {
    pub include_paths: Vec<PathBuf>,
    pub style: Option<String>,
}

pub fn parse_options(json: &str) -> Result<Options, String> {
    if json.is_empty() {
        return Ok(Options::default());
    }
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|error| format!("Invalid Dart generation options: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "Dart generation options must be a JSON object".to_string())?;
    for name in object.keys() {
        if name != "include_paths" && name != "style" {
            return Err(format!("Unknown Dart generation option {name:?}"));
        }
    }
    let style = match object.get("style") {
        None => None,
        Some(serde_json::Value::String(style)) => Some(style.clone()),
        Some(_) => return Err("Dart generation option \"style\" must be a string".into()),
    };
    let include_paths = match object.get("include_paths") {
        None => Vec::new(),
        Some(serde_json::Value::Array(paths)) => paths
            .iter()
            .map(|path| {
                path.as_str().map(PathBuf::from).ok_or_else(|| {
                    "Dart generation option \"include_paths\" must be a list of strings".to_string()
                })
            })
            .collect::<Result<_, _>>()?,
        Some(_) => {
            return Err("Dart generation option \"include_paths\" must be a list of strings".into());
        }
    };
    Ok(Options { include_paths, style })
}

/// The JSON envelope `package:slint_generator` reads from the binary's stdout.
pub fn generation_json(generation: &Generation) -> serde_json::Value {
    serde_json::json!({
        "source": generation.dart,
        "error": generation.error,
        "dependencies": generation.dependencies,
        "diagnostics": generation.diagnostics,
    })
}

/// Compile `input_path` and generate the Dart wrapper that `output_path` will
/// hold. A compile error is a value, not a failure: the caller still needs the
/// dependency list so watch mode can recover when an imported file is fixed.
pub fn generate(input_path: &Path, output_path: &Path, options: Options) -> Generation {
    match generate_inner(input_path, output_path, options) {
        Ok(generation) => generation,
        Err(error) => Generation {
            dart: None,
            error: Some(error),
            dependencies: BTreeSet::new(),
            diagnostics: Vec::new(),
        },
    }
}

fn generate_inner(
    input_path: &Path,
    output_path: &Path,
    options: Options,
) -> Result<Generation, String> {
    let input_path = std::path::absolute(input_path).map_err(|error| error.to_string())?;
    let output_path = std::path::absolute(output_path).map_err(|error| error.to_string())?;

    let mut diagnostics = BuildDiagnostics::default();
    let syntax_node = i_slint_compiler::parser::parse_file(&input_path, &mut diagnostics);
    let no_dependencies = || dependencies_of(input_path.clone(), Vec::new());
    if diagnostics.has_errors() {
        return Ok(failed(None, diagnostics, no_dependencies()));
    }
    let Some(syntax_node) = syntax_node else {
        return Ok(failed(
            Some("The Slint parser produced no document".into()),
            diagnostics,
            no_dependencies(),
        ));
    };

    // `Llr` is the output format to configure with: this crate's generator
    // reads the LLR, and like the C++ and Python generators it wants only
    // builtin resources embedded and inlining left alone. `Interpreter` would
    // force decisions meant for the runtime that compiles the module again.
    let mut compiler_config = i_slint_compiler::CompilerConfiguration::new(OutputFormat::Llr);
    compiler_config.include_paths = options.include_paths;
    compiler_config.style = options.style;
    let (document, diagnostics, loader) = spin_on::spin_on(i_slint_compiler::compile_syntax_node(
        syntax_node,
        diagnostics,
        compiler_config,
    ));
    let dependencies = dependencies_of(input_path, loader.all_files_to_watch());
    if diagnostics.has_errors() {
        return Ok(failed(None, diagnostics, dependencies));
    }

    let module = match bundle::bundle(&document, &loader, loader.compiler_config.style.as_deref()) {
        Ok(module) => module,
        Err(error) => return Ok(failed(Some(error), diagnostics, dependencies)),
    };
    let dart = match dart::generate(&document, &loader.compiler_config, Some(&output_path), &module)
    {
        Ok(dart) => dart,
        Err(error) => return Ok(failed(Some(error.to_string()), diagnostics, dependencies)),
    };

    Ok(Generation {
        dart: Some(dart),
        error: None,
        dependencies,
        diagnostics: diagnostics_json(&diagnostics),
    })
}

fn failed(
    extra: Option<String>,
    diagnostics: BuildDiagnostics,
    dependencies: BTreeSet<String>,
) -> Generation {
    let mut error = diagnostics.to_string_vec().join("\n");
    if let Some(extra) = extra {
        error = if error.is_empty() { extra } else { format!("{error}\n{extra}") };
    }
    Generation {
        dart: None,
        error: (!error.is_empty()).then_some(error),
        dependencies,
        diagnostics: diagnostics_json(&diagnostics),
    }
}

fn dependencies_of(
    input_path: PathBuf,
    dependencies: impl IntoIterator<Item = PathBuf>,
) -> BTreeSet<String> {
    dependencies
        .into_iter()
        .chain(std::iter::once(input_path))
        .filter(|path| !path.to_string_lossy().starts_with("builtin:"))
        .map(|path| std::path::absolute(&path).unwrap_or(path).to_string_lossy().into_owned())
        .collect()
}

fn diagnostics_json(diagnostics: &BuildDiagnostics) -> Vec<serde_json::Value> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            let (line, column) = diagnostic.line_column();
            serde_json::json!({
                "level": match diagnostic.level() {
                    i_slint_compiler::diagnostics::DiagnosticLevel::Error => "error",
                    _ => "warning",
                },
                "message": diagnostic.message(),
                "file": diagnostic.source_file().map(|path| path.display().to_string()),
                "line": line,
                "column": column,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch directory of its own, so the tests can run in parallel and
    /// still delete what they wrote.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let directory = std::env::temp_dir().join(format!(
                "slint-dart-codegen-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            std::fs::create_dir_all(&directory).unwrap();
            Self(directory)
        }

        fn write(&self, name: &str, contents: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, contents).unwrap();
            path
        }

        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn has_file(dependencies: &BTreeSet<String>, name: &str) -> bool {
        dependencies
            .iter()
            .any(|path| Path::new(path).file_name().is_some_and(|found| found == name))
    }

    /// The blob a generated wrapper carries, as the runtime's
    /// `slint_dart_load_compiled` receives it.
    pub(crate) fn compiled_blob(source: &str) -> &str {
        const MARKER: &str = "instantiateCompiled(";
        let start = source.find(MARKER).expect("generated source instantiates a compiled module");
        let after = &source[start + MARKER.len()..];
        let quote = after.find('"').expect("compiled module string");
        let body = &after[quote + 1..];
        &body[..body.find('"').expect("compiled module string terminator")]
    }

    #[test]
    fn dart_is_the_only_thing_generated() {
        let scratch = Scratch::new("dart-only");
        let input = scratch.write("app.slint", "export component App { }");
        let generation = generate(&input, &scratch.path("app.slint.dart"), Options::default());

        assert!(generation.error.is_none(), "{:?}", generation.error);
        let dart = generation.dart.expect("generated Dart");
        // The wrapper instantiates the embedded module and never reads .slint.
        assert!(dart.contains("slint.instantiateCompiled("), "{dart}");
        assert!(!dart.contains("slint.loadFile("), "{dart}");
        assert!(!dart.contains("loadSource("), "{dart}");
        // Nothing here asks for a second toolchain: no Rust, no C, no C++.
        for foreign in ["extern \"C\"", "no_mangle", "#[repr", "#include", "pub fn "] {
            assert!(!dart.contains(foreign), "generated Dart contains {foreign:?}:\n{dart}");
        }
        // And the generator writes nothing beside the file it was asked for.
        let stray = std::fs::read_dir(&scratch.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name != "app.slint")
            .collect::<Vec<_>>();
        assert!(stray.is_empty(), "generator left {stray:?} behind");
    }

    #[test]
    fn dart_names_are_camel_case_and_imports_are_reported() {
        let scratch = Scratch::new("camel");
        scratch.write("shared.slint", "export component Shared { }");
        let input = scratch.write(
            "app.slint",
            r#"
                import { Shared } from "shared.slint";
                export component MainWindow inherits Shared {
                    in-out property <int> todo-model;
                    in-out property <image> icon;
                    callback todo_added(string);
                    public function do_work(value: int) -> int { value }
                }
            "#,
        );
        let generation = generate(&input, &scratch.path("app.slint.dart"), Options::default());

        let source = generation.dart.as_deref().expect("generated Dart");
        assert!(source.contains("int get todoModel"), "{source}");
        assert!(source.contains("slint.SlintImage get icon"), "{source}");
        assert!(source.contains("slint.SlintImage.fromSlint"), "{source}");
        assert!(source.contains("void onTodoAdded"), "{source}");
        assert!(source.contains("int invokeDoWork"), "{source}");
        assert!(source.contains("getProperty(\"todo-model\")"), "{source}");
        assert!(source.contains("factory MainWindow.load("), "{source}");
        assert!(has_file(&generation.dependencies, "app.slint"));
        assert!(has_file(&generation.dependencies, "shared.slint"));
    }

    #[test]
    fn camel_case_collisions_are_rejected() {
        let scratch = Scratch::new("collision");
        let input = scratch.write(
            "app.slint",
            r#"
                export component App {
                    in-out property <int> foo-bar;
                    in-out property <int> fooBar;
                }
            "#,
        );
        let generation = generate(&input, &scratch.path("app.slint.dart"), Options::default());

        assert!(generation.dart.is_none());
        let message = generation.error.expect("a collision error");
        assert!(message.contains("foo-bar"), "{message}");
        assert!(message.contains("fooBar"), "{message}");
        assert!(message.contains("both generate"), "{message}");
    }

    #[test]
    fn an_import_error_still_reports_the_files_to_watch() {
        let scratch = Scratch::new("import-error");
        scratch.write("shared.slint", "export component Shared { this is not slint }");
        let input = scratch.write(
            "app.slint",
            r#"
                import { Shared } from "shared.slint";
                export component App inherits Shared { }
            "#,
        );
        let generation = generate(&input, &scratch.path("app.slint.dart"), Options::default());

        assert!(generation.dart.is_none());
        assert!(generation.error.is_some_and(|message| !message.is_empty()));
        // Watch mode recovers only if it knows to re-run when shared.slint changes.
        assert!(has_file(&generation.dependencies, "app.slint"));
        assert!(has_file(&generation.dependencies, "shared.slint"));
    }

    #[test]
    fn include_paths_and_style_are_baked_into_the_module() {
        let scratch = Scratch::new("options");
        scratch.write("includes/shared.slint", "export component Shared { }");
        let input = scratch.write(
            "app.slint",
            r#"
                import { Shared } from "shared.slint";
                export component App inherits Shared { }
            "#,
        );
        let options = parse_options(
            &serde_json::json!({
                "include_paths": [scratch.path("includes").to_string_lossy()],
                "style": "material",
            })
            .to_string(),
        )
        .unwrap();
        let generation = generate(&input, &scratch.path("app.slint.dart"), options);

        assert!(generation.error.is_none(), "{:?}", generation.error);
        let source = generation.dart.as_deref().unwrap();
        assert!(source.contains("factory App.load()"), "{source}");
        // The developer's include path must not leak into generated source.
        assert!(!source.contains("includePaths"), "{source}");
        assert!(!source.contains(scratch.path("includes").to_string_lossy().as_ref()), "{source}");

        let module = decode_for_test(compiled_blob(source));
        assert_eq!(module["style"], "material");
        assert!(
            module["files"].as_object().unwrap().keys().any(|path| path.ends_with("shared.slint")),
            "{module}"
        );
        assert!(has_file(&generation.dependencies, "shared.slint"));
    }

    #[test]
    fn invalid_options_are_rejected() {
        let error = parse_options(r#"{"include_paths":"not-a-list"}"#).unwrap_err();
        assert!(error.contains("include_paths"), "{error}");
        assert!(error.contains("list of strings"), "{error}");
        assert!(parse_options(r#"{"nope":1}"#).unwrap_err().contains("nope"));
        assert!(parse_options("").is_ok());
    }

    /// Mirror of the runtime's decoder, so a change to the blob layout that
    /// only one side knows about fails here.
    fn decode_for_test(blob: &str) -> serde_json::Value {
        use base64::Engine;
        let compressed = base64::engine::general_purpose::STANDARD.decode(blob.trim()).unwrap();
        serde_json::from_reader(flate2::read::GzDecoder::new(compressed.as_slice())).unwrap()
    }
}
