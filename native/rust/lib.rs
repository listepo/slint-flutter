// cSpell: ignore cdylib

//! C ABI over `slint-interpreter`, shaped for `dart:ffi`.
//!
//! `dart:ffi` speaks plain C: opaque pointers, `const char *`, and function
//! pointers. So this layer deliberately exposes nothing of Slint's Rust ABI —
//! no `SharedString`, no `SharedVector`, no `Box<Value>` — which spares the
//! Dart side from re-modelling layouts that carry no stability guarantee.
//!
//! Two conventions carry everything:
//!
//! * **Values travel as JSON.** The interpreter already converts between
//!   [`Value`] and JSON for the viewer and the LSP preview
//!   (`slint_interpreter::json`), and Dart already has `dart:convert`. Reusing
//!   both means neither side needs per-type marshalling code.
//! * **Fallible calls return a JSON envelope**: `{"ok": <value>}` or
//!   `{"err": "<message>"}`, as a heap-allocated NUL-terminated string that the
//!   caller releases with [`slint_dart_free_string`]. One decode step on the
//!   Dart side turns any error into an exception.
//!
//! Everything here must be called from the thread that runs the Slint event
//! loop, which is the Dart main isolate's thread. That matches the constraint
//! the Python and Node.js bindings already impose.

use i_slint_compiler::langtype::{Function, Type};
use i_slint_compiler::parser::normalize_identifier;
use i_slint_core::timers::{Timer, TimerMode};
use slint_interpreter::{
    CompilationResult, Compiler, ComponentDefinition, ComponentHandle, ComponentInstance, Value,
};
use std::ffi::{CStr, CString, c_char, c_void};
use std::path::PathBuf;
use std::time::Duration;

use values::{dart_value_from_json, dart_value_from_json_str, dart_value_to_json};

mod compiled;
mod embedded;
mod values;

#[cfg(target_arch = "wasm32")]
mod wasm;

// ---------------------------------------------------------------------------
// Envelope and string helpers
// ---------------------------------------------------------------------------

/// Move a string onto the heap for the Dart side, which frees it again with
/// [`slint_dart_free_string`].
pub(crate) fn into_c_string(s: String) -> *mut c_char {
    // JSON never contains an interior NUL, but a malformed payload shouldn't
    // take the process down, so fall back to an empty string instead.
    CString::new(s).unwrap_or_default().into_raw()
}

pub(crate) fn ok(value: serde_json::Value) -> *mut c_char {
    into_c_string(serde_json::json!({ "ok": value }).to_string())
}

pub(crate) fn ok_void() -> *mut c_char {
    ok(serde_json::Value::Null)
}

pub(crate) fn err(message: impl std::fmt::Display) -> *mut c_char {
    into_c_string(serde_json::json!({ "err": message.to_string() }).to_string())
}

/// Turn an unwind into an error envelope.
///
/// A panic that reaches an `extern "C"` frame aborts the process, which would
/// take the whole Dart application down. Slint panics for legitimate reasons
/// the caller can act on — creating a window off the main thread, for one — so
/// every entry point that can reach interpreter code stops the unwind here in
/// builds whose panic strategy supports unwinding and reports it like any other
/// error. A profile built with `panic = "abort"` cannot intercept a panic; this
/// crate leaves both profiles on the default `unwind` so that it can.
pub(crate) fn guard(body: impl FnOnce() -> *mut c_char) -> *mut c_char {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(body))
        .unwrap_or_else(|panic| err(panic_message(&panic)))
}

pub(crate) fn panic_message(panic: &Box<dyn std::any::Any + Send>) -> String {
    let detail = panic
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown reason".into());
    format!("Slint panicked: {detail}")
}

/// Borrow a C string as `&str`. A null pointer becomes `None`.
///
/// # Safety
/// `s` must be null or point to a NUL-terminated string that outlives the call.
pub(crate) unsafe fn opt_str<'a>(s: *const c_char) -> Option<&'a str> {
    (!s.is_null()).then(|| unsafe { CStr::from_ptr(s) }.to_str().unwrap_or_default())
}

/// Same as [`opt_str`], but treats null as the empty string.
///
/// # Safety
/// See [`opt_str`].
pub(crate) unsafe fn str_or_empty<'a>(s: *const c_char) -> &'a str {
    unsafe { opt_str(s) }.unwrap_or_default()
}

/// Release a string returned by any of the functions in this module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_free_string(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

// ---------------------------------------------------------------------------
// Type lookup
//
// Converting JSON into a `Value` needs the declared type of the target
// property, callback argument, or return value. `ComponentDefinition` is the
// only place that knows it, so every conversion starts from the instance's
// definition, the same way the viewer's `--load-data` does.
// ---------------------------------------------------------------------------

fn lookup_type(def: &ComponentDefinition, global: Option<&str>, name: &str) -> Option<Type> {
    // `-` and `_` are the same character to Slint, and the two sides of this
    // ABI disagree on which one they hand over: the interpreter reports a
    // property the way the `.slint` spells it, while a generated wrapper asks
    // with the compiler's canonical spelling. Normalizing both is what the
    // interpreter itself does for the get/set that follows this lookup, so
    // `status_message` and `status-message` name one property here too.
    let wanted = normalize_identifier(name);
    let matches = |candidate: &str| normalize_identifier(candidate) == wanted;
    let found = match global {
        None => def.properties_and_callbacks().find(|(n, _)| matches(n)),
        Some(global) => def.global_properties_and_callbacks(global)?.find(|(n, _)| matches(n)),
    };
    found.map(|(_, (ty, _))| ty)
}

/// The signature of a callback or a public function, whichever `ty` is.
fn as_function(ty: &Type) -> Option<&Function> {
    match ty {
        Type::Callback(f) | Type::Function(f) => Some(f),
        _ => None,
    }
}

