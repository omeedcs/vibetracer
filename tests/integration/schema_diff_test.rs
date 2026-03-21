use vibetracer::analysis::schema_diff::{SchemaKind, diff_schemas, parse_schema};

const PYDANTIC_SOURCE: &str = r#"
class User(BaseModel):
    id: int
    name: str
    email: str
"#;

#[test]
fn test_parse_pydantic_model() {
    let schema =
        parse_schema(PYDANTIC_SOURCE, SchemaKind::Pydantic).expect("should parse pydantic model");

    assert_eq!(schema.name, "User");
    assert_eq!(schema.fields.len(), 3);

    let names: Vec<&str> = schema.fields.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"id"));
    assert!(names.contains(&"name"));
    assert!(names.contains(&"email"));
}

#[test]
fn test_diff_schemas_detects_additions() {
    let old_source = r#"
class User(BaseModel):
    id: int
    name: str
"#;

    let new_source = r#"
class User(BaseModel):
    id: int
    name: str
    email: str
"#;

    let old_schema = parse_schema(old_source, SchemaKind::Pydantic).expect("parse old schema");
    let new_schema = parse_schema(new_source, SchemaKind::Pydantic).expect("parse new schema");

    let diff = diff_schemas(&old_schema, &new_schema);

    assert_eq!(diff.added.len(), 1, "expected 1 added field");
    assert_eq!(diff.added[0].name, "email");
    assert!(diff.removed.is_empty(), "expected no removed fields");
    assert!(diff.type_changed.is_empty(), "expected no type changes");
}
