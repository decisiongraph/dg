---
dev_url: http://localhost:3000
---

# {SERVICE_NAME}

<!-- Brief description of what this service does -->

## Status

<!-- One of: Live, Beta, Sunset, Deprecated, Planned -->
Live

## Owner

<!-- Service owner (@handle from org.kdl) -->
@handle

## SLOs

| Metric | Target | Current |
|--------|--------|---------|
| p99 Latency | < 200ms | 150ms |
| Availability | 99.9% | 99.95% |
| Error Rate | < 0.1% | 0.05% |

## Runbooks

- [Production Deployment Runbook](https://runbooks.acme.com/deploy)
- [Incident Response](https://runbooks.acme.com/incident)
- [On-Call Procedures](https://pagerduty.com/service/{SERVICE_NAME})

## Troubleshooting

### High Latency

1. Check database connection pool
2. Review cache hit rate
3. Examine slow query log

### Service Unavailable

1. Verify health check endpoint
2. Check deployment status
3. Review recent changes

## Deployment

Deployed via CI/CD pipeline to production.

**Environments:**
- Production: `prod.example.com`
- Staging: `staging.example.com`

## Architecture

<!-- Service-level architecture diagram (D2, Mermaid, or ASCII) -->

```d2
# Add your architecture diagram here
```

## Dependencies

### Internal

<!-- List of internal services this service depends on -->
- service-name: brief description of dependency

### External

<!-- External dependencies table -->
| Service | Purpose | ADR |
|---------|---------|-----|
| PostgreSQL | Primary database | ADR-001 |

## API

<!-- Link to API documentation or explicit no-API statement -->

This service does not expose an API.

<!-- OR: Link to OpenAPI spec, GraphQL schema, etc. -->

## Data

<!-- Database and data storage information -->

- **Database**: PostgreSQL
- **Schemas**: `public`, `events`
- **Key tables**: users, sessions, audit_log

## Development

<!-- Local development setup -->

```bash
# Install dependencies
bundle install

# Setup database
rails db:setup

# Run tests
bundle exec rspec

# Run linter
bundle exec rubocop

# Start server
rails server
```

<!-- Add code reference comments in source files linking to decision docs:
     // OPP-001: User authentication flow
     // SPEC-001: Login validation rules
-->

## Decisions

<!-- Architecture decisions for this service (ADR references) -->
- ADR-001: Database selection
- ADR-002: Authentication strategy
