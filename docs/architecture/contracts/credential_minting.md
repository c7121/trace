# Credential minting (`/v1/task/credentials`)

To keep untrusted tasks near-zero-permission, the Dispatcher provides a credential minting endpoint.
It exchanges a task capability token for short-lived AWS credentials scoped to the task allowed object-store prefixes.

This is the v1 replacement for a separate credential broker service.

## API

```
POST /v1/task/credentials
X-Trace-Task-Capability: <capability_token>
```

This endpoint is internal-only. It is reachable only from within the VPC (workers and `runtime: lambda`) and must not be routed through the public Gateway.

- The capability token is issued per `(task_id, attempt)` and defines allowed input and output prefixes.
- Dispatcher calls `sts:AssumeRole` with a session policy derived from the token.
- Returned credentials are short-lived and allow only object-store access within the encoded prefixes.

## Scope derivation and canonicalization (required)

Credential minting is a privilege boundary. The Dispatcher MUST derive the STS session policy from the capability token using deny-by-default rules.

Rules (v1):
- The token MUST encode allowed prefixes as canonical `s3://bucket/prefix/` strings.
- Prefixes MUST be normalized before policy generation:
  - scheme must be `s3`
  - bucket must be non-empty
  - prefix must be non-empty and must not contain `..`
  - wildcards (`*`, `?`) are forbidden
  - prefix must be treated as a directory prefix (effectively `prefix/*`), never as a starts-with anything pattern
- The resulting session policy MUST grant only the minimum required S3 actions within those prefixes.
  - Prefer object-level access (`GetObject` and `PutObject`) over bucket-level actions.
  - If `ListBucket` is required, constrain it with an `s3:prefix` condition to the allowed prefixes only.

Defense-in-depth (recommended):
- Enforce the same prefix constraints at the bucket policy layer so that even a buggy session policy cannot read or write outside allowed prefixes.

Example (illustrative) session policy shape:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": ["s3:GetObject"],
      "Resource": ["arn:aws:s3:::<bucket>/<read_prefix>*"]
    },
    {
      "Effect": "Allow",
      "Action": ["s3:PutObject"],
      "Resource": ["arn:aws:s3:::<bucket>/<write_prefix>*"]
    },
    {
      "Effect": "Allow",
      "Action": ["s3:ListBucket"],
      "Resource": ["arn:aws:s3:::<bucket>"],
      "Condition": {"StringLike": {"s3:prefix": ["<read_prefix>*", "<write_prefix>*"]}}
    }
  ]
}
```

Verification (required):
- Unit tests for prefix normalization and canonicalization.
- Negative tests: `..`, empty prefixes, wildcard widening, wrong bucket.
- AWS profile: integration test that minted credentials cannot read or write outside scope.

Networking: Dispatcher must be able to reach AWS STS. Prefer an STS VPC endpoint.

## Related

- Task capability token: [task_capability_tokens.md](task_capability_tokens.md)
- Task-scoped endpoints: [task_scoped_endpoints.md](task_scoped_endpoints.md)

