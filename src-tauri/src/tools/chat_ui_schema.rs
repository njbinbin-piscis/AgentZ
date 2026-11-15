//! Shared JSON-schema fragments for `chat_ui` / `chat_ui_patch` tool definitions,
//! and the repair pass applied to a definition before it is shown.

use std::collections::{HashMap, HashSet};

use serde_json::{json, Map, Value};

// ── Option repair ──────────────────────────────────────────────────────────
//
// Models do not always write options as `{ "value": "<string>", "label": "<string>" }`:
// values arrive as numbers, under another key (`id`, `key`, `name`), or are left
// out so only a label exists; options can be bare strings; two options can share
// a value. The card compares the selected value against `option.value` as a
// string, so any of those shapes showed a choice that could never be selected,
// and a required field then could not be submitted.
//
// The definition is repaired here before it is emitted, so what is stored and
// replayed is already well-formed. The frontend applies the same rules
// (`src/components/chat/interactiveUi/normalize.ts`) for cards restored from
// older history; keep the two in step.

const SINGLE_CHOICE_TYPES: &[&str] = &["radio", "select", "link_list", "project_picker"];
const MULTI_CHOICE_TYPES: &[&str] = &["checkbox", "tags", "koi_picker"];
/// Fields whose only possible answers are their own options.
const OPTIONS_REQUIRED_TYPES: &[&str] = &["radio", "select", "checkbox"];
const VALUE_KEYS: &[&str] = &["value", "id", "key", "name"];
const LABEL_KEYS: &[&str] = &["label", "text", "title", "name"];

/// Text of a scalar the way the card would show it; `None` for blank strings,
/// null, and non-scalars.
fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => (!s.trim().is_empty()).then(|| s.clone()),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(i.to_string())
            } else if let Some(u) = n.as_u64() {
                Some(u.to_string())
            } else {
                let f = n.as_f64()?;
                if !f.is_finite() {
                    None
                } else if f.fract() == 0.0 && f.abs() < 1e15 {
                    // `2.0` is the choice "2", as it is in the browser.
                    Some(format!("{}", f as i64))
                } else {
                    Some(f.to_string())
                }
            }
        }
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn first_text(source: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| source.get(*key).and_then(scalar_text))
}

/// Options as `{ value, label, .. }` objects with unique non-empty string values.
pub fn normalize_options(raw: &Value) -> Vec<Value> {
    let entries: Vec<Value> = match raw {
        Value::Array(items) => items.clone(),
        // `{ "a": "Option A", "b": "Option B" }`
        Value::Object(map) => map
            .iter()
            .map(|(value, label)| json!({ "value": value, "label": label }))
            .collect(),
        _ => Vec::new(),
    };
    let mut out = Vec::with_capacity(entries.len());
    let mut used: HashSet<String> = HashSet::new();
    for (index, entry) in entries.iter().enumerate() {
        let (value, label, mut rest) = match entry {
            Value::Object(map) => (
                first_text(map, VALUE_KEYS),
                first_text(map, LABEL_KEYS)
                    .or_else(|| map.get("description").and_then(scalar_text)),
                map.clone(),
            ),
            other => {
                let text = scalar_text(other);
                (text.clone(), text, Map::new())
            }
        };
        // Nothing to show and nothing to submit.
        if value.is_none() && label.is_none() {
            continue;
        }
        let mut resolved = value
            .or_else(|| label.clone())
            .unwrap_or_else(|| format!("opt-{}", index + 1));
        if used.contains(&resolved) {
            let mut n = 2;
            while used.contains(&format!("{resolved}-{n}")) {
                n += 1;
            }
            resolved = format!("{resolved}-{n}");
        }
        used.insert(resolved.clone());
        let label = label.unwrap_or_else(|| resolved.clone());
        rest.insert("value".into(), Value::String(resolved));
        rest.insert("label".into(), Value::String(label));
        out.push(Value::Object(rest));
    }
    out
}

fn option_text<'a>(option: &'a Value, key: &str) -> Option<&'a str> {
    option.get(key).and_then(Value::as_str)
}

/// A default written as a label or a number, mapped onto an option value.
fn resolve_choice(raw: &Value, options: &[Value]) -> Value {
    let Some(text) = scalar_text(raw) else {
        return raw.clone();
    };
    if options
        .iter()
        .any(|opt| option_text(opt, "value") == Some(text.as_str()))
    {
        return Value::String(text);
    }
    match options
        .iter()
        .find(|opt| option_text(opt, "label") == Some(text.as_str()))
        .and_then(|opt| option_text(opt, "value"))
    {
        Some(value) => Value::String(value.to_string()),
        None => Value::String(text),
    }
}

