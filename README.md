# Cairn

A Rust-based backend service for [Trackmaker](https://github.com/Anson2251/trackmaker.git) that provides user authentication, route sharing, and cloud synchronization. It follows an **offline-first** design where local storage remains primary and cloud sync is opt-in.

## Features

- **User Authentication**: Email/password login, JWT-based sessions
- **Invite System**: Invitation-only registration for alpha control
- **Offline-First Sync**: Optimistic local-first architecture with cloud sync
- **Route Sharing**: Share sketches with other users (view/edit access)
- **Public Links**: Generate read-only public links for sketches
- **Data Export**: Export all data as GeoJSON
- **Asset Storage**: Upload images/attachments (stored in DB)
- **Trailblazers**: Public list of alpha pioneers

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

### Authentication

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/auth/register` | POST | Register with email/password + invite code |
| `/api/auth/login` | POST | Email/password login |
| `/api/auth/logout` | POST | Logout (revoke refresh token) |
| `/api/auth/refresh` | POST | Refresh access token |
| `/api/auth/me` | GET | Get current user profile |
| `/api/auth/me` | PUT | Update profile (username, avatar) |

### Sketches (Protected)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/sketches` | GET | List user's sketches (paginated) |
| `/api/sketches` | POST | Create new sketch |
| `/api/sketches/shared` | GET | List sketches shared with me |
| `/api/sketches/{id}` | GET | Get sketch details |
| `/api/sketches/{id}` | PUT | Update sketch |
| `/api/sketches/{id}` | DELETE | Delete sketch (soft delete) |

### Routes (Protected)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/sketches/{id}/routes` | GET | List routes in sketch |
| `/api/sketches/{id}/routes` | POST | Create route |
| `/api/routes/{id}` | GET | Get route details |
| `/api/routes/{id}` | PUT | Update route |
| `/api/routes/{id}` | DELETE | Delete route (soft delete) |

### Sync (Protected)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/sync/push` | POST | Push local changes to server |
| `/api/sync/pull` | POST | Pull remote changes |
| `/api/sync/resolve/{route_id}` | POST | Resolve conflict |

### Sharing (Protected)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/sketches/{id}/shares` | GET | List shares |
| `/api/sketches/{id}/shares` | POST | Share with user |
| `/api/sketches/{id}/shares/{user_id}` | PUT | Update access level |
| `/api/sketches/{id}/shares/{user_id}` | DELETE | Revoke share |
| `/api/sketches/{id}/public-link` | POST | Create public link |
| `/api/sketches/{id}/public-link` | DELETE | Revoke public link |

### Assets (Protected)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/assets` | POST | Upload file (multipart, max 10MB) |
| `/assets/{hash}.{ext}` | GET | Download file |

### Export (Protected)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/export` | POST | Export all user data (GeoJSON) |

### Invite

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/invite/{code}/validate` | GET | Validate invite code |

### Public

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/public/{token}` | GET | Access public sketch (no auth) |
| `/api/trailblazers` | GET | List alpha pioneers |

### Admin (Protected)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/admin/invites` | GET | List invite codes |
| `/api/admin/invites` | POST | Generate invite codes |
| `/api/admin/invites/{id}` | DELETE | Revoke invite code |

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
