# ADR 0003: Authentication / IdP

## Status
- Accepted

## Decision
- Use **AWS Cognito/SSO** as the default identity provider (OIDC/SAML), with the option to swap in another OIDC/SAML provider if needed.

## Why
- Native AWS integration; meets SOC2-friendly controls; supports OIDC/SAML; manageable via Terraform.
- Keeps authn centralized and pluggable.

## Consequences
- Provision Cognito/SSO via Terraform; integrate with APIs/UI; enforce role/tenant-scoped authz and audit logging.
- If swapping providers later, maintain OIDC/SAML compatibility to minimize app changes.

## Open Questions
- None currently.
