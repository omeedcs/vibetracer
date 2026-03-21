use regex::Regex;

/// The schema language/framework this schema was parsed from.
#[derive(Debug, Clone, PartialEq)]
pub enum SchemaKind {
    Pydantic,
    TypeScriptInterface,
    SqlTable,
}

/// A single field in a schema.
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaField {
    pub name: String,
    pub field_type: String,
}

/// A parsed schema (class / interface / table).
#[derive(Debug, Clone)]
pub struct Schema {
    pub name: String,
    pub kind: SchemaKind,
    pub fields: Vec<SchemaField>,
}

/// The diff between two versions of a schema.
#[derive(Debug, Clone, Default)]
pub struct SchemaDiff {
    pub added: Vec<SchemaField>,
    pub removed: Vec<SchemaField>,
    pub type_changed: Vec<(SchemaField, SchemaField)>,
}

// ── public API ────────────────────────────────────────────────────────────────

/// Parse `source` as the given `kind`. Returns `None` if parsing fails.
pub fn parse_schema(source: &str, kind: SchemaKind) -> Option<Schema> {
    match kind {
        SchemaKind::Pydantic => parse_pydantic(source),
        SchemaKind::TypeScriptInterface => parse_typescript_interface(source),
        SchemaKind::SqlTable => None, // not implemented in v1
    }
}

/// Detect schema kind from file extension and content heuristics.
pub fn detect_schema_kind(filename: &str, content: &str) -> Option<SchemaKind> {
    if filename.ends_with(".py") && content.contains("BaseModel") {
        return Some(SchemaKind::Pydantic);
    }
    if filename.ends_with(".ts") && content.contains("interface ") {
        return Some(SchemaKind::TypeScriptInterface);
    }
    if filename.ends_with(".sql") && content.to_lowercase().contains("create table") {
        return Some(SchemaKind::SqlTable);
    }
    None
}

/// Compute the diff between two schemas.
pub fn diff_schemas(old: &Schema, new: &Schema) -> SchemaDiff {
    let mut diff = SchemaDiff::default();

    // Build lookup maps keyed by field name.
    let old_map: std::collections::HashMap<&str, &SchemaField> =
        old.fields.iter().map(|f| (f.name.as_str(), f)).collect();
    let new_map: std::collections::HashMap<&str, &SchemaField> =
        new.fields.iter().map(|f| (f.name.as_str(), f)).collect();

    for (name, old_field) in &old_map {
        match new_map.get(*name) {
            None => diff.removed.push((*old_field).clone()),
            Some(new_field) => {
                if old_field.field_type != new_field.field_type {
                    diff.type_changed
                        .push(((*old_field).clone(), (*new_field).clone()));
                }
            }
        }
    }

    for (name, new_field) in &new_map {
        if !old_map.contains_key(*name) {
            diff.added.push((*new_field).clone());
        }
    }

    diff
}

// ── parsers ───────────────────────────────────────────────────────────────────

/// Parse a Pydantic `BaseModel` subclass.
pub fn parse_pydantic(source: &str) -> Option<Schema> {
    let class_re = Regex::new(r"class\s+(\w+)\s*\(.*BaseModel.*\)\s*:").ok()?;
    let field_re = Regex::new(r"^\s{4}(\w+)\s*:\s*([\w\[\], |]+)").ok()?;

    let caps = class_re.captures(source)?;
    let name = caps.get(1)?.as_str().to_string();

    // Collect field lines that appear after the class declaration.
    let mut fields = Vec::new();
    let mut in_body = false;
    for line in source.lines() {
        if class_re.is_match(line) {
            in_body = true;
            continue;
        }
        if in_body {
            if let Some(fc) = field_re.captures(line) {
                let field_name = fc.get(1)?.as_str().to_string();
                let field_type = fc.get(2)?.as_str().trim().to_string();
                fields.push(SchemaField {
                    name: field_name,
                    field_type,
                });
            }
        }
    }

    Some(Schema {
        name,
        kind: SchemaKind::Pydantic,
        fields,
    })
}

/// Parse a TypeScript `interface`.
pub fn parse_typescript_interface(source: &str) -> Option<Schema> {
    let iface_re = Regex::new(r"interface\s+(\w+)\s*\{").ok()?;
    let field_re = Regex::new(r"^\s+(\w+)\s*:\s*([\w\[\]<>, |]+)\s*;").ok()?;

    let caps = iface_re.captures(source)?;
    let name = caps.get(1)?.as_str().to_string();

    let mut fields = Vec::new();
    let mut in_body = false;
    for line in source.lines() {
        if iface_re.is_match(line) {
            in_body = true;
            continue;
        }
        if in_body {
            if line.trim() == "}" {
                break;
            }
            if let Some(fc) = field_re.captures(line) {
                let field_name = fc.get(1)?.as_str().to_string();
                let field_type = fc.get(2)?.as_str().trim().to_string();
                fields.push(SchemaField {
                    name: field_name,
                    field_type,
                });
            }
        }
    }

    Some(Schema {
        name,
        kind: SchemaKind::TypeScriptInterface,
        fields,
    })
}
