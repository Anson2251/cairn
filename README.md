# Cairn

A Rust-based backend service for [Trackmaker](https://github.com/Anson2251/trackmaker.git) that provides user authentication, route sharing, and cloud synchronization. It follows an **offline-first** design where local storage remains primary and cloud sync is opt-in.

## Features

- **User Authentication**: Email/password login, JWT-based sessions, OAuth support (Google, Apple)
- **Invite System**: Invitation-only registration for community control
- **Offline-First Sync**: Optimistic local-first architecture with cloud sync
- **Route Sharing**: Share sketches and routes with other users
- **Public Links**: Generate shareable public links for sketches
- **Export**: Export sketches in various formats (GPX, KML, GeoJSON)

## Tech Stack

- **Runtime**: Rust + Tokio
- **Web Framework**: Axum
- **Database**: PostgreSQL
- **Cache**: Redis
- **ORM**: SQLx (compile-time checked SQL)

## Getting Started

### Prerequisites

- Rust 1.75+
- PostgreSQL 14+
- Redis 7+

### Configuration

Copy `.env.example` to `.env` and configure:

```bash
cp .env.example .env
```

### Database Setup

```bash
# Create database (handled automatically on startup)
# Or manually:
psql -c "CREATE DATABASE cairn;"
```

### Run

```bash
cargo run
```

The server runs on `http://localhost:8080` by default.

## Project Structure

```
src/
├── main.rs              # Entry point
├── config.rs            # Configuration management
├── error.rs             # Error types
├── lib.rs               # Library root, router setup
├── auth/                # Authentication module
│   ├── handlers.rs      # Auth endpoints
│   ├── jwt.rs           # JWT management
│   ├── oauth.rs         # OAuth providers
│   └── types.rs         # Auth types
├── middleware/          # HTTP middleware
│   ├── auth.rs          # JWT validation
│   ├── admin.rs         # Admin checks
│   └── rate_limit.rs    # Rate limiting
├── sketches/            # Sketches module
├── routes/              # Routes module (GPS tracks)
├── sync/                # Cloud sync handlers
├── sharing/             # Sharing module
├── invite/              # Invite system
├── export/              # Export functionality
└── db/
    ├── mod.rs           # Database utilities
    └── migrations/      # SQL migrations
```

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/auth/register` | POST | Register with invite code |
| `/api/auth/login` | POST | Email/password login |
| `/api/auth/logout` | POST | Logout |
| `/api/auth/refresh` | POST | Refresh access token |
| `/api/auth/me` | GET | Get current user |
| `/api/sketches` | GET/POST | List/create sketches |
| `/api/sketches/{id}/routes` | GET/POST | List/create routes |
| `/api/sync/push` | POST | Push local changes |
| `/api/sync/pull` | POST | Pull remote changes |
| `/api/sketches/{id}/shares` | POST | Share sketch |
| `/api/public/{token}` | GET | Access public sketch |

## Architecture

See [ARCHITECTURE.md](./ARCHITECTURE.md) for detailed architecture documentation including:
- System architecture diagrams
- Database schema
- Authentication flows
- Sync flow with conflict resolution
- Security layers

## Development

### Run Tests

```bash
cargo test
```

### Lint

```bash
cargo clippy
```

## License

GPL-3.0
