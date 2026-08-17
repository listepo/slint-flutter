//! `slint-dart-generate`: the binary `package:slint_generator` runs.
//!
//! Prints the generated Dart in a JSON envelope, and with `--write` also puts
//! it at the output path. `build_runner` owns its outputs, so the builder takes
//! the source from stdout and leaves the file alone; `--write` is for running
//! the tool by hand. Either way one input yields one Dart file and nothing else.
//!
//! The output path still has to be the real one even without `--write`: the
//! wrapper resolves the `.slint` path relative to it.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    let write = args.iter().position(|arg| arg == "--write").map(|i| args.remove(i)).is_some();
    if args.len() < 2 || args.len() > 3 {
        eprintln!(
            "usage: slint-dart-generate <input.slint> <output.slint.dart> [options.json] [--write]"
        );
        return ExitCode::from(2);
    }
    let input = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[1]);
    let options_json = args.get(2).map(String::as_str).unwrap_or("{}");

    let options = match slint_dart_codegen::parse_options(options_json) {
        Ok(options) => options,
        Err(error) => {
            // Still a JSON envelope: the builder reports the error with the
            // diagnostics it already knows how to render.
            println!(
                "{}",
                serde_json::json!({
                    "source": null,
                    "error": error,
                    "dependencies": [],
                    "diagnostics": [],
                })
            );
            return ExitCode::from(1);
        }
    };

    let generation = slint_dart_codegen::generate(&input, &output, options);
    if write && let Some(dart) = generation.dart.as_deref() {
        if let Some(parent) = output.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(error) = fs::write(&output, dart) {
            eprintln!("failed to write {}: {error}", output.display());
            return ExitCode::from(1);
        }
    }
    println!("{}", slint_dart_codegen::generation_json(&generation));
    if generation.error.is_some() { ExitCode::from(1) } else { ExitCode::SUCCESS }
}
