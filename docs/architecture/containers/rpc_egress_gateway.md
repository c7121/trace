# RPC Egress Gateway

Platform service that provides a controlled outbound path for blockchain RPC calls.

Workers do not have direct internet egress. Any RPC access happens through this gateway (or via in-VPC nodes).

See ADR 0002 (networking).

## Responsibilities

- Provide a stable internal endpoint for RPC requests from workers.
- Apply environment-level controls (timeouts, concurrency limits, request logging).
- Hold provider credentials (if any) via launch-time secret injection.

## Notes

- This service is the only component with outbound internet egress for RPC access.
- Destination selection is environment-configured (e.g., per-chain upstream RPC providers).
