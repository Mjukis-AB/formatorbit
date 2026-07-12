//! Type conversions between Rust CoreValue and Python objects.

use crate::types::{CoreValue, Interpretation, RichDisplayOption};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyModule, PyString};

/// Add type classes to the forb module.
pub fn add_types_to_module(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    // Add CoreValue class with static constructors
    py.run(
        c"
class CoreValue:
    def __init__(self, type_name, value):
        self._type = type_name
        self._value = value

    @staticmethod
    def Empty():
        return CoreValue(\"empty\", None)

    @staticmethod
    def Bytes(data):
        return CoreValue(\"bytes\", bytes(data) if not isinstance(data, bytes) else data)

    @staticmethod
    def String(value):
        return CoreValue(\"string\", str(value))

    @staticmethod
    def Int(value, original_bytes=None):
        return CoreValue(\"int\", {\"value\": int(value), \"original_bytes\": original_bytes})

    @staticmethod
    def Float(value):
        return CoreValue(\"float\", float(value))

    @staticmethod
    def Bool(value):
        return CoreValue(\"bool\", bool(value))

    @staticmethod
    def DateTime(value):
        return CoreValue(\"datetime\", value)

    @staticmethod
    def Json(data):
        return CoreValue(\"json\", data)

    @staticmethod
    def Currency(amount, code):
        return CoreValue(\"currency\", {\"amount\": float(amount), \"code\": str(code)})

    @staticmethod
    def Coordinates(lat, lon):
        return CoreValue(\"coordinates\", {\"lat\": float(lat), \"lon\": float(lon)})

    @staticmethod
    def Length(meters):
        return CoreValue(\"length\", float(meters))

    @staticmethod
    def Weight(grams):
        return CoreValue(\"weight\", float(grams))

    @staticmethod
    def Temperature(kelvin):
        return CoreValue(\"temperature\", float(kelvin))

    @property
    def type_name(self):
        return self._type

    @property
    def value(self):
        if self._type == \"int\":
            return self._value[\"value\"]
        return self._value

    def __repr__(self):
        return f\"CoreValue.{self._type.title()}({self._value!r})\"
",
        Some(&module.dict()),
        None,
    )?;

    // Add Interpretation class
    py.run(
        c"
class Interpretation:
    def __init__(self, value, confidence, description, rich_display=None):
        self.value = value
        self.confidence = float(confidence)
        self.description = str(description)
        self.rich_display = rich_display or []

    def __repr__(self):
        return f\"Interpretation(confidence={self.confidence}, description={self.description!r})\"
",
        Some(&module.dict()),
        None,
    )?;

    // Add RichDisplay class
    py.run(
        c"
class RichDisplay:
    def __init__(self, type_name, data):
        self._type = type_name
        self._data = data

    @staticmethod
    def KeyValue(pairs):
        return RichDisplay(\"key_value\", {\"pairs\": list(pairs)})

    @staticmethod
    def Table(headers, rows):
        return RichDisplay(\"table\", {\"headers\": list(headers), \"rows\": [list(r) for r in rows]})

    @staticmethod
    def Tree(root):
        return RichDisplay(\"tree\", {\"root\": root})

    @staticmethod
    def Color(r, g, b, a=255):
        return RichDisplay(\"color\", {\"r\": r, \"g\": g, \"b\": b, \"a\": a})

    @staticmethod
    def Code(language, content):
        return RichDisplay(\"code\", {\"language\": str(language), \"content\": str(content)})

    @staticmethod
    def Map(lat, lon, label=None):
        return RichDisplay(\"map\", {\"lat\": float(lat), \"lon\": float(lon), \"label\": label})

    @staticmethod
    def Duration(millis, human):
        return RichDisplay(\"duration\", {\"millis\": int(millis), \"human\": str(human)})

    @staticmethod
    def DateTime(epoch_millis, iso, relative):
        return RichDisplay(\"datetime\", {\"epoch_millis\": int(epoch_millis), \"iso\": str(iso), \"relative\": str(relative)})

    @staticmethod
    def DataSize(bytes_count, human):
        return RichDisplay(\"data_size\", {\"bytes\": int(bytes_count), \"human\": str(human)})

    @staticmethod
    def Markdown(content):
        return RichDisplay(\"markdown\", {\"content\": str(content)})

    @staticmethod
    def Progress(value, label=None):
        return RichDisplay(\"progress\", {\"value\": float(value), \"label\": label})

    @property
    def type_name(self):
        return self._type

    def __repr__(self):
        return f\"RichDisplay.{self._type}({self._data!r})\"

class TreeNode:
    def __init__(self, label, value=None, children=None):
        self.label = str(label)
        self.value = value
        self.children = children or []

    def __repr__(self):
        return f\"TreeNode({self.label!r}, {self.value!r}, {len(self.children)} children)\"
",
        Some(&module.dict()),
        None,
    )?;

    Ok(())
}

/// Convert a Python object to CoreValue.
pub fn py_to_core_value(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<CoreValue> {
    // Check if it's our CoreValue class
    if let Ok(type_name) = obj.getattr("_type") {
        let type_str: String = type_name.extract()?;
        let value = obj.getattr("_value")?;

        return match type_str.as_str() {
            "empty" => Ok(CoreValue::Empty),
            "bytes" => {
                let data: Vec<u8> = value.extract()?;
                Ok(CoreValue::Bytes(data))
            }
            "string" => {
                let s: String = value.extract()?;
                Ok(CoreValue::String(s))
            }
            "int" => {
                let dict = value.downcast::<PyDict>()?;
                let val: i128 = dict.get_item("value")?.unwrap().extract()?;
                let orig_bytes: Option<Vec<u8>> = dict
                    .get_item("original_bytes")?
                    .and_then(|v| v.extract().ok());
                Ok(CoreValue::Int {
                    value: val,
                    original_bytes: orig_bytes,
                })
            }
            "float" => {
                let f: f64 = value.extract()?;
                Ok(CoreValue::Float(f))
            }
            "bool" => {
                let b: bool = value.extract()?;
                Ok(CoreValue::Bool(b))
            }
            "json" => {
                let json_val = py_to_json(&value)?;
                Ok(CoreValue::Json(json_val))
            }
            "currency" => {
                let dict = value.downcast::<PyDict>()?;
                let amount: f64 = dict.get_item("amount")?.unwrap().extract()?;
                let code: String = dict.get_item("code")?.unwrap().extract()?;
                Ok(CoreValue::Currency { amount, code })
            }
            "coordinates" => {
                let dict = value.downcast::<PyDict>()?;
                let lat: f64 = dict.get_item("lat")?.unwrap().extract()?;
                let lon: f64 = dict.get_item("lon")?.unwrap().extract()?;
                Ok(CoreValue::Coordinates { lat, lon })
            }
            "length" => {
                let meters: f64 = value.extract()?;
                Ok(CoreValue::Length(meters))
            }
            "weight" => {
                let grams: f64 = value.extract()?;
                Ok(CoreValue::Weight(grams))
            }
            "temperature" => {
                let kelvin: f64 = value.extract()?;
                Ok(CoreValue::Temperature(kelvin))
            }
            _ => Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Unknown CoreValue type: {}",
                type_str
            ))),
        };
    }

    // Try to convert native Python types
    if obj.is_none() {
        return Ok(CoreValue::Empty);
    }
    if let Ok(b) = obj.downcast::<PyBytes>() {
        return Ok(CoreValue::Bytes(b.as_bytes().to_vec()));
    }
    if let Ok(b) = obj.extract::<bool>() {
        return Ok(CoreValue::Bool(b));
    }
    if let Ok(i) = obj.extract::<i128>() {
        return Ok(CoreValue::Int {
            value: i,
            original_bytes: None,
        });
    }
    if let Ok(f) = obj.extract::<f64>() {
        return Ok(CoreValue::Float(f));
    }
    if let Ok(s) = obj.extract::<String>() {
        return Ok(CoreValue::String(s));
    }

    Err(pyo3::exceptions::PyTypeError::new_err(
        "Cannot convert to CoreValue",
    ))
}

/// Convert a Python object to a JSON value.
fn py_to_json(obj: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    if obj.is_none() {
        return Ok(serde_json::Value::Null);
    }
    if let Ok(b) = obj.extract::<bool>() {
        return Ok(serde_json::Value::Bool(b));
    }
    if let Ok(i) = obj.extract::<i64>() {
        return Ok(serde_json::Value::Number(i.into()));
    }
    if let Ok(f) = obj.extract::<f64>() {
        if let Some(n) = serde_json::Number::from_f64(f) {
            return Ok(serde_json::Value::Number(n));
        }
    }
    if let Ok(s) = obj.extract::<String>() {
        return Ok(serde_json::Value::String(s));
    }
    if let Ok(list) = obj.downcast::<PyList>() {
        let arr: Result<Vec<_>, _> = list.iter().map(|item| py_to_json(&item)).collect();
        return Ok(serde_json::Value::Array(arr?));
    }
    if let Ok(dict) = obj.downcast::<PyDict>() {
        let mut map = serde_json::Map::new();
        for (k, v) in dict.iter() {
            let key: String = k.extract()?;
            let value = py_to_json(&v)?;
            map.insert(key, value);
        }
        return Ok(serde_json::Value::Object(map));
    }

    // Default to string representation
    let s = obj.str()?.extract::<String>()?;
    Ok(serde_json::Value::String(s))
}

/// Convert CoreValue to a Python object.
#[allow(dead_code)]
pub fn core_value_to_py(py: Python<'_>, value: &CoreValue) -> PyResult<PyObject> {
    match value {
        CoreValue::Empty => Ok(py.None()),
        CoreValue::Bytes(b) => Ok(PyBytes::new(py, b).into()),
        CoreValue::String(s) => Ok(PyString::new(py, s).into()),
        CoreValue::Int { value, .. } => Ok(value.into_pyobject(py)?.into_any().unbind()),
        CoreValue::Float(f) => Ok(f.into_pyobject(py)?.into_any().unbind()),
        CoreValue::Bool(b) => Ok((*b).into_pyobject(py)?.to_owned().into_any().unbind()),
        CoreValue::DateTime(dt) => {
            // Convert to ISO string for simplicity
            Ok(PyString::new(py, &dt.to_rfc3339()).into())
        }
        CoreValue::Json(v) => json_to_py(py, v),
        CoreValue::Currency { amount, code } => {
            let dict = PyDict::new(py);
            dict.set_item("amount", amount)?;
            dict.set_item("code", code)?;
            Ok(dict.into())
        }
        CoreValue::Coordinates { lat, lon } => {
            let dict = PyDict::new(py);
            dict.set_item("lat", lat)?;
            dict.set_item("lon", lon)?;
            Ok(dict.into())
        }
        CoreValue::Length(m) => Ok(m.into_pyobject(py)?.into_any().unbind()),
        CoreValue::Weight(g) => Ok(g.into_pyobject(py)?.into_any().unbind()),
        CoreValue::Volume(ml) => Ok(ml.into_pyobject(py)?.into_any().unbind()),
        CoreValue::Speed(mps) => Ok(mps.into_pyobject(py)?.into_any().unbind()),
        CoreValue::Pressure(pa) => Ok(pa.into_pyobject(py)?.into_any().unbind()),
        CoreValue::Energy(j) => Ok(j.into_pyobject(py)?.into_any().unbind()),
        CoreValue::Angle(deg) => Ok(deg.into_pyobject(py)?.into_any().unbind()),
        CoreValue::Area(sqm) => Ok(sqm.into_pyobject(py)?.into_any().unbind()),
        CoreValue::Temperature(k) => Ok(k.into_pyobject(py)?.into_any().unbind()),
        CoreValue::Protobuf(_) => {
            // Convert to string representation for now
            Ok(PyString::new(py, "[protobuf data]").into())
        }
    }
}

/// Convert a JSON value to a Python object.
fn json_to_py(py: Python<'_>, value: &serde_json::Value) -> PyResult<PyObject> {
    match value {
        serde_json::Value::Null => Ok(py.None()),
        serde_json::Value::Bool(b) => Ok((*b).into_pyobject(py)?.to_owned().into_any().unbind()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i.into_pyobject(py)?.into_any().unbind())
            } else if let Some(f) = n.as_f64() {
                Ok(f.into_pyobject(py)?.into_any().unbind())
            } else {
                Ok(py.None())
            }
        }
        serde_json::Value::String(s) => Ok(PyString::new(py, s).into()),
        serde_json::Value::Array(arr) => {
            let list = PyList::empty(py);
            for item in arr {
                list.append(json_to_py(py, item)?)?;
            }
            Ok(list.into())
        }
        serde_json::Value::Object(map) => {
            let dict = PyDict::new(py);
            for (k, v) in map {
                dict.set_item(k, json_to_py(py, v)?)?;
            }
            Ok(dict.into())
        }
    }
}

/// Convert a Python Interpretation object to Rust.
pub fn py_to_interpretation(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<Interpretation> {
    let value_obj = obj.getattr("value")?;
    let value = py_to_core_value(py, &value_obj)?;

    let confidence: f32 = obj.getattr("confidence")?.extract()?;
    let description: String = obj.getattr("description")?.extract()?;

    let rich_display = py_to_rich_display_options(py, obj)?;

    Ok(Interpretation {
        value,
        source_format: String::new(), // Set by the decoder wrapper
        confidence,
        description,
        rich_display,
    })
}

/// Parse the optional `rich_display` attribute of a Python Interpretation into
/// a `Vec<RichDisplayOption>`.
///
/// The Python side exposes `Interpretation.rich_display` as a list of
/// `RichDisplay` objects (from `forb.RichDisplay.KeyValue(...)`, `.Table(...)`,
/// etc.). Each is wrapped in a `RichDisplayOption` with no alternatives — the
/// same one-preferred shape the built-in Rust formats use.
///
/// A `RichDisplay` variant this build does not recognise is skipped rather than
/// raising, so a newer plugin using an unknown variant degrades gracefully to
/// its plain-text `description` instead of failing to load.
pub fn py_to_rich_display_options(
    py: Python<'_>,
    interpretation: &Bound<'_, PyAny>,
) -> PyResult<Vec<RichDisplayOption>> {
    // Missing attribute or None -> no rich display (older plugins / defaults).
    let Ok(rd_attr) = interpretation.getattr("rich_display") else {
        return Ok(Vec::new());
    };
    if rd_attr.is_none() {
        return Ok(Vec::new());
    }

    let list = rd_attr.downcast::<PyList>()?;
    let mut options = Vec::with_capacity(list.len());
    for item in list.iter() {
        // Reuse the visualizer's converter. Unknown variants (or anything that
        // isn't a well-formed RichDisplay) are skipped, not fatal.
        match super::visualizer::py_to_rich_display(py, &item) {
            Ok(rd) => options.push(RichDisplayOption::new(rd)),
            Err(e) => {
                tracing::warn!(error = %e, "skipping unrecognized plugin rich_display entry");
            }
        }
    }
    Ok(options)
}

/// Convert a list of Python Interpretation objects to Rust.
pub fn py_to_interpretations(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
) -> PyResult<Vec<Interpretation>> {
    let list = obj.downcast::<PyList>()?;
    let mut results = Vec::with_capacity(list.len());

    for item in list.iter() {
        results.push(py_to_interpretation(py, &item)?);
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use crate::plugin::{Plugin, PluginError, PythonRuntime};
    use crate::types::RichDisplay;
    use std::io::Write;

    /// Write `source` to a temp `.py` file and load it, returning its plugins.
    ///
    /// Returns `None` (test skipped) if Python is not available in this
    /// environment — plugins are an optional runtime capability, so CI without
    /// a discoverable libpython should not fail the suite.
    fn load_inline_plugin(source: &str) -> Option<Vec<Plugin>> {
        let runtime = match PythonRuntime::init() {
            Ok(rt) => rt,
            Err(PluginError::RuntimeInit(msg)) => {
                eprintln!("skipping: Python runtime unavailable ({msg})");
                return None;
            }
            Err(e) => panic!("unexpected plugin init error: {e:?}"),
        };

        let mut file = tempfile::Builder::new()
            .suffix(".py")
            .tempfile()
            .expect("create temp plugin file");
        file.write_all(source.as_bytes()).expect("write plugin");
        file.flush().expect("flush plugin");

        let plugins = runtime
            .load_plugin(file.path())
            .expect("plugin should load");
        Some(plugins)
    }

    /// A decoder plugin that attaches a KeyValue rich display should round-trip
    /// that rich display through the Rust bridge (previously dropped by a TODO).
    #[test]
    #[serial_test::serial]
    fn key_value_rich_display_round_trips() {
        let source = r#"
import forb

__forb_plugin__ = {
    "name": "rd-test",
    "version": "0.0.1",
}

@forb.decoder(id="rd-kv", name="RD KeyValue")
def decode(text):
    return [forb.Interpretation(
        value=forb.CoreValue.String(text),
        confidence=1.0,
        description="fallback text",
        rich_display=[forb.RichDisplay.KeyValue([("k1", "v1"), ("k2", "v2")])],
    )]
"#;

        let Some(plugins) = load_inline_plugin(source) else {
            return; // Python unavailable; skip.
        };

        let decoder = plugins
            .iter()
            .find_map(|p| match p {
                Plugin::Decoder(d) => Some(d),
                _ => None,
            })
            .expect("plugin defines a decoder");

        let interps = decoder.parse("anything");
        assert_eq!(interps.len(), 1, "decoder should yield one interpretation");
        let interp = &interps[0];

        // Plain-text fallback is preserved.
        assert_eq!(interp.description, "fallback text");

        // Rich display was parsed (not the old hardcoded empty vec).
        assert_eq!(
            interp.rich_display.len(),
            1,
            "rich_display should be bridged from Python"
        );
        match &interp.rich_display[0].preferred {
            RichDisplay::KeyValue { pairs } => {
                assert_eq!(
                    pairs,
                    &vec![
                        ("k1".to_string(), "v1".to_string()),
                        ("k2".to_string(), "v2".to_string()),
                    ]
                );
            }
            other => panic!("expected KeyValue rich display, got {other:?}"),
        }
    }

    /// Multiple rich displays (KeyValue + Color + Code) all survive the bridge.
    #[test]
    #[serial_test::serial]
    fn multiple_rich_displays_round_trip() {
        let source = r#"
import forb

__forb_plugin__ = {"name": "rd-multi", "version": "0.0.1"}

@forb.decoder(id="rd-multi", name="RD Multi")
def decode(text):
    return [forb.Interpretation(
        value=forb.CoreValue.String(text),
        confidence=1.0,
        description="multi",
        rich_display=[
            forb.RichDisplay.KeyValue([("a", "1")]),
            forb.RichDisplay.Color(255, 128, 0, 255),
            forb.RichDisplay.Code("json", "{\"x\": 1}"),
        ],
    )]
"#;

        let Some(plugins) = load_inline_plugin(source) else {
            return;
        };
        let decoder = plugins
            .iter()
            .find_map(|p| match p {
                Plugin::Decoder(d) => Some(d),
                _ => None,
            })
            .expect("plugin defines a decoder");

        let interps = decoder.parse("x");
        let rd = &interps[0].rich_display;
        assert_eq!(rd.len(), 3, "all three rich displays should survive");
        assert!(matches!(rd[0].preferred, RichDisplay::KeyValue { .. }));
        assert!(matches!(
            rd[1].preferred,
            RichDisplay::Color {
                r: 255,
                g: 128,
                b: 0,
                a: 255
            }
        ));
        assert!(matches!(rd[2].preferred, RichDisplay::Code { .. }));
    }

    /// A rich_display entry with an unknown variant type is skipped gracefully
    /// (the interpretation still loads, keeping only the recognised entries)
    /// rather than dropping the whole interpretation.
    #[test]
    #[serial_test::serial]
    fn unknown_rich_display_variant_is_skipped() {
        let source = r#"
import forb

__forb_plugin__ = {"name": "rd-unknown", "version": "0.0.1"}

@forb.decoder(id="rd-unknown", name="RD Unknown")
def decode(text):
    interp = forb.Interpretation(
        value=forb.CoreValue.String(text),
        confidence=1.0,
        description="unknown-variant",
    )
    # A well-formed KeyValue plus a bogus variant the Rust side doesn't know.
    interp.rich_display = [
        forb.RichDisplay.KeyValue([("ok", "yes")]),
        forb.RichDisplay("totally_made_up", {"foo": "bar"}),
    ]
    return [interp]
"#;

        let Some(plugins) = load_inline_plugin(source) else {
            return;
        };
        let decoder = plugins
            .iter()
            .find_map(|p| match p {
                Plugin::Decoder(d) => Some(d),
                _ => None,
            })
            .expect("plugin defines a decoder");

        let interps = decoder.parse("x");
        assert_eq!(interps.len(), 1, "interpretation must still load");
        // Only the recognised KeyValue survives; the unknown variant is dropped.
        assert_eq!(interps[0].rich_display.len(), 1);
        assert!(matches!(
            interps[0].rich_display[0].preferred,
            RichDisplay::KeyValue { .. }
        ));
    }
}
