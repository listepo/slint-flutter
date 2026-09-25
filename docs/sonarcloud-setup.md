# SonarCloud OSS setup (slint-flutter)

Maintainer guide for the SonarCloud workflow in `.github/workflows/sonarcloud.yml` and the scanner
configuration in `sonar-project.properties`.

The `sonar.organization` / `sonar.projectKey` values (`listepo` / `listepo_slint-flutter`) are
**placeholders** until they match the SonarCloud UI after you import the project.
As of 2026-09-25 the public SonarCloud API reports no organization with the key
`listepo`, so the organization still has to be created (bound to the `listepo`
GitHub account) on first import.

## 1. Import the repository

1. Sign in at [sonarcloud.io](https://sonarcloud.io) with GitHub.
2. Create or pick the organization bound to the **listepo** GitHub account and choose
   the free plan for open-source (public) projects.
3. Import the **listepo/slint-flutter** repository.
4. Compare the **organization key** and **project key** shown in the UI with
   `sonar-project.properties`. If they differ, update the file so they match exactly;
   a mismatch makes the scan fail or report into the wrong project.

## 2. Turn Automatic Analysis off

This project is analyzed by CI (`SonarSource/sonarqube-scan-action`), so SonarCloud's
**Automatic Analysis** must be **off**. With both enabled, the CI scan fails with an
error about Automatic Analysis being enabled.

In the SonarCloud project: **Administration → Analysis Method → Automatic Analysis → off**.

## 3. Create a token

1. Open <https://sonarcloud.io/account/security>
   (**My Account → Security**, section **Access Tokens / Personal Tokens**).
2. Generate a token (for example `slint-flutter-github-actions`).
3. Copy the value; it is shown only once.

## 4. Add the `SONAR_TOKEN` secret

On **listepo/slint-flutter**: **Settings → Secrets and variables → Actions → New repository secret**

- Name: `SONAR_TOKEN`
- Value: the token from step 3

Never commit the token. Without the secret (fork pull requests, or before it is added)
every step in the workflow soft-skips with a notice, so the check stays green; a green
run in that state does **not** mean an analysis happened.

## 5. When the analysis runs

- On pull requests targeting `main` (draft pull requests are skipped until they are
  marked ready for review). Pull request decoration needs the SonarCloud GitHub App,
  which the import in step 1 installs.
- On every push to `main`.
- Manually via **Actions → sonarcloud → Run workflow**.

## 6. Soft-fail for now, blocking later

The workflow does not fail the build yet: the coverage and scan steps use
`continue-on-error: true`, and the Quality Gate is not awaited. To make it blocking
once the dashboard looks sane:

1. Remove `continue-on-error: true` from the scan (and, if wanted, coverage) steps.
2. Add `sonar.qualitygate.wait=true` to `sonar-project.properties` so the scan step
   fails when the Quality Gate fails.
3. Optionally mark the `sonarcloud` check as required in the branch protection rules
   for `main`.

## 7. Coverage

Coverage is **not wired yet** for Dart. SonarCloud analyzes Dart natively, so the
scan reports bugs, code smells and duplication, but the dashboard shows no coverage.

Follow-up: run the tests with coverage (`dart test --coverage=coverage` or
`flutter test --coverage`), convert to LCOV with `package:coverage`'s `format_coverage`
if needed, and point Sonar at the file with `sonar.dart.lcov.reportPaths`.

Despite GitHub's language bar (Rust and Dart are about even), this repository is a
Flutter integration: Dart packages (`slint/`, `slint_flutter/`, `slint_generator/`) on top
of a Rust native library in `native/`. Both are analyzed; neither reports coverage yet.
Rust coverage (`cargo llvm-cov` in `native/`) is a follow-up: the crate builds the Slint
interpreter with the winit backend and the Skia/FemtoVG renderers, which needs extra
system packages on the runner, and `native/rust-toolchain.toml` lists a dozen
cross-compilation targets that rustup would download first. Report it through
`sonar.rust.lcov.reportPaths=coverage/lcov.info` once wired.

Scope: the three Dart packages' `lib/` (and `bin/`), their `test/` directories as tests,
and the Rust sources in `native/`. `examples/`, `tools/` (Bazel/Starlark), generated
bindings and build output are outside the analysis scope.

Note: this repository had no GitHub Actions workflows before this one.

### C, C++ and Objective-C

SonarCloud's C-family analyzer needs a compilation database (`compile_commands.json`)
or build-wrapper output. This repository does not produce one in CI yet, so C, C++ and
Objective-C analysis is turned off in `sonar-project.properties` via
`sonar.c.file.suffixes=-`, `sonar.cpp.file.suffixes=-` and `sonar.objc.file.suffixes=-`.
Follow-up: generate `compile_commands.json` for any first-party native code that should be analyzed, set
`sonar.cfamily.compile-commands`, and remove those three lines.

## 8. Local dry run (optional)

Run the scanner locally only with `SONAR_TOKEN` exported in your shell; never write the
token into the repository.

## References

- Workflow: `.github/workflows/sonarcloud.yml`
- Scanner configuration: `sonar-project.properties`
- [SonarQube Cloud documentation](https://docs.sonarsource.com/sonarqube-cloud/)
