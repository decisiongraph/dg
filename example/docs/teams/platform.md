The Platform Team builds and maintains shared infrastructure, CI/CD pipelines, and developer tooling. We own the deployment platform and internal services that other teams depend on.

## Charter

The Platform Team exists to provide reliable, scalable infrastructure and developer tooling that accelerates product delivery. We own:

- **Deployment platform** — Kubernetes clusters, CI/CD pipelines, container registry
- **Internal services** — shared libraries, service mesh, observability stack
- **Developer experience** — local dev tooling, documentation, onboarding automation

We do NOT own application-level services or product features. Our boundary is the platform layer — we provide the tools, other teams build on top.

## Communication

| Channel | Purpose |
|---------|---------|
| #platform | General discussion |
| #platform-alerts | Monitoring alerts |
| #platform-support | Support requests from other teams |

## On-Call

- **Rotation**: Weekly, managed in PagerDuty
- **Escalation**: On-call engineer → team lead → VP Engineering
- **Runbooks**: See `docs/runbooks/` in the monorepo

## Getting Started

1. Clone the [monorepo](https://github.com/example/monorepo) and run `make setup`
2. Install Docker Desktop and authenticate with our container registry
3. Request access to the staging Kubernetes cluster via IT ticket
4. Set up your local `.env` from the template: `cp .env.example .env`

## Processes

- **PR review**: At least one approval required, prefer team members as reviewers
- **Deploy**: Merge to main triggers staging deploy; production requires manual promotion
- **RFCs**: For changes affecting multiple teams, create an ADR first

## Key Contacts

- **VP Engineering** — escalation path for infrastructure incidents
- **Security Team** — coordinate on infrastructure hardening and compliance
- **Product Team** — prioritize developer experience improvements

## External Services

| Service | Purpose | Access |
|---------|---------|--------|
| AWS Console | Infrastructure | SSO via Okta |
| Datadog | Monitoring & alerting | Request in #platform-access |
| PagerDuty | On-call rotations | Added by team lead |
| GitHub | Source control | Org invite on day 1 |
