# Agent access to Hiring Record

This is a local, synthetic, read-only MCP server at `http://127.0.0.1:8080/mcp` using Streamable HTTP. There is no authentication or real candidate data; do not expose this local demo as a production service.

Tools: `connection_check`, `get_role`, `get_people`, `get_report`, `list_events`, `explain_refusal`.

Omit `session_id` to inspect a reproducible example in which proposal B is approved. To inspect a particular browser scenario, copy its synthetic session ID from the workspace's “For builders” section and pass it as the optional `session_id` argument. The MCP server has no mutation tools.

Example tool input: `explain_refusal` with `{"code":"R-AFTER-HIRE"}`. The result preserves the descriptive count and explains why a predictive claim is refused. `R-REJECTS-MET` is another example.

The HTTP API supports writes for the local synthetic UI. Those writes are not exposed as MCP tools. A signed export can be checked in `/verify/` or by `record verify`; signer identity requires an independently trusted key.
