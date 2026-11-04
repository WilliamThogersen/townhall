//! The MCP endpoint (unit 2.3): JSON-RPC 2.0 for townhall's session, tools in `townhall_core::mcp`.
//! Not built yet: every handler answers `501 not implemented yet`.

use crate::error::ApiError;

pub async fn rpc() -> ApiError {
    ApiError::NotImplemented
}
