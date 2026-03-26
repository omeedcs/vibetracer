pub mod handlers;
pub mod pagination;
pub mod streaming;
pub mod tools;
pub mod transport;
pub mod types;

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::json;

use self::handlers::HandlerContext;
use self::tools::all_tool_definitions;
use self::transport::{StdioReader, StdioWriter};
use self::types::{INVALID_PARAMS, JsonRpcError, JsonRpcResponse, METHOD_NOT_FOUND};

/// Run the MCP server main loop, reading JSON-RPC from stdin and writing
/// responses to stdout. This is the entry point for `vibetracer mcp`.
pub fn run_mcp_server(project_path: PathBuf) -> Result<()> {
    let sessions_dir = project_path.join(".vibetracer").join("sessions");
    let ctx = HandlerContext::new(sessions_dir.clone());

    let mut reader = StdioReader::new(Box::new(std::io::stdin()));
    let mut writer = StdioWriter::new(Box::new(std::io::stdout()));

    loop {
        let request = match reader.read_message()? {
            Some(req) => req,
            None => break, // EOF
        };

        // Notifications have no id — skip (no response expected).
        let id = match request.id {
            Some(id) => id,
            None => continue,
        };

        let params = request.params.unwrap_or(json!({}));

        let response = match request.method.as_str() {
            "initialize" => JsonRpcResponse::success(
                id,
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "vibetracer-mcp",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }),
            ),

            "tools/list" => {
                let tools = all_tool_definitions();
                JsonRpcResponse::success(id, json!({ "tools": tools }))
            }

            "tools/call" => {
                let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

                match dispatch_tool(
                    &ctx,
                    tool_name,
                    &arguments,
                    &project_path,
                    &sessions_dir,
                    &mut writer,
                ) {
                    Ok(result) => {
                        let text = serde_json::to_string(&result)?;
                        JsonRpcResponse::success(
                            id,
                            json!({
                                "content": [{
                                    "type": "text",
                                    "text": text
                                }]
                            }),
                        )
                    }
                    Err(e) => JsonRpcResponse::error(
                        id,
                        JsonRpcError {
                            code: INVALID_PARAMS,
                            message: format!("{}", e),
                            data: None,
                        },
                    ),
                }
            }

            _ => JsonRpcResponse::error(
                id,
                JsonRpcError {
                    code: METHOD_NOT_FOUND,
                    message: format!("unknown method: {}", request.method),
                    data: None,
                },
            ),
        };

        writer.write_message(&response)?;
    }

    Ok(())
}

/// Route a tool name to the appropriate handler method.
fn dispatch_tool(
    ctx: &HandlerContext,
    tool_name: &str,
    arguments: &serde_json::Value,
    project_path: &Path,
    sessions_dir: &Path,
    writer: &mut StdioWriter,
) -> Result<serde_json::Value> {
    match tool_name {
        "list_sessions" => ctx.handle_list_sessions(arguments),
        "get_timeline" => ctx.handle_get_timeline(arguments),
        "get_frame" => ctx.handle_get_frame(arguments),
        "diff_frames" => ctx.handle_diff_frames(arguments),
        "search_edits" => ctx.handle_search_edits(arguments),
        "get_regression_window" => ctx.handle_get_regression_window(arguments),
        "subscribe_edits" => {
            streaming::handle_subscribe(arguments, project_path, sessions_dir, writer)
        }
        _ => anyhow::bail!("unknown tool: {}", tool_name),
    }
}
