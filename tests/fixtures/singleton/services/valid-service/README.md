# Valid Service

A valid service with all required sections.

## Status

Live

## Owner

@alice

## Architecture

```d2
client -> api: HTTP
api -> db: SQL
```

Service architecture diagram.

## Dependencies

### Internal

- auth-service: User authentication
- logging-service: Centralized logging

### External

| Service | Purpose | ADR |
|---------|---------|-----|
| PostgreSQL | Primary database | ADR-001 |
| Redis | Caching layer | ADR-002 |

## API

API documentation: https://api.example.com/docs

## Data

- **Database**: PostgreSQL 14
- **Schemas**: public, audit
- **Key tables**: users, sessions

## Deployment

Deployed via GitHub Actions to AWS ECS.

## Development

```bash
npm install
npm run dev
```

## Troubleshooting

### Database connection issues

Check DATABASE_URL environment variable.

## Decisions

- ADR-001: Database selection
- ADR-002: Caching strategy
