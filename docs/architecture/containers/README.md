# Containers

Container docs describe Trace's deployable units: responsibilities, trust boundaries, and key interactions.

Start with the system view in [../c4.md](../c4.md).

## Index

- [Gateway](gateway.md): public entrypoint, authn and authz, request routing
- [Dispatcher](dispatcher.md): task orchestration, leases, retries, outbox
- [Workers](workers.md): trusted worker wrappers and platform workers
- [Query Service](query_service.md): SQL execution against hot and cold data
- [Delivery Service](delivery_service.md): outbound delivery and retries
- [RPC Egress Gateway](rpc_egress_gateway.md): controlled egress for JSON-RPC

## Related

- Architecture index: [../README.md](../README.md)
- Interface contracts: [../contracts.md](../contracts.md)
- Specs index: [../../specs/README.md](../../specs/README.md)

