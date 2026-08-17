/// A Bazel persistent worker around `slint-dart-generate`.
///
/// Bazel starts this once and feeds it one `WorkRequest` per `.slint` file, so
/// a build of many files pays for one process instead of one per file. Without
/// `--persistent_worker` it runs a single request from the command line, which
/// is what `bazel build --strategy=SlintCodegen=local` and manual runs use.
///
/// Each request is `<input.slint> <output.slint.dart> [options-json]`, the same
/// argument order the generator binary itself takes.
library;

import 'dart:convert';
import 'dart:io';

import 'package:bazel_worker/bazel_worker.dart';

Future<void> main(List<String> arguments) async {
  if (arguments.contains('--persistent_worker')) {
    await _SlintCodegenWorker().run();
    return;
  }
  // One-shot mode still gets Bazel's `@flagfile`, because the rule always
  // writes one so that worker mode is available.
  final result = await _generate(_expandFlagfiles(arguments));
  stderr.write(result.output);
  exit(result.exitCode);
}

List<String> _expandFlagfiles(List<String> arguments) => [
      for (final argument in arguments)
        if (argument.startsWith('@'))
          ...File(argument.substring(1))
              .readAsLinesSync()
              .where((line) => line.isNotEmpty)
        else
          argument,
    ];

class _SlintCodegenWorker extends AsyncWorkerLoop {
  @override
  Future<WorkResponse> performRequest(WorkRequest request) async {
    final result = await _generate(request.arguments);
    return WorkResponse(exitCode: result.exitCode, output: result.output);
  }
}

class _Result {
  _Result(this.exitCode, this.output);
  final int exitCode;
  final String output;
}

/// The generator binary. Bazel passes it in `SLINT_DART_GENERATE`, which the
/// rule wires to the `//native:slint_dart_generate` output so the worker never
/// searches for a toolchain of its own.
String get _generator =>
    Platform.environment['SLINT_DART_GENERATE'] ?? 'slint-dart-generate';

Future<_Result> _generate(List<String> arguments) async {
  if (arguments.length < 2) {
    return _Result(
      2,
      'usage: codegen_worker <input.slint> <output.slint.dart> [options.json]\n',
    );
  }
  final input = arguments[0];
  final output = arguments[1];
  final options = arguments.length > 2 ? arguments[2] : '{}';

  final ProcessResult run;
  try {
    run = await Process.run(_generator, [input, output, options, '--write']);
  } on ProcessException catch (error) {
    return _Result(1, 'cannot run $_generator: ${error.message}\n');
  }

  final stdoutText = (run.stdout as String).trim();
  if (run.exitCode != 0 || stdoutText.isEmpty) {
    return _Result(
      run.exitCode == 0 ? 1 : run.exitCode,
      '${run.stderr}\n$stdoutText\n',
    );
  }

  // The envelope carries diagnostics even on success; surface warnings so they
  // reach the Bazel log rather than vanishing.
  final envelope = jsonDecode(stdoutText) as Map<String, dynamic>;
  final buffer = StringBuffer();
  for (final diagnostic in (envelope['diagnostics'] as List<Object?>? ?? [])) {
    final map = diagnostic! as Map<String, dynamic>;
    buffer.writeln('${map['level']}: ${map['file']}:${map['line']}:'
        '${map['column']}: ${map['message']}');
  }
  final error = envelope['error'];
  if (error != null) {
    buffer.writeln(error as String);
    return _Result(1, buffer.toString());
  }
  return _Result(0, buffer.toString());
}
