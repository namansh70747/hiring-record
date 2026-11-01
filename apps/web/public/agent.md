# Agent access to Hiring Record

MCP Streamable HTTP is available at `/mcp`. All data is synthetic. Tools are read-only and inspect a reproducible approved example; browser sessions are private and cannot be selected by ID.

Tools: `connection_check`, `get_role`, `get_people`, `get_report`, `list_events`, `explain_refusal`, `verify_report`. The verifier accepts signed JSON and an optional independently trusted public key.

Example: `explain_refusal` with `{ "code": "R-AFTER-HIRE" }`. The result preserves the descriptive count and explains the refusal. Signatures and event chains establish integrity, not source accuracy.
