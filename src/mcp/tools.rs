use serde_json::json;

use super::types::McpToolDef;

/// Returns definitions for all 7 MCP tools exposed by vibetracer.
///
/// Each tool has a name, human-readable description, and a JSON Schema
/// (`input_schema`) that describes the accepted parameters. These are
/// returned verbatim in the `tools/list` MCP response.
pub fn all_tool_definitions() -> Vec<McpToolDef> {
    vec![
        // 1. list_sessions
        McpToolDef {
            name: "list_sessions".to_string(),
            description: "List recorded vibetracer sessions, most recent first.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of sessions to return."
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Number of sessions to skip for pagination."
                    }
                }
            }),
        },
        // 2. get_timeline
        McpToolDef {
            name: "get_timeline".to_string(),
            description:
                "Get the edit timeline for a session, returning frames in chronological order."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "The session identifier."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of frames to return."
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Number of frames to skip for pagination."
                    },
                    "file_filter": {
                        "type": "string",
                        "description": "Glob pattern to filter frames by file path."
                    }
                },
                "required": ["session_id"]
            }),
        },
        // 3. get_frame
        McpToolDef {
            name: "get_frame".to_string(),
            description: "Get a single frame (snapshot) from a session, including its diff."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "The session identifier."
                    },
                    "frame_id": {
                        "type": "integer",
                        "description": "The frame number within the session."
                    },
                    "file": {
                        "type": "string",
                        "description": "Optional file path to restrict the frame output to."
                    }
                },
                "required": ["session_id", "frame_id"]
            }),
        },
        // 4. diff_frames
        McpToolDef {
            name: "diff_frames".to_string(),
            description: "Compute the diff between two frames in a session.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "The session identifier."
                    },
                    "frame_a": {
                        "type": "integer",
                        "description": "The first frame number to compare."
                    },
                    "frame_b": {
                        "type": "integer",
                        "description": "The second frame number to compare."
                    },
                    "file": {
                        "type": "string",
                        "description": "Optional file path to restrict the diff to."
                    }
                },
                "required": ["session_id", "frame_a", "frame_b"]
            }),
        },
        // 5. search_edits
        McpToolDef {
            name: "search_edits".to_string(),
            description: "Search for edits matching a pattern within a session.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "The session identifier."
                    },
                    "query": {
                        "type": "string",
                        "description": "Regex pattern, falls back to literal if invalid."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results to return."
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Number of results to skip for pagination."
                    }
                },
                "required": ["session_id", "query"]
            }),
        },
        // 6. get_regression_window
        McpToolDef {
            name: "get_regression_window".to_string(),
            description: "Identify the frame range where a regression was likely introduced."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "The session identifier."
                    },
                    "file": {
                        "type": "string",
                        "description": "Optional file path to narrow the search."
                    },
                    "start_frame": {
                        "type": "integer",
                        "description": "Start of the frame range to search within."
                    },
                    "end_frame": {
                        "type": "integer",
                        "description": "End of the frame range to search within."
                    }
                },
                "required": ["session_id"]
            }),
        },
        // 7. subscribe_edits
        McpToolDef {
            name: "subscribe_edits".to_string(),
            description: "Subscribe to real-time edit notifications for a session.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "The session identifier."
                    }
                },
                "required": ["session_id"]
            }),
        },
    ]
}

// ─── unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_tools_have_valid_schemas() {
        let tools = all_tool_definitions();
        assert_eq!(tools.len(), 7, "expected exactly 7 tool definitions");
        for tool in &tools {
            assert!(!tool.name.is_empty(), "tool name must not be empty");
            assert!(
                !tool.description.is_empty(),
                "tool description must not be empty"
            );
            assert_eq!(
                tool.input_schema["type"], "object",
                "input_schema type must be \"object\" for tool {}",
                tool.name
            );
        }
    }

    #[test]
    fn test_tool_names() {
        let tools = all_tool_definitions();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        let expected = [
            "list_sessions",
            "get_timeline",
            "get_frame",
            "diff_frames",
            "search_edits",
            "get_regression_window",
            "subscribe_edits",
        ];
        for name in &expected {
            assert!(
                names.contains(name),
                "missing tool definition for \"{}\"",
                name
            );
        }
    }

    #[test]
    fn test_list_sessions_schema_has_pagination() {
        let tools = all_tool_definitions();
        let ls = tools.iter().find(|t| t.name == "list_sessions").unwrap();
        let props = &ls.input_schema["properties"];
        assert!(
            props.get("limit").is_some(),
            "list_sessions should have a limit property"
        );
        assert!(
            props.get("offset").is_some(),
            "list_sessions should have an offset property"
        );
    }

    #[test]
    fn test_get_frame_schema_requires_session_id_and_frame_id() {
        let tools = all_tool_definitions();
        let gf = tools.iter().find(|t| t.name == "get_frame").unwrap();
        let required = gf.input_schema["required"]
            .as_array()
            .expect("get_frame should have a required array");
        let required_strs: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
        assert!(
            required_strs.contains(&"session_id"),
            "get_frame must require session_id"
        );
        assert!(
            required_strs.contains(&"frame_id"),
            "get_frame must require frame_id"
        );
    }
}
