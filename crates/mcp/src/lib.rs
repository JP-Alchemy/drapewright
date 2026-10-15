//! A Model Context Protocol (MCP) server for Drapewright: AI assistants
//! can open, check and change projects with the same commands as the app,
//! and see pictures of the pieces.
//!
//! It speaks JSON-RPC 2.0 over stdio, one message per line
//! (<https://modelcontextprotocol.io>). Everything stays on this computer:
//! the server reads and writes only the files it is asked to, and makes no
//! network connections.

mod drape;
mod font;
mod render;
mod session;
mod text;
mod tools;
mod worn;

use serde_json::{Value, json};

pub use session::{Host, Session, generator};
pub use tools::{Output, Tool};

/// Protocol versions this server speaks, newest first.
pub const PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// What the server tells an assistant when it connects.
const INSTRUCTIONS: &str = "Drapewright is a sewing pattern studio. This server works on one Drapewright project at a time: a .drapewright file the user opens in the Drapewright app.

- All lengths are millimetres. Points are [x, y] with y pointing down, in a piece's own frame unless a tool says the work table.
- Start with project_summary, open_project, new_project, or inspect_pattern_file and import_pattern.
- Use render_preview to see the pieces; with labels \"edges\" it shows every edge's id, which update_edge, add_notch and split_edge take.
- Never guess what the user should decide. When a file holds several sizes, doesn't say how big it is, or may or may not include seam allowance, ask the user and pass their answer.
- Each tool call is one undo step (undo, redo). Nothing is saved until save_project.
- Seam allowance terms: \"add\" means the outline is the stitching line and allowance goes outside it; \"included\" means the outline is the cutting line. Fold edges get no allowance.";

/// A tool that could not do what was asked; the message says why, in
/// plain words, and is passed back to the assistant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolError(pub String);

impl ToolError {
    pub fn new(message: impl Into<String>) -> Self {
        ToolError(message.into())
    }
}

/// Standard base64, for pictures.
pub fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> shift) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[derive(Debug)]
pub struct Server {
    session: Session,
    tools: Vec<Tool>,
}

impl Server {
    pub fn new(host: Host) -> Self {
        Server {
            session: Session::new(host),
            tools: tools::all(),
        }
    }

    pub fn tools(&self) -> &[Tool] {
        &self.tools
    }

    /// Handles one line from the client: a message, or a batch of them.
    /// Returns the line to send back, if any.
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let message: Value = match serde_json::from_str(line) {
            Ok(m) => m,
            Err(_) => return Some(error(Value::Null, -32700, "Parse error").to_string()),
        };
        match message {
            Value::Array(batch) => {
                let replies: Vec<Value> =
                    batch.into_iter().filter_map(|m| self.handle(m)).collect();
                (!replies.is_empty()).then(|| Value::Array(replies).to_string())
            }
            m => self.handle(m).map(|r| r.to_string()),
        }
    }

    /// Handles one message. Notifications and responses get no reply.
    pub fn handle(&mut self, message: Value) -> Option<Value> {
        let id = message.get("id").cloned();
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            // A response to a request we never send, or not a message.
            let is_response = message.get("result").is_some() || message.get("error").is_some();
            return match id {
                Some(id) if !is_response => Some(error(id, -32600, "Invalid request")),
                _ => None,
            };
        };
        let id = id?;
        let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
        let result = match method {
            "initialize" => Ok(initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({
                "tools": self.tools.iter().map(Tool::describe).collect::<Vec<_>>(),
            })),
            "tools/call" => self.call(&params),
            "resources/list" => Ok(json!({ "resources": [] })),
            "resources/templates/list" => Ok(json!({ "resourceTemplates": [] })),
            "prompts/list" => Ok(json!({ "prompts": [] })),
            _ => Err((-32601, format!("Method not found: {method}"))),
        };
        Some(match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message)) => error(id, code, &message),
        })
    }

    fn call(&mut self, params: &Value) -> Result<Value, (i64, String)> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or((-32602, "tools/call needs a tool name".to_string()))?;
        let tool = self
            .tools
            .iter()
            .find(|t| t.name == name)
            .ok_or_else(|| (-32602, format!("Unknown tool: {name}")))?;
        let args = match params.get("arguments") {
            None | Some(Value::Null) => json!({}),
            Some(a) => a.clone(),
        };
        Ok(match tool.call(&mut self.session, args) {
            Ok(output) => json!({ "content": output.content, "isError": false }),
            Err(ToolError(message)) => json!({
                "content": [{ "type": "text", "text": message }],
                "isError": true,
            }),
        })
    }
}

fn initialize(params: &Value) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str);
    let version = asked
        .filter(|v| PROTOCOL_VERSIONS.contains(v))
        .unwrap_or(PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": {
            "name": "drapewright",
            "title": "Drapewright",
            "version": drapewright_pattern_core::VERSION,
        },
        "instructions": INSTRUCTIONS,
    })
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(&[0xff, 0xfe, 0xfd, 0x00]), "//79AA==");
    }
}
