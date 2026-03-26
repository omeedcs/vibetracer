use serde::{Deserialize, Serialize};

// ─── JSON-RPC 2.0 error codes ───────────────────────────────────────────────

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;

// ─── JSON-RPC 2.0 types ─────────────────────────────────────────────────────

/// A JSON-RPC 2.0 request (has an `id`; expects a response).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// A JSON-RPC 2.0 response (success or error, never both).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    /// Build a success response.
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Build an error response.
    pub fn error(id: serde_json::Value, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

/// The error object inside a JSON-RPC 2.0 error response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// A JSON-RPC 2.0 notification (no `id`; fire-and-forget).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

// ─── MCP-specific types ─────────────────────────────────────────────────────

/// An MCP tool definition advertised by the server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpToolDef {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

// ─── unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── JSON-RPC request deserialization ─────────────────────────────────────

    #[test]
    fn test_jsonrpc_request_deserialize_full() {
        let raw = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {"capabilities": {}}
        }"#;
        let req: JsonRpcRequest = serde_json::from_str(raw).unwrap();
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.id, Some(json!(1)));
        assert_eq!(req.method, "initialize");
        assert_eq!(req.params, Some(json!({"capabilities": {}})));
    }

    #[test]
    fn test_jsonrpc_request_deserialize_no_params() {
        let raw = r#"{"jsonrpc": "2.0", "id": 42, "method": "tools/list"}"#;
        let req: JsonRpcRequest = serde_json::from_str(raw).unwrap();
        assert_eq!(req.id, Some(json!(42)));
        assert_eq!(req.method, "tools/list");
        assert_eq!(req.params, None);
    }

    #[test]
    fn test_jsonrpc_request_deserialize_string_id() {
        let raw = r#"{"jsonrpc": "2.0", "id": "abc-123", "method": "ping"}"#;
        let req: JsonRpcRequest = serde_json::from_str(raw).unwrap();
        assert_eq!(req.id, Some(json!("abc-123")));
    }

    #[test]
    fn test_jsonrpc_request_deserialize_null_id() {
        // JSON-RPC 2.0 allows "id": null. With serde's default Option handling,
        // null is deserialized as None (same as absent). This is acceptable
        // because notifications (no id) use a separate type.
        let raw = r#"{"jsonrpc": "2.0", "id": null, "method": "ping"}"#;
        let req: JsonRpcRequest = serde_json::from_str(raw).unwrap();
        assert_eq!(req.id, None);
    }

    // ── JSON-RPC response serialization ─────────────────────────────────────

    #[test]
    fn test_jsonrpc_response_success_serialization() {
        let resp = JsonRpcResponse::success(json!(1), json!({"status": "ok"}));
        let serialized = serde_json::to_value(&resp).unwrap();
        assert_eq!(serialized["jsonrpc"], "2.0");
        assert_eq!(serialized["id"], 1);
        assert_eq!(serialized["result"]["status"], "ok");
        // error field should be absent when None
        assert!(serialized.get("error").is_none());
    }

    #[test]
    fn test_jsonrpc_response_success_no_error_field() {
        let resp = JsonRpcResponse::success(json!(1), json!("done"));
        let json_str = serde_json::to_string(&resp).unwrap();
        assert!(!json_str.contains("error"));
    }

    #[test]
    fn test_jsonrpc_response_error_serialization() {
        let resp = JsonRpcResponse::error(
            json!(1),
            JsonRpcError {
                code: METHOD_NOT_FOUND,
                message: "Method not found".to_string(),
                data: None,
            },
        );
        let serialized = serde_json::to_value(&resp).unwrap();
        assert_eq!(serialized["jsonrpc"], "2.0");
        assert_eq!(serialized["id"], 1);
        assert_eq!(serialized["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(serialized["error"]["message"], "Method not found");
        // result field should be absent when None
        assert!(serialized.get("result").is_none());
    }

    #[test]
    fn test_jsonrpc_response_error_no_result_field() {
        let resp = JsonRpcResponse::error(
            json!(5),
            JsonRpcError {
                code: INTERNAL_ERROR,
                message: "boom".to_string(),
                data: None,
            },
        );
        let json_str = serde_json::to_string(&resp).unwrap();
        assert!(!json_str.contains("result"));
    }

    // ── JSON-RPC error serialization ────────────────────────────────────────

    #[test]
    fn test_jsonrpc_error_serialization() {
        let err = JsonRpcError {
            code: PARSE_ERROR,
            message: "Parse error".to_string(),
            data: Some(json!({"detail": "unexpected token"})),
        };
        let serialized = serde_json::to_value(&err).unwrap();
        assert_eq!(serialized["code"], PARSE_ERROR);
        assert_eq!(serialized["message"], "Parse error");
        assert_eq!(serialized["data"]["detail"], "unexpected token");
    }

    #[test]
    fn test_jsonrpc_error_no_data() {
        let err = JsonRpcError {
            code: INVALID_REQUEST,
            message: "Invalid request".to_string(),
            data: None,
        };
        let json_str = serde_json::to_string(&err).unwrap();
        assert!(!json_str.contains("data"));
    }

    // ── Error code constants ────────────────────────────────────────────────

    #[test]
    fn test_error_code_constants() {
        assert_eq!(PARSE_ERROR, -32700);
        assert_eq!(INVALID_REQUEST, -32600);
        assert_eq!(METHOD_NOT_FOUND, -32601);
        assert_eq!(INVALID_PARAMS, -32602);
        assert_eq!(INTERNAL_ERROR, -32603);
    }

    // ── JSON-RPC notification handling ──────────────────────────────────────

    #[test]
    fn test_jsonrpc_notification_deserialize() {
        let raw = r#"{
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        }"#;
        let notif: JsonRpcNotification = serde_json::from_str(raw).unwrap();
        assert_eq!(notif.jsonrpc, "2.0");
        assert_eq!(notif.method, "notifications/initialized");
        assert_eq!(notif.params, Some(json!({})));
    }

    #[test]
    fn test_jsonrpc_notification_no_params() {
        let raw = r#"{"jsonrpc": "2.0", "method": "notifications/cancelled"}"#;
        let notif: JsonRpcNotification = serde_json::from_str(raw).unwrap();
        assert_eq!(notif.method, "notifications/cancelled");
        assert_eq!(notif.params, None);
    }

    #[test]
    fn test_jsonrpc_notification_serialize() {
        let notif = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "notifications/initialized".to_string(),
            params: None,
        };
        let json_str = serde_json::to_string(&notif).unwrap();
        assert!(json_str.contains("\"jsonrpc\":\"2.0\""));
        assert!(json_str.contains("\"method\":\"notifications/initialized\""));
        assert!(!json_str.contains("params"));
    }

    // ── MCP tool definition serialization ───────────────────────────────────

    #[test]
    fn test_mcp_tool_def_serialization() {
        let tool = McpToolDef {
            name: "get_timeline".to_string(),
            description: "Get the edit timeline".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": {"type": "integer"}
                }
            }),
        };
        let serialized = serde_json::to_value(&tool).unwrap();
        assert_eq!(serialized["name"], "get_timeline");
        assert_eq!(serialized["description"], "Get the edit timeline");
        // Field must be renamed to "inputSchema" in JSON
        assert!(serialized.get("inputSchema").is_some());
        assert!(serialized.get("input_schema").is_none());
        assert_eq!(serialized["inputSchema"]["type"], "object");
    }

    #[test]
    fn test_mcp_tool_def_deserialize() {
        let raw = r#"{
            "name": "search",
            "description": "Search edits",
            "inputSchema": {"type": "object", "properties": {}}
        }"#;
        let tool: McpToolDef = serde_json::from_str(raw).unwrap();
        assert_eq!(tool.name, "search");
        assert_eq!(tool.description, "Search edits");
        assert_eq!(tool.input_schema["type"], "object");
    }

    // ── Roundtrip tests ─────────────────────────────────────────────────────

    #[test]
    fn test_jsonrpc_request_roundtrip() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(99)),
            method: "tools/call".to_string(),
            params: Some(json!({"name": "get_timeline", "arguments": {}})),
        };
        let json_str = serde_json::to_string(&req).unwrap();
        let restored: JsonRpcRequest = serde_json::from_str(&json_str).unwrap();
        assert_eq!(restored.jsonrpc, req.jsonrpc);
        assert_eq!(restored.id, req.id);
        assert_eq!(restored.method, req.method);
        assert_eq!(restored.params, req.params);
    }

    #[test]
    fn test_jsonrpc_response_roundtrip() {
        let resp = JsonRpcResponse::success(json!(7), json!(["a", "b"]));
        let json_str = serde_json::to_string(&resp).unwrap();
        let restored: JsonRpcResponse = serde_json::from_str(&json_str).unwrap();
        assert_eq!(restored.jsonrpc, "2.0");
        assert_eq!(restored.id, json!(7));
        assert_eq!(restored.result, Some(json!(["a", "b"])));
        assert_eq!(restored.error, None);
    }
}