fn parse_args(json: &str, signature: &Function) -> Result<Vec<Value>, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("invalid argument list: {e}"))?;
    let array = parsed.as_array().ok_or_else(|| "arguments must be a JSON array".to_string())?;
    if array.len() != signature.args.len() {
        return Err(format!("expected {} argument(s), got {}", signature.args.len(), array.len()));
    }
    array.iter().zip(signature.args.iter()).map(|(v, ty)| dart_value_from_json(ty, v)).collect()
}

fn values_to_json(values: &[Value]) -> Result<serde_json::Value, String> {
    values
        .iter()
        .map(dart_value_to_json)
        .collect::<Result<Vec<_>, _>>()
        .map(serde_json::Value::Array)
}

/// Create a compiler. Release it with [`slint_dart_compiler_free`].
#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_compiler_new() -> *mut Compiler {
    Box::into_raw(Box::new(Compiler::default()))
}

/// # Safety
/// `compiler` must come from [`slint_dart_compiler_new`] and not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_compiler_free(compiler: *mut Compiler) {
    if !compiler.is_null() {
        drop(unsafe { Box::from_raw(compiler) });
    }
}

/// # Safety
/// `compiler` must be a live compiler, `style` a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_compiler_set_style(
    compiler: &mut Compiler,
    style: *const c_char,
) {
    compiler.set_style(unsafe { str_or_empty(style) }.to_string());
}

/// # Safety
/// `compiler` must be a live compiler, `path` a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_compiler_add_include_path(
    compiler: &mut Compiler,
    path: *const c_char,
) {
    let mut paths = compiler.include_paths().clone();
    paths.push(PathBuf::from(unsafe { str_or_empty(path) }));
    compiler.set_include_paths(paths);
}

/// Compile a `.slint` file. Inspect the result with
/// [`slint_dart_result_diagnostics`] and [`slint_dart_result_component`]; it is
/// null only if the compiler itself panicked.
///
/// # Safety
/// `compiler` must be a live compiler, `path` a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_compiler_build_from_path(
    compiler: &Compiler,
    path: *const c_char,
) -> *mut CompilationResult {
    let path = PathBuf::from(unsafe { str_or_empty(path) });
    into_raw_or_null(|| spin_on::spin_on(compiler.build_from_path(path)))
}

/// Compile `.slint` source code. `path` is only used for diagnostics and to
/// resolve relative imports. See [`slint_dart_compiler_build_from_path`].
///
/// # Safety
/// `compiler` must be a live compiler, `source` and `path` NUL-terminated strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_compiler_build_from_source(
    compiler: &Compiler,
    source: *const c_char,
    path: *const c_char,
) -> *mut CompilationResult {
    let source = unsafe { str_or_empty(source) }.to_string();
    let path = PathBuf::from(unsafe { str_or_empty(path) });
    into_raw_or_null(|| spin_on::spin_on(compiler.build_from_source(source, path)))
}

/// Instantiate a component from a compilation unit produced at generate time.
/// Returns null on failure, with the reason written to `error` (release it
/// with [`slint_dart_free_string`]).
///
/// # Safety
/// `module_blob` and `component` must be NUL-terminated strings (`component`
/// may be null). `error` must be a valid pointer to a `*mut c_char`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_load_compiled(
    module_blob: *const c_char,
    component: *const c_char,
    error: *mut *mut c_char,
) -> *mut ComponentInstance {
    let blob = unsafe { str_or_empty(module_blob) };
    let component = unsafe { opt_str(component) };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        compiled::instantiate(blob, component)
    })) {
        Ok(Ok(instance)) => Box::into_raw(Box::new(instance)),
        Ok(Err(message)) => {
            unsafe { *error = into_c_string(message) };
            std::ptr::null_mut()
        }
        Err(panic) => {
            unsafe { *error = into_c_string(panic_message(&panic)) };
            std::ptr::null_mut()
        }
    }
}

/// Box the result of `body`, or return null if it panicked. See [`guard`].
fn into_raw_or_null<T>(body: impl FnOnce() -> T) -> *mut T {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)) {
        Ok(value) => Box::into_raw(Box::new(value)),
        Err(panic) => {
            eprintln!("{}", panic_message(&panic));
            std::ptr::null_mut()
        }
    }
}

// ---------------------------------------------------------------------------
// Compilation result
// ---------------------------------------------------------------------------

/// # Safety
/// `result` must come from a `build_from_*` call and not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_result_free(result: *mut CompilationResult) {
    if !result.is_null() {
        drop(unsafe { Box::from_raw(result) });
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_result_has_errors(result: &CompilationResult) -> bool {
    result.has_errors()
}

/// All diagnostics as a JSON array of
/// `{"level": "error"|"warning", "message": …, "file": …, "line": …, "column": …}`.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_result_diagnostics(result: &CompilationResult) -> *mut c_char {
    let diagnostics = result
        .diagnostics()
        .map(|d| {
            let (line, column) = d.line_column();
            serde_json::json!({
                "level": match d.level() {
                    i_slint_compiler::diagnostics::DiagnosticLevel::Error => "error",
                    _ => "warning",
                },
                "message": d.message(),
                "file": d.source_file().map(|p| p.display().to_string()),
                "line": line,
                "column": column,
            })
        })
        .collect::<Vec<_>>();
    ok(serde_json::Value::Array(diagnostics))
}

/// The names of every component that can be instantiated, as a JSON array.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_result_component_names(result: &CompilationResult) -> *mut c_char {
    ok(result.component_names().collect::<Vec<_>>().into())
}

