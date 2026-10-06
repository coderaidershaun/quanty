//! Builds the JSON schemas that force the shape of a model reply, using only the forms the
//! `claude` command accepts.

use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde_json::{Map, Value};

/// The schema `claude` is given for a reply of type `T`.
///
/// The model must decide every field, so each is required and an optional value is a field that
/// may be null. Only forms the `claude` command accepts are used: no `oneOf`, no `$ref`, no
/// `format`.
pub(crate) fn reply_schema<T: JsonSchema>() -> Value {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcribe::{CopiedPage, TranscribedPage};

    fn walk(value: &Value, visit: &mut impl FnMut(&Map<String, Value>)) {
        match value {
            Value::Object(object) => {
                visit(object);
                object.values().for_each(|child| walk(child, visit));
            }
            Value::Array(items) => items.iter().for_each(|child| walk(child, visit)),
            _ => {}
        }
    }

    fn keys(object: &Value) -> Vec<&str> {
        object["properties"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect()
    }

    fn position(names: &[&str], name: &str) -> usize {
        names.iter().position(|found| *found == name).unwrap()
    }

    #[test]
    fn schemas_use_only_accepted_forms_and_copy_has_no_math_kinds() {
        let transcription = reply_schema::<TranscribedPage>();
        let copy = reply_schema::<CopiedPage>();

        for schema in [&transcription, &copy] {
            assert_eq!(schema["$schema"], "http://json-schema.org/draft-07/schema#");
            walk(schema, &mut |object| {
                for refused in ["oneOf", "$ref", "format", "definitions"] {
                    assert!(!object.contains_key(refused), "{refused} in {object:?}");
                }
                if let Some(Value::Object(properties)) = object.get("properties") {
                    assert_eq!(object["additionalProperties"], false, "{object:?}");
                    let required: Vec<&str> = object["required"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|name| name.as_str().unwrap())
                        .collect();
                    let declared: Vec<&str> = properties.keys().map(String::as_str).collect();
                    assert_eq!(required, declared);
                }
            });
        }

        let transcription_pieces = &transcription["properties"]["pieces"]["items"]["anyOf"];
        let copy_pieces = &copy["properties"]["pieces"]["items"]["anyOf"];
        let kinds = |pieces: &Value| -> Vec<String> {
            pieces
                .as_array()
                .unwrap()
                .iter()
                .map(|piece| {
                    assert_eq!(keys(piece)[0], "kind");
                    piece["properties"]["kind"]["const"]
                        .as_str()
                        .unwrap()
                        .to_owned()
                })
                .collect()
        };
        assert_eq!(
            kinds(transcription_pieces),
            ["heading", "text", "formula", "figure", "table", "footnote"]
        );
        assert_eq!(kinds(copy_pieces), ["heading", "text", "footnote"]);

        let page_fields = keys(&transcription);
        assert!(position(&page_fields, "pieces") < position(&page_fields, "discusses"));
        let figure = &transcription_pieces[3];
        let figure_fields = keys(figure);
        assert!(position(&figure_fields, "printed-text") < position(&figure_fields, "explanation"));
        assert!(position(&figure_fields, "printed-text") < position(&figure_fields, "bounds"));
        assert!(position(&figure_fields, "bounds") < position(&figure_fields, "explanation"));
        assert_eq!(
            keys(&figure["properties"]["bounds"]),
            ["left", "top", "right", "bottom"]
        );
        assert_eq!(keys(&copy)[0], "needs-stronger-model");
    }
}
