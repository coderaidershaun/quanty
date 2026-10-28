//! The short schemas of a tool's arguments and of its result, cut down from the full schemas to
//! what a model reads.

use std::sync::Arc;

use rmcp::handler::server::common::schema_for_type;
use rmcp::handler::server::tool::schema_for_output;
use rmcp::model::JsonObject;
use schemars::JsonSchema;
use serde_json::{Map, Value, json};

/// How deep `$ref`s are followed before a schema is given up as `{}`, so that a type that holds
/// itself cannot loop.
const MAX_REF_DEPTH: usize = 8;

/// The schema of the result of a tool whose result is `T`, cut down to what a model needs to know
/// which fields to expect: the object at the root, each field with its JSON type, and the
/// required fields. The fields of every part that is flattened into the result, and of every
/// choice of results, are fields of the root. Nothing nests: a field that holds an object or a
/// list says `object` or `array` and no more.
pub(crate) fn output_schema<T: JsonSchema + 'static>() -> Arc<JsonObject> {
    let full = schema_for_output::<T>();
    let defs = defs_of(&full);
    let fields = Fields::of(&Value::Object(full.as_ref().clone()), &defs);
    let mut lean = Map::new();
    lean.insert("type".into(), "object".into());
    lean.insert("properties".into(), Value::Object(fields.properties));
    if !fields.required.is_empty() {
        let required = fields.required.into_iter().map(Value::String).collect();
        lean.insert("required".into(), Value::Array(required));
    }
    Arc::new(lean)
}

/// The schema of the arguments of a tool whose arguments are `T`, cut down to what a model
/// reads: the types, the enum values, the required names and the short note on each argument.
/// An optional argument keeps `null` in its types, because a host validates arguments against
/// this schema and the docs promise that an optional argument can be set to `null`.
pub(crate) fn input_schema<T: JsonSchema + 'static>() -> Arc<JsonObject> {
    let full = schema_for_type::<T>();
    let defs = defs_of(&full);
    let mut lean = Map::new();
    for (key, value) in full.iter() {
        match key.as_str() {
            "type" | "required" => {
                lean.insert(key.clone(), value.clone());
            }
            "properties" => {
                lean.insert(key.clone(), lean_arguments(value, &defs));
            }
            _ => {}
        }
    }
    Arc::new(lean)
}

fn defs_of(schema: &JsonObject) -> Map<String, Value> {
    schema
        .get("$defs")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

/// The schema that a `{"$ref": "#/$defs/Name"}` points at.
fn referenced<'a>(schema: &'a Value, defs: &'a Map<String, Value>) -> Option<&'a Value> {
    let name = schema.get("$ref")?.as_str()?.strip_prefix("#/$defs/")?;
    defs.get(name)
}

/// The fields of a result, collected from the root and from every part that adds to it.
#[derive(Default)]
struct Fields {
    properties: Map<String, Value>,
    required: Vec<String>,
}

impl Fields {
    fn of(schema: &Value, defs: &Map<String, Value>) -> Fields {
        let mut fields = Fields::default();
        fields.gather(schema, defs, true, 0);
        fields
    }

    /// Adds the properties of `schema` and of its `allOf`, `anyOf` and `oneOf` parts. The required
    /// names of a choice are not required of the result, because the result is only one choice.
    fn gather(&mut self, schema: &Value, defs: &Map<String, Value>, required: bool, depth: usize) {
        if depth > MAX_REF_DEPTH {
            return;
        }
        if let Some(target) = referenced(schema, defs) {
            self.gather(target, defs, required, depth + 1);
            return;
        }
        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            for (name, property) in properties {
                self.add(name, field_type(property, defs, 0));
            }
        }
        if required && let Some(names) = schema.get("required").and_then(Value::as_array) {
            for name in names.iter().filter_map(Value::as_str) {
                if !self.required.iter().any(|have| have == name) {
                    self.required.push(name.to_owned());
                }
            }
        }
        for (key, is_required) in [("allOf", required), ("anyOf", false), ("oneOf", false)] {
            let parts = schema.get(key).and_then(Value::as_array);
            for part in parts.into_iter().flatten() {
                self.gather(part, defs, is_required, depth + 1);
            }
        }
    }

    /// Adds a field. A field that two choices both have takes the types of both.
    fn add(&mut self, name: &str, types: Vec<String>) {
        let known = self.properties.get(name).map(types_of).unwrap_or_default();
        let both = union(known, types);
        self.properties.insert(name.to_owned(), type_schema(both));
    }
}

/// The JSON types a field can have, with `null` when the field can be null. A field with no
/// known type has none.
fn field_type(schema: &Value, defs: &Map<String, Value>, depth: usize) -> Vec<String> {
    if depth > MAX_REF_DEPTH {
        return Vec::new();
    }
    if let Some(target) = referenced(schema, defs) {
        return field_type(target, defs, depth + 1);
    }
    let mut types = types_of(schema.get("type").unwrap_or(&Value::Null));
    if types.is_empty() && (schema.get("enum").is_some() || schema.get("const").is_some()) {
        types.push("string".to_owned());
    }
    for key in ["anyOf", "oneOf", "allOf"] {
        let parts = schema.get(key).and_then(Value::as_array);
        for part in parts.into_iter().flatten() {
            types = union(types, field_type(part, defs, depth + 1));
        }
    }
    if types.is_empty() && schema.get("properties").is_some() {
        types.push("object".to_owned());
    }
    types
}