fn resolve_choice_list(raw: &Value, options: &[Value]) -> Value {
    let items: Vec<&Value> = match raw {
        Value::Null => return Value::Null,
        Value::Array(items) => items.iter().collect(),
        other => vec![other],
    };
    Value::Array(
        items
            .into_iter()
            .map(|item| resolve_choice(item, options))
            .filter(Value::is_string)
            .collect(),
    )
}

/// What the repair pass learned about one field.
struct FieldInfo {
    block_type: String,
    options: Vec<Value>,
}

#[derive(Default)]
struct Repair {
    /// Fields by id, for resolving the `data` model afterwards.
    fields: HashMap<String, FieldInfo>,
    /// Choice fields the user could not answer: no option survived the repair.
    unanswerable: Vec<String>,
}

fn normalize_block(block: &mut Value, repair: &mut Repair) {
    let Some(obj) = block.as_object_mut() else {
        return;
    };
    if let Some(Value::Array(children)) = obj.get_mut("blocks") {
        for child in children.iter_mut() {
            normalize_block(child, repair);
        }
    }
    let block_type = obj
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if let Some(raw) = obj.get("options") {
        let options = normalize_options(raw);
        obj.insert("options".into(), Value::Array(options));
    }
    let options: Vec<Value> = obj
        .get("options")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Some(default) = obj.get("default") {
        let resolved = if SINGLE_CHOICE_TYPES.contains(&block_type.as_str()) {
            Some(resolve_choice(default, &options))
        } else if MULTI_CHOICE_TYPES.contains(&block_type.as_str()) {
            Some(resolve_choice_list(default, &options))
        } else {
            None
        };
        if let Some(resolved) = resolved {
            obj.insert("default".into(), resolved);
        }
    }
    let id = obj
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_string);
    let allow_custom = obj
        .get("allow_custom")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if OPTIONS_REQUIRED_TYPES.contains(&block_type.as_str()) && options.is_empty() && !allow_custom
    {
        let name = id
            .clone()
            .or_else(|| obj.get("label").and_then(scalar_text))
            .unwrap_or_else(|| "(unnamed)".to_string());
        repair.unanswerable.push(format!("{name} ({block_type})"));
    }
    if let Some(id) = id {
        repair.fields.insert(id, FieldInfo { block_type, options });
    }
}

fn normalize_block_list(holder: &mut Map<String, Value>, repair: &mut Repair) {
    match holder.get_mut("blocks") {
        Some(Value::Array(blocks)) => {
            for block in blocks.iter_mut() {
                normalize_block(block, repair);
            }
        }
        _ => {
            holder.insert("blocks".into(), Value::Array(Vec::new()));
        }
    }
}

fn normalize_steps(holder: &mut Map<String, Value>, repair: &mut Repair) {
    if let Some(Value::Array(steps)) = holder.get_mut("steps") {
        for step in steps.iter_mut() {
            match step.as_object_mut() {
                Some(step_obj) => normalize_block_list(step_obj, repair),
                None => *step = json!({ "blocks": [] }),
            }
        }
    }
}

/// Repair a `chat_ui` definition in place.
///
/// Returns the choice fields that still cannot be answered (a select, radio or
/// checkbox with no usable option). The caller reports those to the model
/// instead of showing a form the user cannot submit.
pub fn normalize_ui_definition(def: &mut Value) -> Vec<String> {
    let Some(obj) = def.as_object_mut() else {
        *def = json!({ "blocks": [] });
        return Vec::new();
    };
    let mut repair = Repair::default();
    normalize_block_list(obj, &mut repair);
    normalize_steps(obj, &mut repair);
    if let Some(Value::Object(data)) = obj.get_mut("data") {
        for (id, field) in &repair.fields {
            let Some(current) = data.get(id) else {
                continue;
            };
            let resolved = if SINGLE_CHOICE_TYPES.contains(&field.block_type.as_str()) {
                resolve_choice(current, &field.options)
            } else if MULTI_CHOICE_TYPES.contains(&field.block_type.as_str()) {
                resolve_choice_list(current, &field.options)
            } else {
                continue;
            };
            data.insert(id.clone(), resolved);
        }
    }
    repair.unanswerable
}

