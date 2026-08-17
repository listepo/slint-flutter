"""A Bazel rule that turns `.slint` files into typed Dart wrappers.

The work is done by `slint-dart-generate`, wrapped in a Dart persistent worker
(`slint_generator/bin/codegen_worker.dart`). Bazel keeps one worker alive for
the whole build, so a package with many `.slint` files pays for one process
rather than one per file, and the files still generate in parallel across
workers.
"""

def _codegen_worker_impl(ctx):
    """Launcher Bazel calls as the persistent SlintCodegen worker."""
    launcher = ctx.actions.declare_file(ctx.label.name)
    worker = None
    for src in ctx.files._worker_srcs:
        if src.basename == "codegen_worker.dart":
            worker = src
            break
    if worker == None:
        fail("codegen_worker.dart not found in //slint_generator:worker_srcs")

    ctx.actions.write(
        output = launcher,
        is_executable = True,
        content = """#!/usr/bin/env bash
set -euo pipefail
if ! command -v dart >/dev/null 2>&1; then
  echo 'dart not found on PATH; run bazel with --action_env=PATH="$PATH".' >&2
  exit 1
fi
exec dart run "{worker}" "$@"
""".format(worker = worker.path),
    )

    return [DefaultInfo(
        executable = launcher,
        runfiles = ctx.runfiles(files = ctx.files._worker_srcs),
    )]

codegen_worker = rule(
    implementation = _codegen_worker_impl,
    doc = "Runs the Slint codegen worker with its Dart package resolved.",
    executable = True,
    attrs = {
        "_worker_srcs": attr.label(
            default = "//slint_generator:worker_srcs",
        ),
    },
)

def _slint_dart_library_impl(ctx):
    outputs = []
    options = json.encode({
        "style": ctx.attr.style,
        "include_paths": [path.dirname for path in ctx.files.include_paths],
    }) if (ctx.attr.style or ctx.files.include_paths) else "{}"

    for source in ctx.files.srcs:
        generated = ctx.actions.declare_file(
            source.basename.removesuffix(".slint") + ".slint.dart",
            sibling = source,
        )
        outputs.append(generated)

        # The worker protocol requires exactly one `@flagfile`: Bazel reads it
        # and sends the lines as the request's arguments. A plain argument list
        # makes Bazel refuse to use the worker at all.
        args = ctx.actions.args()
        args.add(source.path)
        args.add(generated.path)
        args.add(options)
        args.use_param_file("@%s", use_always = True)
        args.set_param_file_format("multiline")

        ctx.actions.run(
            outputs = [generated],
            inputs = ctx.files.srcs + ctx.files.include_paths,
            executable = ctx.executable._worker,
            arguments = [args],
            mnemonic = "SlintCodegen",
            progress_message = "Generating Dart for %s" % source.short_path,
            execution_requirements = {
                "supports-workers": "1",
                "requires-worker-protocol": "proto",
            },
            env = {
                "SLINT_DART_GENERATE": ctx.executable._generator.path,
            },
            tools = [ctx.executable._generator],
            use_default_shell_env = True,
        )

    return [DefaultInfo(files = depset(outputs))]

slint_dart_library = rule(
    implementation = _slint_dart_library_impl,
    doc = "Generates one `.slint.dart` per `.slint` source.",
    attrs = {
        "srcs": attr.label_list(
            allow_files = [".slint"],
            mandatory = True,
            doc = "The `.slint` files to generate wrappers for.",
        ),
        "include_paths": attr.label_list(
            allow_files = [".slint"],
            doc = "Extra files importable from `srcs`.",
        ),
        "style": attr.string(doc = "The widget style, e.g. `material`."),
        "_worker": attr.label(
            default = "//tools:codegen_worker",
            executable = True,
            cfg = "exec",
        ),
        "_generator": attr.label(
            default = "//native:slint_dart_generate",
            executable = True,
            cfg = "exec",
        ),
    },
)