/// Look up a component by name; a null `name` picks the last exported one.
/// Returns null when there is no such component.
///
/// # Safety
/// `name` must be null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_result_component(
    result: &CompilationResult,
    name: *const c_char,
) -> *mut ComponentDefinition {
    let definition = match unsafe { opt_str(name) } {
        Some(name) => result.component(name),
        None => result.components().last(),
    };
    definition.map_or(std::ptr::null_mut(), |d| Box::into_raw(Box::new(d)))
}

// ---------------------------------------------------------------------------
// Component definition
// ---------------------------------------------------------------------------

/// # Safety
/// `definition` must come from [`slint_dart_result_component`] and not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_definition_free(definition: *mut ComponentDefinition) {
    if !definition.is_null() {
        drop(unsafe { Box::from_raw(definition) });
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_definition_name(definition: &ComponentDefinition) -> *mut c_char {
    into_c_string(definition.name().to_string())
}

/// The public API of the component, as
/// `{"properties": {name: type}, "callbacks": [...], "functions": [...], "globals": [...]}`.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_definition_api(definition: &ComponentDefinition) -> *mut c_char {
    let properties = definition
        .properties()
        .map(|(name, ty)| (name, serde_json::Value::from(format!("{ty:?}"))))
        .collect::<serde_json::Map<_, _>>();
    ok(serde_json::json!({
        "properties": properties,
        "callbacks": definition.callbacks().collect::<Vec<_>>(),
        "functions": definition.functions().collect::<Vec<_>>(),
        "globals": definition.globals().collect::<Vec<_>>(),
    }))
}

/// Instantiate the component. Returns null on failure, with the reason written
/// to `error` (release it with [`slint_dart_free_string`]).
///
/// # Safety
/// `error` must be a valid pointer to a `*mut c_char`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_definition_create(
    definition: &ComponentDefinition,
    error: *mut *mut c_char,
) -> *mut ComponentInstance {
    // Creating the window adapter is where Slint learns it is on the wrong
    // thread or has no usable backend, and it says so by panicking.
    let created = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| definition.create()));
    match created {
        Ok(Ok(instance)) => Box::into_raw(Box::new(instance)),
        Ok(Err(e)) => {
            unsafe { *error = into_c_string(e.to_string()) };
            std::ptr::null_mut()
        }
        Err(panic) => {
            unsafe { *error = into_c_string(panic_message(&panic)) };
            std::ptr::null_mut()
        }
    }
}

// ---------------------------------------------------------------------------
// Component instance
// ---------------------------------------------------------------------------

/// # Safety
/// `instance` must come from [`slint_dart_definition_create`] and not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_instance_free(instance: *mut ComponentInstance) {
    if !instance.is_null() {
        drop(unsafe { Box::from_raw(instance) });
    }
}

/// Read a property. Pass a non-null `global` to read it from a global singleton.
///
/// # Safety
/// `global` must be null or NUL-terminated, `name` NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_instance_get_property(
    instance: &ComponentInstance,
    global: *const c_char,
    name: *const c_char,
) -> *mut c_char {
    guard(|| {
        let name = unsafe { str_or_empty(name) };
        let value = match unsafe { opt_str(global) } {
            None => instance.get_property(name).map_err(|e| e.to_string()),
            Some(global) => instance.get_global_property(global, name).map_err(|e| e.to_string()),
        };
        match value.and_then(|v| dart_value_to_json(&v)) {
            Ok(json) => ok(json),
            Err(e) => err(e),
        }
    })
}

/// Write a property from its JSON representation. Pass a non-null `global` to
/// write it into a global singleton.
///
/// # Safety
/// `global` must be null or NUL-terminated, `name` and `json` NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_instance_set_property(
    instance: &ComponentInstance,
    global: *const c_char,
    name: *const c_char,
    json: *const c_char,
) -> *mut c_char {
    guard(|| {
        let global = unsafe { opt_str(global) };
        let name = unsafe { str_or_empty(name) };
        let json = unsafe { str_or_empty(json) };

        let Some(ty) = lookup_type(&instance.definition(), global, name) else {
            return err(format!("no such property: {name}"));
        };
        let value = match dart_value_from_json_str(&ty, json) {
            Ok(value) => value,
            Err(e) => return err(e),
        };
        let result = match global {
            None => instance.set_property(name, value).map_err(|e| e.to_string()),
            Some(global) => {
                instance.set_global_property(global, name, value).map_err(|e| e.to_string())
            }
        };
        result.map_or_else(err, |()| ok_void())
    })
}

/// Call a callback or a public function with a JSON array of arguments, and
/// return its result. Pass a non-null `global` to reach into a global singleton.
///
/// # Safety
/// `global` must be null or NUL-terminated, `name` and `args_json` NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_instance_invoke(
    instance: &ComponentInstance,
    global: *const c_char,
    name: *const c_char,
    args_json: *const c_char,
) -> *mut c_char {
    guard(|| {
        let global = unsafe { opt_str(global) };
        let name = unsafe { str_or_empty(name) };
        let args_json = unsafe { str_or_empty(args_json) };

        let definition = instance.definition();
        let Some(ty) = lookup_type(&definition, global, name) else {
            return err(format!("no such callback or function: {name}"));
        };
        let Some(signature) = as_function(&ty) else {
            return err(format!("{name} is a property, not a callback or function"));
        };
        let args = match parse_args(args_json, signature) {
            Ok(args) => args,
            Err(e) => return err(e),
        };
        let result = match global {
            None => instance.invoke(name, &args).map_err(|e| e.to_string()),
            Some(global) => instance.invoke_global(global, name, &args).map_err(|e| e.to_string()),
        };
        match result.and_then(|v| dart_value_to_json(&v)) {
            Ok(json) => ok(json),
            Err(e) => err(e),
        }
    })
}