fn types_of(schema: &Value) -> Vec<String> {
    match schema {
        Value::String(name) => vec![name.clone()],
        Value::Array(names) => names
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        Value::Object(object) => types_of(object.get("type").unwrap_or(&Value::Null)),
        _ => Vec::new(),
    }
}

fn union(mut first: Vec<String>, second: Vec<String>) -> Vec<String> {
    for name in second {
        if !first.contains(&name) {
            first.push(name);
        }
    }
    first
}

/// `{"type": "string"}` for one type, `{"type": ["string", "null"]}` for several, `{}` for none.
fn type_schema(types: Vec<String>) -> Value {
    match types.as_slice() {
        [] => json!({}),
        [one] => json!({ "type": one }),
        _ => json!({ "type": types }),
    }
}

/// The key of the short note that each argument keeps.
const NOTE: &str = "description";

/// The properties of the arguments: each one cut down, and only its own note kept.
fn lean_arguments(properties: &Value, defs: &Map<String, Value>) -> Value {
    let Some(properties) = properties.as_object() else {
        return properties.clone();
    };
    let lean = properties.iter().map(|(name, property)| {
        let mut argument = lean_argument(property, defs, 0);
        if let (Some(note), Some(object)) = (property.get(NOTE), argument.as_object_mut()) {
            object.insert(NOTE.into(), note.clone());
        }
        (name.clone(), argument)
    });
    Value::Object(lean.collect())
}

/// Keys that say nothing a model or a validator of arguments needs.
const NOISE: [&str; 9] = [
    "$schema", "$defs", "title", "format", "minimum", "maximum", "default", "examples", NOTE,
];

fn lean_argument(schema: &Value, defs: &Map<String, Value>, depth: usize) -> Value {
    let Some(object) = schema.as_object() else {
        return schema.clone();
    };
    if depth > MAX_REF_DEPTH {
        return json!({});
    }
    if let Some(target) = referenced(schema, defs) {
        return lean_argument(target, defs, depth + 1);
    }
    let mut lean = Map::new();
    for (key, value) in object {
        if NOISE.contains(&key.as_str()) {
            continue;
        }
        let kept = match key.as_str() {
            "properties" => lean_each(value, defs, depth),
            "anyOf" | "oneOf" | "allOf" => lean_all(value, defs, depth),
            "items" | "additionalProperties" => lean_argument(value, defs, depth + 1),
            _ => value.clone(),
        };
        lean.insert(key.clone(), kept);
    }
    let lean = enum_of_constants(lean);
    Value::Object(without_null_choice(lean))
}

fn lean_each(properties: &Value, defs: &Map<String, Value>, depth: usize) -> Value {
    let Some(properties) = properties.as_object() else {
        return properties.clone();
    };
    let each = properties
        .iter()
        .map(|(name, property)| (name.clone(), lean_argument(property, defs, depth + 1)));
    Value::Object(each.collect())
}

fn lean_all(choices: &Value, defs: &Map<String, Value>, depth: usize) -> Value {
    let Some(choices) = choices.as_array() else {
        return choices.clone();
    };
    let lean = choices
        .iter()
        .map(|choice| lean_argument(choice, defs, depth + 1));
    Value::Array(lean.collect())
}

/// A choice between constants becomes one `enum`.
fn enum_of_constants(mut schema: Map<String, Value>) -> Map<String, Value> {
    for key in ["oneOf", "anyOf"] {
        let Some(choices) = schema.get(key).and_then(Value::as_array) else {
            continue;
        };
        let constants: Option<Vec<Value>> = choices
            .iter()
            .map(|choice| choice.get("const").cloned())
            .collect();
        if let Some(constants) = constants {
            schema.remove(key);
            schema.insert("type".into(), "string".into());
            schema.insert("enum".into(), Value::Array(constants));
        }
    }
    schema
}

/// `anyOf` of one schema that has a type and `null` becomes that schema with `null` among its
/// types, and among its values if it has an `enum`.
fn without_null_choice(schema: Map<String, Value>) -> Map<String, Value> {
    let Some(choices) = schema.get("anyOf").and_then(Value::as_array) else {
        return schema;
    };
    let is_null = |choice: &Value| choice.get("type").and_then(Value::as_str) == Some("null");
    let [one, null] = choices.as_slice() else {
        return schema;
    };
    let (Some(typed), true) = (one.as_object(), is_null(null)) else {
        return schema;
    };
    let types = types_of(&Value::Object(typed.clone()));
    if types.len() != 1 || schema.len() != 1 {
        return schema;
    }
    let mut merged = typed.clone();
    merged.insert("type".into(), json!([types[0], "null"]));
    if let Some(Value::Array(values)) = merged.get_mut("enum") {
        values.push(Value::Null);
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_optional_argument_keeps_null_and_a_choice_of_names_becomes_an_enum() {
        let schema = input_schema::<crate::server::catalog::CatalogArgs>();

        let detail = &schema["properties"]["detail"];
        assert_eq!(detail["type"], json!(["string", "null"]));
        assert_eq!(detail["enum"], json!(["summaries", "names", null]));
    }
}