/// Repair the blocks a `chat_ui_patch` replaces. A patch that carries no
/// `blocks` / `steps` is left as it is. Returns unanswerable choice fields, as
/// [`normalize_ui_definition`] does.
pub fn normalize_ui_patch(patch: &mut Value) -> Vec<String> {
    let Some(obj) = patch.as_object_mut() else {
        return Vec::new();
    };
    let mut repair = Repair::default();
    if obj.contains_key("blocks") {
        normalize_block_list(obj, &mut repair);
    }
    normalize_steps(obj, &mut repair);
    repair.unanswerable
}

/// Tool error for a form with choice fields that have nothing to choose from.
pub fn unanswerable_fields_error(fields: &[String]) -> String {
    format!(
        "Form not shown: these choice fields have no options the user could pick: {}. \
         Every select, radio and checkbox needs a non-empty `options` array of \
         {{\"value\": \"<unique non-empty string>\", \"label\": \"<text shown>\"}} \
         (or `allow_custom: true`). Fix the definition and call the tool again.",
        fields.join(", ")
    )
}

pub fn block_type_enum() -> Value {
    json!([
        "text",
        "divider",
        "section",
        "row",
        "column",
        "card",
        "image",
        "code_preview",
        "progress",
        "link_list",
        "text_input",
        "number_input",
        "slider",
        "switch",
        "date",
        "time",
        "datetime",
        "select",
        "radio",
        "checkbox",
        "tags",
        "koi_picker",
        "project_picker",
        "file_picker",
        "confirm",
        "actions"
    ])
}

pub fn block_item_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type"],
        "properties": {
            "type": {
                "type": "string",
                "enum": block_type_enum()
            },
            "id": { "type": "string" },
            "label": { "type": "string" },
            "description": { "type": "string" },
            "required": { "type": "boolean" },
            "disabled": { "type": "boolean" },
            "value": {},
            "content": { "type": "string" },
            "url": { "type": "string" },
            "alt": { "type": "string" },
            "language": { "type": "string" },
            "accept": { "type": "string" },
            "multiple": { "type": "boolean" },
            "blocks": {
                "type": "array",
                "description": "Child blocks for row/column/card layout containers.",
                "items": { "type": "object" }
            },
            "options": {
                "type": "array",
                "description": "Choices for select/radio/checkbox/tags/link_list. Required and non-empty for select, radio and checkbox.",
                "items": {
                    "type": "object",
                    "required": ["value", "label"],
                    "properties": {
                        "value": {
                            "type": "string",
                            "description": "Submitted value. A non-empty string, unique within this field (not a number, not null)."
                        },
                        "label": {
                            "type": "string",
                            "description": "Text shown to the user."
                        },
                        "description": { "type": "string" },
                        "href": { "type": "string" }
                    }
                }
            },
            "default": {},
            "placeholder": { "type": "string" },
            "allow_custom": { "type": "boolean" },
            "custom_label": { "type": "string" },
            "multiline": { "type": "boolean" },
            "rows": { "type": "integer" },
            "input_mode": {
                "type": "string",
                "enum": ["text", "email", "url", "password"]
            },
            "min_length": { "type": "integer" },
            "max_length": { "type": "integer" },
            "pattern": { "type": "string" },
            "show_when": {
                "type": "object",
                "properties": {
                    "field": { "type": "string" },
                    "equals": {},
                    "one_of": { "type": "array" },
                    "not_equals": {}
                }
            },
            "suggestions": {
                "type": "array",
                "items": { "type": "string" }
            },
            "allow_new": { "type": "boolean" },
            "min": {},
            "max": {},
            "step": { "type": "number" },
            "buttons": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["label"],
                    "properties": {
                        "id": { "type": "string" },
                        "label": { "type": "string" },
                        "value": {},
                        "style": {
                            "type": "string",
                            "enum": ["primary", "danger", "default"]
                        },
                        "emit": {
                            "type": "string",
                            "enum": ["submit", "action"]
                        }
                    }
                }
            }
        }
    })
}