/// The Dart handler for a Slint callback.
///
/// It receives the arguments as a JSON array and returns the result as a JSON
/// string it allocated itself, or null for a void callback. This module hands
/// that string straight back to `free_result` once it has been read, so the
/// two sides never free each other's allocations.
pub type DartCallback =
    unsafe extern "C" fn(user_data: *mut c_void, args_json: *const c_char) -> *mut c_char;

/// Releases a string returned by a [`DartCallback`].
pub type DartFree = unsafe extern "C" fn(s: *mut c_char);

struct DartHandler {
    callback: DartCallback,
    free_result: DartFree,
    user_data: *mut c_void,
    return_type: Type,
}

impl DartHandler {
    fn call(&self, args: &[Value]) -> Value {
        let args_json = match values_to_json(args) {
            Ok(json) => json.to_string(),
            Err(e) => {
                eprintln!("Slint: cannot pass callback arguments to Dart: {e}");
                return Value::Void;
            }
        };
        let Ok(args_json) = CString::new(args_json) else {
            return Value::Void;
        };

        let returned = unsafe { (self.callback)(self.user_data, args_json.as_ptr()) };
        if returned.is_null() {
            return Value::Void;
        }
        let json = unsafe { CStr::from_ptr(returned) }.to_string_lossy().into_owned();
        unsafe { (self.free_result)(returned) };

        match dart_value_from_json_str(&self.return_type, &json) {
            Ok(value) => value,
            Err(e) => {
                eprintln!("Slint: cannot convert the Dart callback result: {e}");
                Value::Void
            }
        }
    }
}

/// Install a Dart handler for a callback. Pass a non-null `global` to reach
/// into a global singleton.
///
/// # Safety
/// `global` must be null or NUL-terminated and `name` NUL-terminated.
/// `callback` and `free_result` must stay valid, and `user_data` must stay
/// meaningful, until the instance is destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_instance_set_callback(
    instance: &ComponentInstance,
    global: *const c_char,
    name: *const c_char,
    callback: DartCallback,
    free_result: DartFree,
    user_data: *mut c_void,
) -> *mut c_char {
    guard(|| {
        let global = unsafe { opt_str(global) };
        let name = unsafe { str_or_empty(name) };

        let definition = instance.definition();
        let Some(ty) = lookup_type(&definition, global, name) else {
            return err(format!("no such callback: {name}"));
        };
        let Some(signature) = as_function(&ty) else {
            return err(format!("{name} is a property, not a callback"));
        };

        let handler = DartHandler {
            callback,
            free_result,
            user_data,
            return_type: signature.return_type.clone(),
        };
        let result = match global {
            None => instance
                .set_callback(name, move |args| handler.call(args))
                .map_err(|e| e.to_string()),
            Some(global) => instance
                .set_global_callback(global, name, move |args| handler.call(args))
                .map_err(|e| e.to_string()),
        };
        result.map_or_else(err, |()| ok_void())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_instance_show(
    instance: &ComponentInstance,
    visible: bool,
) -> *mut c_char {
    guard(|| {
        let result = if visible { instance.show() } else { instance.hide() };
        result.map_or_else(err, |()| ok_void())
    })
}

/// Show the window and run the event loop until the last window closes or
/// [`slint_dart_quit_event_loop`] is called.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_instance_run(instance: &ComponentInstance) -> *mut c_char {
    guard(|| instance.run().map_or_else(err, |()| ok_void()))
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_run_event_loop() -> *mut c_char {
    guard(|| slint_interpreter::run_event_loop().map_or_else(err, |()| ok_void()))
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_quit_event_loop() {
    let _ = i_slint_core::api::quit_event_loop();
}

// ---------------------------------------------------------------------------
// Timers
//
// Dart's own timers never fire while `slint_dart_instance_run` owns the
// thread, so periodic work has to be driven by Slint's event loop.
// ---------------------------------------------------------------------------

/// Start a timer. Release it with [`slint_dart_timer_free`], which also stops it.
///
/// # Safety
/// `callback` must stay valid, and `user_data` meaningful, until the returned
/// timer is freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_timer_start(
    repeated: bool,
    interval_ms: u64,
    callback: unsafe extern "C" fn(user_data: *mut c_void),
    user_data: *mut c_void,
) -> *mut Timer {
    // A zero-interval repeating timer fires on every event-loop tick — a busy
    // loop with no backpressure. A single-shot zero timer ("as soon as
    // possible") is legitimate. Only checked in debug builds.
    debug_assert!(!(repeated && interval_ms == 0), "repeating timer with a zero interval");
    let mode = if repeated { TimerMode::Repeated } else { TimerMode::SingleShot };
    let timer = Box::new(Timer::default());
    // Raw pointers aren't `Send`, but Slint timers only ever fire on the event
    // loop thread, which is the same thread that installed them.
    let user_data = user_data as usize;
    timer.start(mode, Duration::from_millis(interval_ms), move || unsafe {
        callback(user_data as *mut c_void)
    });
    Box::into_raw(timer)
}

/// # Safety
/// `timer` must come from [`slint_dart_timer_start`] and not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_timer_free(timer: *mut Timer) {
    if !timer.is_null() {
        drop(unsafe { Box::from_raw(timer) });
    }
}

// ---------------------------------------------------------------------------
// Translations
//
// `@tr(...)` in `.slint` asks the host language for a string. Python hands
// Slint a gettext object; Dart hands it a callback, which is what `intl` and
// a hand-written map both look like.
// ---------------------------------------------------------------------------

struct DartTranslator {
    callback: DartCallback,
    free_result: DartFree,
    user_data: usize,
}

impl i_slint_core::translations::Translator for DartTranslator {
    fn translate<'a>(
        &'a self,
        string: &'a str,
        context: Option<&'a str>,
    ) -> std::borrow::Cow<'a, str> {
        std::borrow::Cow::Owned(self.invoke(string, context, None, 1))
    }

    fn ntranslate<'a>(
        &'a self,
        n: u64,
        singular: &'a str,
        plural: &'a str,
        context: Option<&'a str>,
    ) -> std::borrow::Cow<'a, str> {
        std::borrow::Cow::Owned(self.invoke(singular, context, Some(plural), n))
    }
}

