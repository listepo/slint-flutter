/// An image that can be assigned to a Slint `image` property.
///
/// `@image-url` inside `.slint` still works. This type is how Dart passes one
/// in: a path on disk, encoded PNG/JPEG/SVG bytes, SVG source, or raw RGBA
/// pixels — the same shapes Python's `Image` and Node's `ImageData` cover.
library;

import 'dart:convert';
import 'dart:typed_data';

/// A Slint `image` value.
///
/// Construct with [SlintImage.fromPath], [SlintImage.fromEncoded],
/// [SlintImage.fromSvg], or [SlintImage.fromRgba], then assign it to a
/// property. Reading an image property returns one of these, via
/// [SlintImage.fromSlint].
class SlintImage {
  /// Load from a file. PNG, JPEG and SVG are supported.
  SlintImage.fromPath(String path) : this._(path: path);

  /// Decode PNG, JPEG or SVG bytes, the way a Flutter asset arrives.
  SlintImage.fromEncoded(Uint8List bytes, {String format = ''})
      : this._(encoded: Uint8List.fromList(bytes), format: format);

  /// Parse SVG source.
  SlintImage.fromSvg(String source) : this._(svg: source);

  /// Wrap already-decoded RGBA pixels, 4 bytes per pixel, unpremultiplied,
  /// row-major.
  SlintImage.fromRgba(int width, int height, Uint8List pixels)
      : this._(
          width: width,
          height: height,
          rgba: _copyRgba(width, height, pixels),
        );

  /// Rebuild from the JSON the runtime returns for an `image` property.
  factory SlintImage.fromSlint(Object? value) {
    if (value == null) return const SlintImage._();
    if (value is String) return SlintImage.fromPath(value);
    if (value is Map) {
      final map = value.cast<String, Object?>();
      final path = map['path'] as String?;
      final svg = map['svg'] as String?;
      final data = map['data'] as String?;
      final rgba = map['rgba'] as String?;
      final width = (map['width'] as num?)?.toInt() ?? 0;
      final height = (map['height'] as num?)?.toInt() ?? 0;
      final format = map['format'] as String? ?? '';
      if (path != null) {
        return SlintImage._(path: path, width: width, height: height);
      }
      if (svg != null) {
        return SlintImage._(svg: svg, width: width, height: height);
      }
      if (data != null) {
        return SlintImage._(
          encoded: base64Decode(data),
          format: format,
          width: width,
          height: height,
        );
      }
      if (rgba != null) {
        return SlintImage._(
          width: width,
          height: height,
          rgba: base64Decode(rgba),
        );
      }
    }
    throw ArgumentError.value(value, 'value', 'not a Slint image');
  }

  const SlintImage._({
    this.path,
    this.svg,
    this.encoded,
    this.format = '',
    this.rgba,
    this.width = 0,
    this.height = 0,
  });

  /// Filesystem path, when the image was loaded from disk.
  final String? path;

  /// SVG source, when constructed with [SlintImage.fromSvg].
  final String? svg;

  /// Encoded file bytes, when constructed with [SlintImage.fromEncoded].
  final Uint8List? encoded;

  /// Hint for [encoded], such as `png` or `svg`. Empty lets Slint sniff.
  final String format;

  /// Unpremultiplied RGBA pixels, when known.
  final Uint8List? rgba;

  /// Pixel width. Zero until the runtime has decoded the image, except for
  /// [SlintImage.fromRgba], which knows it immediately.
  final int width;

  /// Pixel height. See [width].
  final int height;

  static Uint8List _copyRgba(int width, int height, Uint8List pixels) {
    // A negative dimension makes the expected-length check below meaningless
    // (it can never match), so catch it with a clear message first.
    assert(width >= 0 && height >= 0, 'image dimensions cannot be negative');
    final expected = width * height * 4;
    if (pixels.length != expected) {
      throw ArgumentError.value(
        pixels.length,
        'pixels.length',
        'expected $expected bytes for ${width}x$height RGBA',
      );
    }
    return Uint8List.fromList(pixels);
  }

  /// JSON the runtime understands. [jsonEncode] calls this.
  Object? toJson() {
    if (path != null) return path;
    if (svg != null) return {'svg': svg};
    if (encoded != null) {
      return {
        'data': base64Encode(encoded!),
        if (format.isNotEmpty) 'format': format,
      };
    }
    if (rgba != null) {
      return {
        'width': width,
        'height': height,
        'rgba': base64Encode(rgba!),
      };
    }
    return null;
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    return other is SlintImage &&
        other.path == path &&
        other.svg == svg &&
        other.format == format &&
        other.width == width &&
        other.height == height &&
        _bytesEqual(other.encoded, encoded) &&
        _bytesEqual(other.rgba, rgba);
  }

  @override
  int get hashCode => Object.hash(
        path,
        svg,
        format,
        width,
        height,
        encoded == null ? null : Object.hashAll(encoded!),
        rgba == null ? null : Object.hashAll(rgba!),
      );
}

bool _bytesEqual(Uint8List? left, Uint8List? right) {
  if (identical(left, right)) return true;
  if (left == null || right == null || left.length != right.length) {
    return false;
  }
  for (var i = 0; i < left.length; i++) {
    if (left[i] != right[i]) return false;
  }
  return true;
}
