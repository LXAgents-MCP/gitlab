---
name: memory-decisions-rust-runtime-dependencies
description: Why the Rust server depends on more than the MCP SDK, and what each runtime crate is for.
---

# The Rust server's runtime dependencies

## 2026-11-14

**Decision.** The repository rule has been that the runtime dependencies are the MCP SDK and its schema
library, and that a third needs a record here before it lands. In Rust the standard library has no async
runtime, no HTTP client or server, no TLS and no JSON, so the server depends on the crates below. Each one
replaces something Node gave for free, or is the framework the owner chose.

| Crate | For | Replaces |
|---|---|---|
| `rmcp` | The MCP protocol, the stdio transport and the stateless streamable HTTP transport. The official SDK, which the owner chose. | `@modelcontextprotocol/sdk` |
| `axum` | The HTTP routes (`GET /healthz`, `POST /mcp`) and the guard that runs before the body is read. The owner chose it. | `node:http` |
| `tokio` | Async runtime, stdin and stdout, sockets, signals, timers. | Node's event loop |
| `reqwest` (rustls) | Calls to gitlab.com: a timeout, a streamed size cap, and redirects that drop `Authorization` when the host changes, as `fetch` did. rustls, so no OpenSSL. | `fetch` |
| `serde`, `serde_json` | JSON. `preserve_order`, because the rendered text lists a payload's fields in the order the tool wrote them. | `JSON` |
| `sha2`, `subtle` | The access token's digest and its constant-time comparison. | `createHash`, `timingSafeEqual` |
| `base64` | File content written and read. | `Buffer` |
| `percent-encoding` | `encodeURIComponent`, character for character. | `encodeURIComponent` |
| `regex` | The patterns the tools match GitLab's messages and logs against; linear time. | `RegExp` |
| `chrono` | Reading ISO 8601 dates, and writing the time a rate limit resets. | `Date` |
| `encoding_rs` | Decoding a file that is not UTF-8 as Windows-1252. | `TextDecoder` |
| `futures` | Catching a handler that panics, so it is an Internal Error and not a dropped connection. | `try`/`catch` |
| `tower`, `http-body-util`, `bytes` | Handing a checked request on to the SDK's HTTP service. | — |

**What is still true.** A dependency is not added to serve one call site: a crate joins this list with a
reason here first. No crate reads the environment, writes to stdout or opens a listener of its own.
