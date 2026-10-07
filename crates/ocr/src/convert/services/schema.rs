//! Builds the JSON schemas that force the shape of a model reply, using only the forms the
//! `claude` command accepts.

use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde_json::{Map, Value};

/// The model must decide every field, so each is required and an optional value is a field that
/// may be null. There is no `oneOf`, no `$ref` and no `format`.
pub(super) fn reply_schema<T: JsonSchema>() -> Value {
    let mut settings = SchemaSettings::draft07();
    settings.inline_subschemas = true;
    let mut schema = settings
        .into_generator()
        .into_root_schema_for::<T>()
        .to_value();
    tidy_schema(&mut schema);
    schema
}

fn tidy_schema(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let mut tidied = Map::new();
            // Rebuilt by iteration, because removing a key from the middle of an ordered map
            // would move the last key into the gap.
            for (key, mut child) in std::mem::take(object) {
                match key.as_str() {
                    "format" => continue,
                    "oneOf" => {
                        tidy_schema(&mut child);
                        tidied.insert("anyOf".to_owned(), child)
                    }
                    "properties" => {
                        if let Value::Object(properties) = &mut child {
                            let mut reordered = kind_first(std::mem::take(properties));
                            // Each field's schema is tidied, but not the map of fields itself:
                            // a field may be named like a schema keyword.
                            reordered.values_mut().for_each(tidy_schema);
                            tidied.insert(
                                "required".to_owned(),
                                Value::Array(
                                    reordered.keys().cloned().map(Value::String).collect(),
                                ),
                            );
                            child = Value::Object(reordered);
                        }
                        tidied.insert(key, child)
                    }
                    // Set from the properties above, so the model must decide every field.
                    "required" => continue,
                    _ => {
                        tidy_schema(&mut child);
                        tidied.insert(key, child)
                    }
                };
            }
            *object = tidied;
        }
        Value::Array(items) => items.iter_mut().for_each(tidy_schema),
        _ => {}
    }
}

fn kind_first(properties: Map<String, Value>) -> Map<String, Value> {
    let mut ordered = Map::new();
    let mut rest = Vec::new();
    for (name, schema) in properties {
        if name == "kind" {
            ordered.insert(name, schema);
        } else {
            rest.push((name, schema));
        }
    }
    ordered.extend(rest);
    ordered
}
