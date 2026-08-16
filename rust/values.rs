//! JSON conversion for values the interpreter's `internal-json` helper cannot
//! round-trip: images without a path, and images built from pixels or encoded
//! bytes rather than a filesystem path.

use std::collections::HashMap;
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use i_slint_compiler::langtype::Type;
use i_slint_core::graphics::{Image, Rgba8Pixel, SharedPixelBuffer};
use i_slint_core::model::Model;
use slint_interpreter::json::{value_from_json, value_to_json};
use slint_interpreter::{Struct, Value};

pub(crate) fn dart_value_to_json(value: &Value) -> Result<serde_json::Value, String> {
    match value {
        Value::Image(image) => image_to_json(image),
        Value::Model(model) => Ok(serde_json::Value::Array(
            model.iter().map(|item| dart_value_to_json(&item)).collect::<Result<Vec<_>, _>>()?,
        )),
        Value::Struct(structure) => Ok(serde_json::Value::Object(
            structure
                .iter()
                .map(|(name, item)| dart_value_to_json(item).map(|json| (name.to_string(), json)))
                .collect::<Result<serde_json::Map<_, _>, _>>()?,
        )),
        other => value_to_json(other),
    }
}

pub(crate) fn dart_value_from_json(ty: &Type, value: &serde_json::Value) -> Result<Value, String> {
    match ty {
        Type::Image => image_from_json(value),
        Type::Array(element) => {
            let array = value
                .as_array()
                .ok_or_else(|| "Got an array where none was expected".to_string())?;
            let items = array
                .iter()
                .map(|item| dart_value_from_json(element, item))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Value::Model(std::rc::Rc::new(i_slint_core::model::VecModel::from(items)).into()))
        }
        Type::Struct(structure) => {
            let object = value
                .as_object()
                .ok_or_else(|| "Got a struct where none was expected".to_string())?;
            let mut fields = HashMap::new();
            for (name, item) in object {
                let Some(field_type) = structure
                    .fields
                    .get(name.as_str())
                    .or_else(|| structure.fields.get(name.replace('_', "-").as_str()))
                else {
                    return Err(format!("Found unknown field in struct: {name}"));
                };
                fields.insert(name.clone(), dart_value_from_json(field_type, item)?);
            }
            Ok(Struct::from_iter(fields).into())
        }
        _ => value_from_json(ty, value),
    }
}

pub(crate) fn dart_value_from_json_str(ty: &Type, json: &str) -> Result<Value, String> {
    let value =
        serde_json::from_str(json).map_err(|error| format!("Failed to parse JSON: {error}"))?;
    dart_value_from_json(ty, &value)
}

fn image_to_json(image: &Image) -> Result<serde_json::Value, String> {
    let size = image.size();
    if let Some(path) = image.path() {
        return Ok(serde_json::json!({
            "path": path.to_string_lossy(),
            "width": size.width,
            "height": size.height,
        }));
    }
    if size.width == 0 && size.height == 0 {
        return Ok(serde_json::Value::Null);
    }
    let buffer = image.to_rgba8().ok_or_else(|| "Cannot serialize this image".to_string())?;
    Ok(serde_json::json!({
        "width": buffer.width(),
        "height": buffer.height(),
        "rgba": STANDARD.encode(buffer.as_bytes()),
    }))
}

fn image_from_json(value: &serde_json::Value) -> Result<Value, String> {
    match value {
        serde_json::Value::Null => Ok(Image::default().into()),
        serde_json::Value::String(path) => load_from_path(path),
        serde_json::Value::Object(object) => {
            if let Some(path) = object.get("path").and_then(serde_json::Value::as_str) {
                return load_from_path(path);
            }
            if let Some(svg) = object.get("svg") {
                let bytes = json_bytes(svg, "svg")?;
                return Image::load_from_svg_data(&bytes)
                    .map(Into::into)
                    .map_err(|error| format!("Failed to load SVG image: {error}"));
            }
            if let Some(data) = object.get("data") {
                let bytes = json_bytes(data, "data")?;
                let format = object.get("format").and_then(serde_json::Value::as_str).unwrap_or("");
                return i_slint_core::graphics::load_image_from_dynamic_data(&bytes, format)
                    .map(Into::into)
                    .map_err(|error| format!("Failed to decode image data: {error}"));
            }
            let width =
                object.get("width").and_then(serde_json::Value::as_u64).ok_or_else(|| {
                    "an image object needs a path, svg, data, or width/height/rgba".to_string()
                })? as u32;
            let height =
                object.get("height").and_then(serde_json::Value::as_u64).ok_or_else(|| {
                    "an image object needs width and height for rgba data".to_string()
                })? as u32;
            let rgba = object.get("rgba").ok_or_else(|| {
                "an image object needs a path, svg, data, or width/height/rgba".to_string()
            })?;
            let bytes = json_bytes(rgba, "rgba")?;
            let expected = width as usize * height as usize * 4;
            if bytes.len() != expected {
                return Err(format!(
                    "rgba image data has {} bytes, expected {expected} for {width}x{height}",
                    bytes.len()
                ));
            }
            let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&bytes, width, height);
            Ok(Image::from_rgba8(buffer).into())
        }
        _ => Err("an image must be a path string, null, or an object".into()),
    }
}

fn load_from_path(path: &str) -> Result<Value, String> {
    Image::load_from_path(Path::new(path))
        .map(Into::into)
        .map_err(|error| format!("Failed to load image from path: {path}: {error}"))
}

fn json_bytes(value: &serde_json::Value, field: &str) -> Result<Vec<u8>, String> {
    match value {
        serde_json::Value::String(text)
            if field == "svg" && text.as_bytes().first() == Some(&b'<') =>
        {
            Ok(text.as_bytes().to_vec())
        }
        serde_json::Value::String(text) => STANDARD
            .decode(text.as_bytes())
            .map_err(|error| format!("{field} is not base64: {error}")),
        _ => Err(format!("{field} must be a base64 string")),
    }
}
