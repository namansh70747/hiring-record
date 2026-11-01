# Reference register

Observed on 1 November 2026. Our stack is not their stack.

| Observed | Where | Our choice |
|---|---|---|
| Marketing site is HTML and CSS. `platform-3.css` says “Platform 3.0: the Daylight design. Dark hero and header, light body.” `body.p3` is `#fff` with text `#0a2540`. Header stays dark. Ember accent in `styles.css` is `#ec754a`. Type is Inter and IBM Plex Mono. | https://lantern.md/platform-3.css and https://lantern.md/styles.css | Hiring Record will use that dark-to-light rhythm. Light body tokens in the blueprint (`#F5F3EE`, `#14202B`) were not in the live CSS. The implementation will use `#fff` and `#0a2540` unless a later design pass chooses our own values and marks them as ours. |
| Public API is OpenAPI 3.1. Paths: `GET /v1/auth/validate`, `POST /v1/auth/revoke`, `GET /v1/me`, `GET /v1/workspaces`, `GET /v1/jobs/definitions`, `GET /v1/jobs`, `GET /v1/jobs/{id}`. The job tag says “Read-only for now.” A `lantern:write` scope is advertised. Lists “may lag.” | https://api.lantern.md/openapi.json | Our `/v1` commands are our contract. Northwind, not their API, will be the lagging system of record. |
| No public product repository was found. | Search on 1 Nov 2026 | We do not clone one. |
| `app.lantern.md` is a separate login shell. | https://app.lantern.md/ | Our app will be our own shell. |

`R-REJECTS-MET` means a criterion every reject also met cannot be reported as having predicted the hires. The blueprint table also says “claimed discrimination.” That word is not the test. The test is the prediction rule.