impl DartTranslator {
    fn invoke(&self, string: &str, context: Option<&str>, plural: Option<&str>, n: u64) -> String {
        let payload = serde_json::json!({
            "string": string,
            "context": context,
            "plural": plural,
            "n": n,
        })
        .to_string();
        let Ok(payload) = CString::new(payload) else {
            return fallback(string, plural, n);
        };
        let returned = unsafe { (self.callback)(self.user_data as *mut c_void, payload.as_ptr()) };
        if returned.is_null() {
            return fallback(string, plural, n);
        }
        let json = unsafe { CStr::from_ptr(returned) }.to_string_lossy().into_owned();
        unsafe { (self.free_result)(returned) };
        match serde_json::from_str::<serde_json::Value>(&json) {
            Ok(serde_json::Value::String(translated)) => translated,
            _ => fallback(string, plural, n),
        }
    }
}

fn fallback(string: &str, plural: Option<&str>, n: u64) -> String {
    if n == 1 || plural.is_none() { string } else { plural.unwrap() }.to_string()
}

/// Install a Dart function as the translator for `@tr(...)` strings.
///
/// The callback receives a JSON object `{"string", "context", "plural", "n"}`
/// and returns a JSON string — the translated text — which `free_result`
/// then releases. A null return keeps the original string.
/// `enabled` is false to uninstall and fall back to the original strings.
///
/// The Slint platform has to exist first, which `SlintSurface` or creating a
/// component both arrange.
///
/// # Safety
/// `callback` and `free_result` must stay valid for as long as they remain
/// installed. `user_data` must stay meaningful for that long too.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_init_translations(
    callback: DartCallback,
    free_result: DartFree,
    user_data: *mut c_void,
    enabled: bool,
) -> *mut c_char {
    guard(|| {
        let translator = enabled.then(|| {
            Box::new(DartTranslator { callback, free_result, user_data: user_data as usize })
                as Box<dyn i_slint_core::translations::Translator>
        });
        match i_slint_core::with_global_context(
            || Err(i_slint_core::platform::PlatformError::NoPlatform),
            |ctx| ctx.set_external_translator(translator),
        ) {
            Ok(()) => ok_void(),
            Err(error) => err(format!(
                "{error}; create a SlintSurface or a component before installing translations"
            )),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read back and release an envelope, the way the Dart side does.
    pub(crate) fn envelope(ptr: *mut c_char) -> serde_json::Value {
        assert!(!ptr.is_null());
        let json = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap().to_string();
        unsafe { slint_dart_free_string(ptr) };
        serde_json::from_str(&json).unwrap()
    }

    pub(crate) fn unwrap_ok(ptr: *mut c_char) -> serde_json::Value {
        let value = envelope(ptr);
        assert!(value.get("err").is_none(), "unexpected error: {value}");
        value["ok"].clone()
    }

    pub(crate) fn unwrap_err(ptr: *mut c_char) -> String {
        let value = envelope(ptr);
        value["err"].as_str().expect("expected an error envelope").to_string()
    }

    pub(crate) fn c(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    fn compiled_blob_from_dart(source: &str) -> &str {
        const MARKER: &str = "instantiateCompiled(";
        let start = source.find(MARKER).expect("generated source instantiates a compiled module");
        let after = &source[start + MARKER.len()..];
        let quote = after.find('"').expect("compiled module string");
        let body = &after[quote + 1..];
        let end = body.find('"').expect("compiled module string terminator");
        &body[..end]
    }

    /// Compile `source` and instantiate its only component.
    fn instantiate(source: &str) -> ComponentInstance {
        i_slint_backend_testing::init_no_event_loop();
        let compiler = Compiler::default();
        let result = spin_on::spin_on(
            compiler.build_from_source(source.into(), PathBuf::from("test.slint")),
        );
        assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
        result.components().last().unwrap().create().unwrap()
    }

    const COUNTER: &str = r#"
        export struct Item { title: string, checked: bool }
        export global Logic {
            in-out property <int> offset: 3;
            callback shout(string) -> string;
            callback noted();
        }
        export component App {
            in-out property <int> value: 42;
            in-out property <string> label: "hello";
            in-out property <[Item]> items: [{ title: "a", checked: true }];
            callback add(string) -> int;
            public function double(v: int) -> int { v * 2 }
        }
    "#;

    #[test]
    fn get_and_set_properties() {
        let instance = instantiate(COUNTER);

        assert_eq!(unwrap_ok(unsafe { get(&instance, None, "value") }), 42);
        assert_eq!(unwrap_ok(unsafe { get(&instance, None, "label") }), "hello");

        unwrap_ok(unsafe { set(&instance, None, "value", "7") });
        assert_eq!(unwrap_ok(unsafe { get(&instance, None, "value") }), 7);

        unwrap_ok(unsafe { set(&instance, None, "label", "\"bye\"") });
        assert_eq!(unwrap_ok(unsafe { get(&instance, None, "label") }), "bye");
    }

    /// A `.slint` may spell a name with `_` while the compiler's canonical form
    /// uses `-`, and generated wrappers ask with the canonical one. Both must
    /// reach the same property, for properties, callbacks and functions alike.
    #[test]
    fn underscores_and_hyphens_name_the_same_member() {
        let instance = instantiate(
            r#"
            export global Logic { in-out property <int> some_offset: 1; }
            export component App {
                in-out property <string> status_message: "ready";
                callback did_something(int);
                public function do_work(v: int) -> int { v * 2 }
            }
            "#,
        );

        // Writing with one spelling must be visible through the other.
        assert_eq!(unwrap_ok(unsafe { get(&instance, None, "status_message") }), "ready");
        unwrap_ok(unsafe { set(&instance, None, "status-message", "\"canonical\"") });
        assert_eq!(unwrap_ok(unsafe { get(&instance, None, "status_message") }), "canonical");
        unwrap_ok(unsafe { set(&instance, None, "status_message", "\"as written\"") });
        assert_eq!(unwrap_ok(unsafe { get(&instance, None, "status-message") }), "as written");
        for name in ["do_work", "do-work"] {
            let name = c(name);
            let args = c("[21]");
            let result = unsafe {
                slint_dart_instance_invoke(
                    &instance,
                    std::ptr::null(),
                    name.as_ptr(),
                    args.as_ptr(),
                )
            };
            assert_eq!(unwrap_ok(result), 42);
        }
        for name in ["some_offset", "some-offset"] {
            assert_eq!(unwrap_ok(unsafe { get(&instance, Some("Logic"), name) }), 1);
        }
        // A name that is genuinely absent still reports one.
        assert!(unwrap_err(unsafe { get(&instance, None, "nope") }).contains("no such property"));
    }

    #[test]
    fn models_round_trip_as_json_arrays() {
        let instance = instantiate(COUNTER);

        assert_eq!(
            unwrap_ok(unsafe { get(&instance, None, "items") }),
            serde_json::json!([{ "title": "a", "checked": true }])
        );

        unwrap_ok(unsafe {
            set(
                &instance,
                None,
                "items",
                r#"[{"title": "b", "checked": false}, {"title": "c", "checked": true}]"#,
            )
        });
        assert_eq!(
            unwrap_ok(unsafe { get(&instance, None, "items") }),
            serde_json::json!([
                { "title": "b", "checked": false },
                { "title": "c", "checked": true },
            ])
        );
    }

    #[test]
    fn global_properties() {
        let instance = instantiate(COUNTER);
        assert_eq!(unwrap_ok(unsafe { get(&instance, Some("Logic"), "offset") }), 3);
        unwrap_ok(unsafe { set(&instance, Some("Logic"), "offset", "9") });
        assert_eq!(unwrap_ok(unsafe { get(&instance, Some("Logic"), "offset") }), 9);
    }

    #[test]
    fn unknown_names_report_errors_instead_of_panicking() {
        let instance = instantiate(COUNTER);
        assert!(unwrap_err(unsafe { get(&instance, None, "nope") }).contains("no such property"));
        assert!(
            unwrap_err(unsafe { set(&instance, None, "nope", "1") }).contains("no such property")
        );
        assert!(
            unwrap_err(unsafe { get(&instance, Some("Nope"), "offset") })
                .contains("no such property")
        );
    }

    #[test]
    fn setting_a_property_to_the_wrong_type_reports_an_error() {
        let instance = instantiate(COUNTER);
        let message = unwrap_err(unsafe { set(&instance, None, "value", "\"not a number\"") });
        assert!(!message.is_empty(), "expected a conversion error");
        // The property keeps its previous value.
        assert_eq!(unwrap_ok(unsafe { get(&instance, None, "value") }), 42);
    }

    #[test]
    fn invoke_a_public_function() {
        let instance = instantiate(COUNTER);
        let name = c("double");
        let args = c("[21]");
        let result = unsafe {
            slint_dart_instance_invoke(&instance, std::ptr::null(), name.as_ptr(), args.as_ptr())
        };
        assert_eq!(unwrap_ok(result), 42);
    }

    #[test]
    fn invoke_checks_the_argument_count() {
        let instance = instantiate(COUNTER);
        let name = c("double");
        let args = c("[]");
        let result = unsafe {
            slint_dart_instance_invoke(&instance, std::ptr::null(), name.as_ptr(), args.as_ptr())
        };
        assert!(unwrap_err(result).contains("expected 1 argument(s), got 0"));
    }

    // A callback handler standing in for the Dart side: it records the
    // arguments it saw and answers with a JSON string it allocated itself.
    thread_local! {
        static SEEN_ARGS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    unsafe extern "C" fn recording_handler(
        user_data: *mut c_void,
        args_json: *const c_char,
    ) -> *mut c_char {
        let args = unsafe { CStr::from_ptr(args_json) }.to_str().unwrap().to_string();
        SEEN_ARGS.with(|seen| seen.borrow_mut().push(args));
        // `user_data` is the id the caller passed in; echo it back as the result.
        into_c_string((user_data as usize).to_string())
    }

    unsafe extern "C" fn shout_handler(
        _user_data: *mut c_void,
        args_json: *const c_char,
    ) -> *mut c_char {
        let args: serde_json::Value =
            serde_json::from_str(unsafe { CStr::from_ptr(args_json) }.to_str().unwrap()).unwrap();
        into_c_string(serde_json::Value::from(args[0].as_str().unwrap().to_uppercase()).to_string())
    }

    unsafe extern "C" fn free_handler_result(s: *mut c_char) {
        unsafe { slint_dart_free_string(s) };
    }

    #[test]
    fn callbacks_receive_arguments_and_return_values() {
        let instance = instantiate(COUNTER);
        SEEN_ARGS.with(|seen| seen.borrow_mut().clear());

        let name = c("add");
        unwrap_ok(unsafe {
            slint_dart_instance_set_callback(
                &instance,
                std::ptr::null(),
                name.as_ptr(),
                recording_handler,
                free_handler_result,
                17 as *mut c_void,
            )
        });

        let args = c(r#"["milk"]"#);
        let result = unsafe {
            slint_dart_instance_invoke(&instance, std::ptr::null(), name.as_ptr(), args.as_ptr())
        };
        assert_eq!(unwrap_ok(result), 17);
        SEEN_ARGS.with(|seen| assert_eq!(seen.borrow().as_slice(), [r#"["milk"]"#]));
    }

    #[test]
    fn global_callbacks() {
        let instance = instantiate(COUNTER);
        let global = c("Logic");
        let name = c("shout");
        unwrap_ok(unsafe {
            slint_dart_instance_set_callback(
                &instance,
                global.as_ptr(),
                name.as_ptr(),
                shout_handler,
                free_handler_result,
                std::ptr::null_mut(),
            )
        });

        let args = c(r#"["hello"]"#);
        let result = unsafe {
            slint_dart_instance_invoke(&instance, global.as_ptr(), name.as_ptr(), args.as_ptr())
        };
        assert_eq!(unwrap_ok(result), "HELLO");
    }

    #[test]
    fn a_void_callback_may_answer_with_null() {
        let instance = instantiate(COUNTER);

        unsafe extern "C" fn void_handler(
            _user_data: *mut c_void,
            _args_json: *const c_char,
        ) -> *mut c_char {
            SEEN_ARGS.with(|seen| seen.borrow_mut().push("noted".into()));
            std::ptr::null_mut()
        }

        SEEN_ARGS.with(|seen| seen.borrow_mut().clear());
        let global = c("Logic");
        let name = c("noted");
        unwrap_ok(unsafe {
            slint_dart_instance_set_callback(
                &instance,
                global.as_ptr(),
                name.as_ptr(),
                void_handler,
                free_handler_result,
                std::ptr::null_mut(),
            )
        });

        let args = c("[]");
        unwrap_ok(unsafe {
            slint_dart_instance_invoke(&instance, global.as_ptr(), name.as_ptr(), args.as_ptr())
        });
        SEEN_ARGS.with(|seen| assert_eq!(seen.borrow().len(), 1));
    }

    #[test]
    fn a_panic_becomes_an_error_envelope_instead_of_aborting() {
        // Silence the panic hook so the deliberate panic below doesn't look
        // like a test failure in the output.
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let result = guard(|| panic!("boom"));
        std::panic::set_hook(previous);

        let message = unwrap_err(result);
        assert!(message.contains("boom"), "{message}");
    }

    #[test]
    fn compile_errors_are_reported_as_diagnostics() {
        i_slint_backend_testing::init_no_event_loop();
        let compiler = slint_dart_compiler_new();
        let source = c("export component Broken { this is not slint }");
        let path = c("broken.slint");
        let result = unsafe {
            slint_dart_compiler_build_from_source(&*compiler, source.as_ptr(), path.as_ptr())
        };

        assert!(slint_dart_result_has_errors(unsafe { &*result }));
        let diagnostics = unwrap_ok(slint_dart_result_diagnostics(unsafe { &*result }));
        let diagnostics = diagnostics.as_array().unwrap();
        assert!(!diagnostics.is_empty());
        assert_eq!(diagnostics[0]["level"], "error");
        assert!(diagnostics[0]["message"].as_str().is_some_and(|m| !m.is_empty()));

        unsafe { slint_dart_result_free(result) };
        unsafe { slint_dart_compiler_free(compiler) };
    }

    /// The whole point of the split: `codegen` turns a `.slint` into Dart, and
    /// this library instantiates what that Dart carries — with the original
    /// files deleted, and without linking the compiler that produced them.
    /// `slint-dart-codegen` is a dev-dependency here, never a real one.
    #[test]
    fn a_generated_wrapper_instantiates_with_no_slint_source_left() {
        i_slint_backend_testing::init_no_event_loop();
        let directory = std::env::temp_dir().join(format!(
            "slint-dart-aot-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("shared.slint"), "export component Shared { }").unwrap();
        let input = directory.join("app.slint");
        std::fs::write(
            &input,
            r#"
                import { Shared } from "shared.slint";
                export component App inherits Shared {
                    in-out property <int> n: 9;
                }
            "#,
        )
        .unwrap();

        let generation = slint_dart_codegen::generate(
            &input,
            &directory.join("app.slint.dart"),
            slint_dart_codegen::Options::default(),
        );
        let source = generation.dart.expect("generated Dart");
        let blob = compiled_blob_from_dart(&source).to_string();

        // Nothing may be read from disk from here on.
        std::fs::remove_dir_all(&directory).unwrap();

        let instance = compiled::instantiate(&blob, Some("App")).unwrap();
        assert_eq!(instance.get_property("n").unwrap(), slint_interpreter::Value::Number(9.0));
    }

    /// The Rust generators embed a resource once per path, and so does this
    /// bundler: an image referenced twice is carried once as an asset, not
    /// twice as inlined data URIs, and it still instantiates with no source
    /// (or image) left on disk.
    #[test]
    fn a_generated_wrapper_bundles_repeated_images_once() {
        i_slint_backend_testing::init_no_event_loop();
        let directory = std::env::temp_dir().join(format!(
            "slint-dart-aot-img-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        // A 1x1 red PNG.
        let png = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
        )
        .unwrap();
        std::fs::write(directory.join("pixel.png"), &png).unwrap();
        let input = directory.join("app.slint");
        std::fs::write(
            &input,
            r#"
                export component App {
                    in-out property <image> a: @image-url("pixel.png");
                    in-out property <image> b: @image-url("pixel.png");
                }
            "#,
        )
        .unwrap();

        let generation = slint_dart_codegen::generate(
            &input,
            &directory.join("app.slint.dart"),
            slint_dart_codegen::Options::default(),
        );
        let source = generation.dart.expect("generated Dart");
        let blob = compiled_blob_from_dart(&source).to_string();

        let module = compiled::decode_module(&blob).unwrap();
        let files = module["files"].as_object().unwrap();
        let assets = module["assets"].as_object().unwrap();
        assert_eq!(assets.len(), 1, "the repeated image must be bundled once: {assets:?}");
        assert!(assets.contains_key("/slint-aot/pixel.png"));
        assert!(
            !files.values().any(|v| v.as_str().is_some_and(|s| s.contains("data:image"))),
            "no image may be inlined into the source"
        );

        // Nothing may be read from disk from here on.
        std::fs::remove_dir_all(&directory).unwrap();

        let instance = compiled::instantiate(&blob, Some("App")).unwrap();
        let slint_interpreter::Value::Image(image) = instance.get_property("a").unwrap() else {
            panic!("expected an image property");
        };
        let path = image.path().expect("image resolves to a path").to_string_lossy().into_owned();
        assert!(
            path.contains("slint-aot") && path.ends_with("pixel.png"),
            "unexpected path: {path}"
        );
        assert!(std::path::Path::new(&path).exists(), "materialized image missing: {path}");
    }

    #[test]
    fn a_missing_component_returns_null() {
        i_slint_backend_testing::init_no_event_loop();
        let compiler = slint_dart_compiler_new();
        let source = c("export component App { }");
        let path = c("app.slint");
        let result = unsafe {
            slint_dart_compiler_build_from_source(&*compiler, source.as_ptr(), path.as_ptr())
        };

        let names = unwrap_ok(slint_dart_result_component_names(unsafe { &*result }));
        assert_eq!(names, serde_json::json!(["App"]));

        let missing = c("Nope");
        assert!(
            unsafe { slint_dart_result_component(&*result, missing.as_ptr()) }.is_null(),
            "an unknown component name must not produce a definition"
        );

        // A null name picks a component without having to know its name.
        let definition = unsafe { slint_dart_result_component(&*result, std::ptr::null()) };
        assert!(!definition.is_null());
        let name = slint_dart_definition_name(unsafe { &*definition });
        assert_eq!(unsafe { CStr::from_ptr(name) }.to_str().unwrap(), "App");

        unsafe { slint_dart_free_string(name) };
        unsafe { slint_dart_definition_free(definition) };
        unsafe { slint_dart_result_free(result) };
        unsafe { slint_dart_compiler_free(compiler) };
    }

    #[test]
    fn images_round_trip_from_pixels_and_from_a_path() {
        let instance = instantiate(
            r#"
                export component App {
                    in-out property <image> icon;
                }
            "#,
        );

        assert_eq!(unwrap_ok(unsafe { get(&instance, None, "icon") }), serde_json::Value::Null);

        let rgba = vec![255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255];
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &rgba);
        unwrap_ok(unsafe {
            set(
                &instance,
                None,
                "icon",
                &serde_json::json!({"width": 2, "height": 2, "rgba": encoded}).to_string(),
            )
        });
        let got = unwrap_ok(unsafe { get(&instance, None, "icon") });
        assert_eq!(got["width"], 2);
        assert_eq!(got["height"], 2);
        let round_trip = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            got["rgba"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(round_trip, rgba);

        assert!(
            unwrap_err(unsafe { set(&instance, None, "icon", "\"/definitely/not/here.png\"") })
                .contains("Failed to load image from path")
        );

        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="#00ff00"/></svg>"##;
        unwrap_ok(unsafe {
            set(&instance, None, "icon", &serde_json::json!({ "svg": svg }).to_string())
        });
        let loaded = unwrap_ok(unsafe { get(&instance, None, "icon") });
        assert_eq!(loaded["width"], 2);
        assert_eq!(loaded["height"], 2);
    }

    unsafe extern "C" fn shout_translate(
        _user_data: *mut c_void,
        args_json: *const c_char,
    ) -> *mut c_char {
        let request: serde_json::Value =
            serde_json::from_str(unsafe { CStr::from_ptr(args_json) }.to_str().unwrap()).unwrap();
        let string = request["string"].as_str().unwrap().to_uppercase();
        into_c_string(serde_json::Value::from(string).to_string())
    }

    #[test]
    fn translations_replace_tr_strings() {
        let instance = instantiate(
            r#"
                export component App {
                    in-out property <string> greeting: @tr("Hello");
                }
            "#,
        );
        assert_eq!(unwrap_ok(unsafe { get(&instance, None, "greeting") }), "Hello");

        unwrap_ok(unsafe {
            slint_dart_init_translations(
                shout_translate,
                free_handler_result,
                std::ptr::null_mut(),
                true,
            )
        });
        assert_eq!(unwrap_ok(unsafe { get(&instance, None, "greeting") }), "HELLO");

        unwrap_ok(unsafe {
            slint_dart_init_translations(
                shout_translate,
                free_handler_result,
                std::ptr::null_mut(),
                false,
            )
        });
    }

    // Thin wrappers so the tests above read as calls rather than pointer juggling.
    unsafe fn get(instance: &ComponentInstance, global: Option<&str>, name: &str) -> *mut c_char {
        let global = global.map(c);
        let name = c(name);
        unsafe {
            slint_dart_instance_get_property(
                instance,
                global.as_ref().map_or(std::ptr::null(), |g| g.as_ptr()),
                name.as_ptr(),
            )
        }
    }

    unsafe fn set(
        instance: &ComponentInstance,
        global: Option<&str>,
        name: &str,
        json: &str,
    ) -> *mut c_char {
        let global = global.map(c);
        let name = c(name);
        let json = c(json);
        unsafe {
            slint_dart_instance_set_property(
                instance,
                global.as_ref().map_or(std::ptr::null(), |g| g.as_ptr()),
                name.as_ptr(),
                json.as_ptr(),
            )
        }
    }
}
