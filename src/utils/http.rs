//! The HTTP transport's refusals, in the JSON-RPC envelope every other refusal on it uses, so a client
//! never has to branch on content type to find out it was refused.
use axum::body::Body;
use axum::http::{HeaderName, HeaderValue, Response, StatusCode, header};
use serde_json::json;

/// A JSON-RPC error with `id: null`, and any extra headers.
pub fn rpc_error(
    status: u16,
    code: i64,
    message: &str,
    headers: &[(&str, &str)],
) -> Response<Body> {
    let body =
        json!({ "jsonrpc": "2.0", "error": { "code": code, "message": message }, "id": null })
            .to_string();
    let mut response = Response::new(Body::from(body.clone()));
    *response.status_mut() =
        StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let map = response.headers_mut();
    map.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    map.insert(header::CONTENT_LENGTH, HeaderValue::from(body.len()));
    for (name, value) in headers {
        if let (Ok(name), Ok(value)) = (HeaderName::try_from(*name), HeaderValue::from_str(value)) {
            map.insert(name, value);
        }
    }
    response
}

/// The refusal for a request that does not carry the access token. A request that sent a credential which
/// did not match also gets `error="invalid_token"`, so a client can tell a missing header from a wrong
/// one. Neither answer says anything about the token itself.
pub fn unauthorized(presented: bool) -> Response<Body> {
    if presented {
        rpc_error(
            401,
            -32001,
            "Unauthorized: the token is not valid",
            &[(
                "www-authenticate",
                "Bearer realm=\"mcp\", error=\"invalid_token\"",
            )],
        )
    } else {
        rpc_error(
            401,
            -32001,
            "Unauthorized: send the server's access token as Authorization: Bearer <token>",
            &[("www-authenticate", "Bearer realm=\"mcp\"")],
        )
    }
}
