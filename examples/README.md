# Examples

Flutter applications that use [`package:slint`](../slint) and
[`package:slint_flutter`](../slint_flutter). Each demo lives under
`examples/<name>/flutter` and loads a generated typed wrapper via
`Component.load()`.

Examples that were not migrated (and why) are listed in
[`fail.md`](./fail.md).

| Example | Path |
| --- | --- |
| Todo | [`todo/flutter`](./todo/flutter) |
| 7GUIs | [`7guis/flutter`](./7guis/flutter) |
| Async I/O | [`async-io/flutter`](./async-io/flutter) |
| Carousel | [`carousel/flutter`](./carousel/flutter) |
| Dial | [`dial/flutter`](./dial/flutter) |
| Fancy switches | [`fancy-switches/flutter`](./fancy-switches/flutter) |
| Fancy demo | [`fancy_demo/flutter`](./fancy_demo/flutter) |
| Gallery | [`gallery/flutter`](./gallery/flutter) |
| Image filter | [`imagefilter/flutter`](./imagefilter/flutter) |
| IoT dashboard | [`iot-dashboard/flutter`](./iot-dashboard/flutter) |
| Layouts | [`layouts/flutter`](./layouts/flutter) |
| Memory | [`memory/flutter`](./memory/flutter) |
| Native gestures | [`native-gestures/flutter`](./native-gestures/flutter) |
| Orbit animation | [`orbit-animation/flutter`](./orbit-animation/flutter) |
| Repeater | [`repeater/flutter`](./repeater/flutter) |
| Slide puzzle | [`slide_puzzle/flutter`](./slide_puzzle/flutter) |
| Speedometer | [`speedometer/flutter`](./speedometer/flutter) |
| Sprite sheet | [`sprite-sheet/flutter`](./sprite-sheet/flutter) |

The counter code-generation sample lives outside this tree at
[`slint_flutter/example`](../slint_flutter/example).

Build pattern (from the repository root):

```sh
cd native
cargo build --release -p slint-dart-codegen
cargo build --release -p slint-dart
cd ../examples/<name>/flutter
dart pub get
dart run build_runner build --delete-conflicting-outputs
flutter create . --platforms=macos --project-name <package_name>
flutter run -d macos
```
