# Examples not migrated to Flutter

Examples under `examples/` that were removed or left without a Dart/Flutter port.

| Example | Reason |
| --- | --- |
| `bash` | Shell-script host (`sysinfo_*.sh`); not a Dart/Flutter app |
| `bevy` | Bevy game-engine hosting / GPU interop; not portable to `SlintView` |
| `dnd-kanban` | Needs Slint `DataTransfer` / drag `user_data` host APIs not exposed in the Dart bindings |
| `ffmpeg` | FFmpeg native video decode pipeline; not a UI-only demo |
| `gstreamer-player` | GStreamer video sink integration; requires native multimedia host |
| `maps` | Tile fetching + map math with a large host loop; deferred (UI alone is thin) |
| `mcu-board-support` | MCU / embedded board support only; no desktop Flutter host |
| `mcu-embassy` | Embassy MCU firmware demo; no desktop Flutter host |
| `opengl_texture` | Custom OpenGL texture rendering in the host; not available via Dart FFI |
| `opengl_underlay` | OpenGL underlay compositor; not available via Dart FFI |
| `plotter` | Uses the `plotters` crate to rasterize charts into a pixel buffer |
| `runtime_key_bindings` | Needs runtime `Keys` construction / capture APIs not exposed in Dart |
| `safe-ui` | Safety-critical C++ core + simulator; not a Flutter UI demo |
| `servo` | Embeds the Servo browser engine; far beyond a Slint UI port |
| `system-tray` | OS system-tray APIs; not supported through Flutter `SlintView` |
| `todo-mvc` | Full Rust MVC (controllers/repositories/adapters); large rewrite, skipped |
| `uefi-demo` | UEFI firmware target; no Flutter host |
| `virtual_keyboard` | Dispatches `WindowEvent::KeyPressed/Released` on the native window; not wired for embedded `SlintSurface` |
| `wgpu_texture` | wgpu shader texture interop; not available via Dart FFI |

Previously skipped and now ported: `repeater`, `imagefilter`.
Within `7guis`, Circle drawer and Cells are still unfinished (canvas / spreadsheet host logic).
