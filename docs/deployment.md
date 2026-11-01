# Deployment

`compose.full.yaml` runs PostgreSQL, Restate, Northwind, and the API on one machine. `just up` keeps the API on the host for development. `just up-full` builds the API image and does not delete volumes.

Production startup rejects `OPERATOR_DEV` and requires distinct report, session, and workflow keys. Those keys are derived from `SIGNING_KEY_FILE`. A public origin must be HTTPS. The operator login in this repository is the loopback development login. A real OIDC issuer, client secret, and allowlisted subjects are not configured here because no identity-provider credentials were available.

No public hostname was created. Publishing the demo still needs an authorized host, TLS, a managed database, a private Restate journal, and the OIDC provider. Do not point a public DNS name at the local compose stack.