pub fn ui_definition_schema() -> Value {
    json!({
        "type": "object",
        "required": ["blocks"],
        "properties": {
            "protocol_version": {
                "type": "string",
                "enum": ["1", "2"]
            },
            "mode": {
                "type": "string",
                "enum": ["form", "display", "wizard"]
            },
            "title": { "type": "string" },
            "description": { "type": "string" },
            "submit_label": { "type": "string" },
            "data": {
                "type": "object",
                "description": "v2 data model; echoed as __data_model__ on submit/action."
            },
            "blocks": {
                "type": "array",
                "items": block_item_schema()
            },
            "steps": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["blocks"],
                    "properties": {
                        "id": { "type": "string" },
                        "label": { "type": "string" },
                        "description": { "type": "string" },
                        "blocks": {
                            "type": "array",
                            "items": block_item_schema()
                        }
                    }
                }
            }
        }
    })
}

pub fn patch_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "title": { "type": "string" },
            "description": { "type": "string" },
            "submit_label": { "type": "string" },
            "mode": { "type": "string", "enum": ["form", "display", "wizard"] },
            "data": { "type": "object" },
            "blocks": {
                "type": "array",
                "items": block_item_schema()
            },
            "steps": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "label": { "type": "string" },
                        "description": { "type": "string" },
                        "blocks": {
                            "type": "array",
                            "items": block_item_schema()
                        }
                    }
                }
            },
            "wizard_step": { "type": "integer" },
            "reopen_submit": {
                "type": "boolean",
                "description": "Enable submit on the card (use with chat_ui_listen)."
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(options: &[Value]) -> Vec<&str> {
        options
            .iter()
            .map(|o| o["value"].as_str().expect("value is a string"))
            .collect()
    }
    fn labels(options: &[Value]) -> Vec<&str> {
        options
            .iter()
            .map(|o| o["label"].as_str().expect("label is a string"))
            .collect()
    }

    #[test]
    fn well_formed_options_are_left_as_they_are() {
        let raw = json!([
            { "value": "a", "label": "Alpha", "description": "first" },
            { "value": "b", "label": "Beta", "href": "https://example.com" }
        ]);
        assert_eq!(Value::Array(normalize_options(&raw)), raw);
    }

    #[test]
    fn numeric_and_boolean_values_become_strings() {
        let options = normalize_options(&json!([
            { "value": 1, "label": "One" },
            { "value": 2.0, "label": "Two" },
            { "value": 2.5, "label": "Two and a half" },
            { "value": true, "label": "Yes" }
        ]));
        assert_eq!(values(&options), ["1", "2", "2.5", "true"]);
    }

    #[test]
    fn value_is_taken_from_other_keys_or_from_the_label() {
        let options = normalize_options(&json!([
            { "id": "install", "label": "Install" },
            { "key": "skip", "text": "Skip" },
            { "name": "later" },
            { "label": "No value at all" },
            { "value": "", "label": "Blank value" },
            { "value": null, "title": "Null value" }
        ]));
        assert_eq!(
            values(&options),
            ["install", "skip", "later", "No value at all", "Blank value", "Null value"]
        );
        assert_eq!(
            labels(&options),
            ["Install", "Skip", "later", "No value at all", "Blank value", "Null value"]
        );
    }

    #[test]
    fn bare_scalars_and_value_label_maps_are_accepted() {
        assert_eq!(
            values(&normalize_options(&json!(["Apple", "Banana", 3]))),
            ["Apple", "Banana", "3"]
        );
        let from_map = normalize_options(&json!({ "a": "Option A", "b": "Option B" }));
        assert_eq!(values(&from_map), ["a", "b"]);
        assert_eq!(labels(&from_map), ["Option A", "Option B"]);
    }

    #[test]
    fn duplicate_values_are_made_unique() {
        let options = normalize_options(&json!([
            { "value": "a", "label": "First" },
            { "value": "a", "label": "Second" },
            { "value": "a", "label": "Third" },
            { "value": "a-2", "label": "Fourth" }
        ]));
        assert_eq!(values(&options), ["a", "a-2", "a-3", "a-2-2"]);
        // Nothing the model wrote is lost: labels still tell the choices apart.
        assert_eq!(labels(&options), ["First", "Second", "Third", "Fourth"]);
    }

    #[test]
    fn entries_with_nothing_to_show_are_dropped() {
        let options = normalize_options(&json!([{}, null, [], { "value": "   " }, { "value": "ok" }]));
        assert_eq!(values(&options), ["ok"]);
        assert!(normalize_options(&json!("not a list")).is_empty());
        assert!(normalize_options(&Value::Null).is_empty());
    }

    #[test]
    fn repair_is_idempotent() {
        let once = normalize_options(&json!([
            { "value": 1, "label": "One" },
            { "label": "Two" },
            "Three",
            { "value": 1, "label": "One again" }
        ]));
        let twice = normalize_options(&Value::Array(once.clone()));
        assert_eq!(once, twice);
    }

    #[test]
    fn defaults_and_data_follow_the_repaired_values() {
        let mut def = json!({
            "data": { "size": 2, "tags": [1, "Blue"], "note": 7 },
            "blocks": [
                {
                    "type": "select", "id": "size", "default": "Medium",
                    "options": [{ "value": 1, "label": "Small" }, { "value": 2, "label": "Medium" }]
                },
                {
                    "type": "checkbox", "id": "tags", "default": 1,
                    "options": [{ "value": 1, "label": "Red" }, { "value": "b", "label": "Blue" }]
                },
                { "type": "number_input", "id": "note", "default": 7 }
            ]
        });
        assert!(normalize_ui_definition(&mut def).is_empty());
        // A default written as a label resolves to that option's value.
        assert_eq!(def["blocks"][0]["default"], json!("2"));
        // A single default on a multi-choice field becomes a list.
        assert_eq!(def["blocks"][1]["default"], json!(["1"]));
        assert_eq!(def["data"]["size"], json!("2"));
        assert_eq!(def["data"]["tags"], json!(["1", "b"]));
        // Non-choice fields are not touched.
        assert_eq!(def["blocks"][2]["default"], json!(7));
        assert_eq!(def["data"]["note"], json!(7));
    }

    #[test]
    fn nested_blocks_and_wizard_steps_are_repaired() {
        let mut def = json!({
            "mode": "wizard",
            "blocks": [],
            "steps": [
                { "id": "one", "blocks": [
                    { "type": "card", "blocks": [
                        { "type": "radio", "id": "lang", "options": [{ "id": "go" }, { "id": "rust" }] }
                    ] }
                ] },
                "not a step"
            ]
        });
        assert!(normalize_ui_definition(&mut def).is_empty());
        let options = def["steps"][0]["blocks"][0]["blocks"][0]["options"].as_array().unwrap();
        assert_eq!(values(options), ["go", "rust"]);
        assert_eq!(def["steps"][1], json!({ "blocks": [] }));
    }

    #[test]
    fn choice_fields_nobody_can_answer_are_reported() {
        let mut def = json!({
            "blocks": [
                { "type": "radio", "id": "a", "options": [] },
                { "type": "select", "label": "Region" },
                { "type": "checkbox", "id": "c", "options": [{}] },
                // Answerable without options of their own:
                { "type": "radio", "id": "free", "options": [], "allow_custom": true },
                { "type": "tags", "id": "t" },
                { "type": "project_picker", "id": "p" },
                { "type": "koi_picker", "id": "k" },
                { "type": "text_input", "id": "name" }
            ]
        });
        assert_eq!(
            normalize_ui_definition(&mut def),
            ["a (radio)", "Region (select)", "c (checkbox)"]
        );
        let message = unanswerable_fields_error(&["a (radio)".to_string()]);
        assert!(message.contains("a (radio)"));
        assert!(message.contains("options"));
    }

    #[test]
    fn patch_blocks_are_repaired_and_other_patches_are_untouched() {
        let mut patch = json!({
            "title": "Step 2",
            "blocks": [{ "type": "select", "id": "s", "options": [{ "value": 10, "label": "Ten" }] }]
        });
        assert!(normalize_ui_patch(&mut patch).is_empty());
        assert_eq!(patch["blocks"][0]["options"][0]["value"], json!("10"));

        let mut title_only = json!({ "title": "Done", "data": { "progress": 100 } });
        let before = title_only.clone();
        assert!(normalize_ui_patch(&mut title_only).is_empty());
        assert_eq!(title_only, before, "a patch without blocks must not gain an empty blocks list");

        let mut broken = json!({ "blocks": [{ "type": "radio", "id": "r", "options": [] }] });
        assert_eq!(normalize_ui_patch(&mut broken), ["r (radio)"]);
    }

    #[test]
    fn option_schema_requires_value_and_label() {
        let schema = block_item_schema();
        let item = &schema["properties"]["options"]["items"];
        assert_eq!(item["required"], json!(["value", "label"]));
        assert_eq!(item["properties"]["value"]["type"], json!("string"));
    }
}
