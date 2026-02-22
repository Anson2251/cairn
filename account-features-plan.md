# Account & Sharing Feature Implementation Plan (Revised)

## Overview

The backend service is named **Cairn** — a stack of stones that marks the path for travelers, fitting for a service that stores and syncs routes between users.

This document outlines the implementation plan for adding account features (user authentication, route/sketch sharing, cloud sync) to Trackmaker. The design follows an **offline-first** approach: local storage remains the primary data source, cloud sync is opt-in.

### Design Principles

1. **Offline-first**: The app must work fully without a network connection
2. **Granular data model**: Routes are the atomic unit of sync and sharing, not entire sketches
3. **Honest simplicity**: No real-time collaborative editing in the initial release — it's a fundamentally different problem that would be addressed separately if needed
4. **Alpha-gated**: Registration requires an invite code during the alpha phase

---

## Technology Stack

### Backend
- **Language**: Rust
- **Framework**: Axum (async, lightweight, Tauri-compatible)
- **Database**: PostgreSQL (robust, supports relationships and spatial indexing)
- **Cache/Session**: Redis (token blacklist, rate limiting)
- **Authentication**: JWT (short-lived) + Refresh Tokens (HttpOnly cookie / Tauri secure storage)
- **Async Runtime**: Tokio

### Client
- **Framework**: Vue 3 + Pinia (existing)
- **UI Components**: Naive UI (existing)
- **Storage**: Extended platform abstraction layer

---

## Database Schema

### Users Table

```sql
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) UNIQUE NOT NULL,
    email_verified BOOLEAN DEFAULT FALSE,
    username VARCHAR(50) UNIQUE NOT NULL,
    hashed_password TEXT,                           -- NULL if OAuth-only
    avatar_url TEXT,
    role VARCHAR(20) NOT NULL DEFAULT 'user',       -- 'user', 'admin'
    settings JSONB DEFAULT '{}',
    invite_code_id UUID REFERENCES invite_codes(id),
    trailblazer_seq INTEGER,                        -- alpha user sequence number, NULL for post-alpha users
    deleted_at TIMESTAMP WITH TIME ZONE,            -- soft delete
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_active ON users(id) WHERE deleted_at IS NULL;
```

### OAuth Accounts Table

```sql
-- Separate table: users can link multiple OAuth providers
CREATE TABLE user_oauth_accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider VARCHAR(20) NOT NULL,                  -- 'google', 'github'
    provider_id VARCHAR(100) NOT NULL,
    provider_email VARCHAR(255),
    access_token TEXT,                              -- encrypted, for API calls if needed
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    UNIQUE(provider, provider_id),                  -- one provider account links to one user
    UNIQUE(user_id, provider)                       -- one user has one account per provider
);

CREATE INDEX idx_oauth_user ON user_oauth_accounts(user_id);
```

### Invite Codes Table

```sql
CREATE TABLE invite_codes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sequence INTEGER UNIQUE NOT NULL,               -- 1, 2, 3... global incrementing
    code VARCHAR(64) UNIQUE NOT NULL,               -- CAIRN-003-mist-plateau
    cairn_name VARCHAR(64) NOT NULL,                -- mist-plateau (permanent trailblazer identity)
    origin_coord POINT,                             -- easter egg coordinate
    memo TEXT,                                      -- admin's private note: "for Alice"
    used BOOLEAN DEFAULT FALSE,
    used_by UUID REFERENCES users(id),
    used_at TIMESTAMP WITH TIME ZONE,
    expires_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_invite_code ON invite_codes(code);
```

### Sketches Table

```sql
-- A sketch is a container/folder for routes.
-- It holds metadata only — no route data blob.
CREATE TABLE sketches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    is_public BOOLEAN DEFAULT FALSE,
    deleted_at TIMESTAMP WITH TIME ZONE,            -- soft delete
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_sketches_user ON sketches(user_id);
CREATE INDEX idx_sketches_public ON sketches(is_public) WHERE is_public = TRUE;
CREATE INDEX idx_sketches_active ON sketches(id) WHERE deleted_at IS NULL;
```

### Routes Table

```sql
-- Routes are the atomic unit of sync, sharing, and conflict detection.
-- Each route has its own version counter.
CREATE TABLE routes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sketch_id UUID NOT NULL REFERENCES sketches(id),
    name VARCHAR(255),
    description TEXT,
    geojson JSONB NOT NULL,                         -- GeoJSON FeatureCollection
    metadata JSONB DEFAULT '{}',                    -- distance, tags, activity type, etc.
    notes TEXT,                                     -- markdown notes
    version INTEGER NOT NULL DEFAULT 1,             -- incremented on each server-side update
    deleted_at TIMESTAMP WITH TIME ZONE,            -- soft delete
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_routes_sketch ON routes(sketch_id);
CREATE INDEX idx_routes_active ON routes(id) WHERE deleted_at IS NULL;
```

### Shares Table

```sql
CREATE TABLE shares (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sketch_id UUID NOT NULL REFERENCES sketches(id),
    user_id UUID NOT NULL REFERENCES users(id),     -- the recipient
    access_level VARCHAR(20) NOT NULL DEFAULT 'view', -- 'view', 'edit'
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_by UUID NOT NULL REFERENCES users(id),  -- the sharer

    UNIQUE(sketch_id, user_id)                      -- one permission entry per user per sketch
);

CREATE INDEX idx_shares_user ON shares(user_id);
```

### Public Links Table

```sql
-- Separated from shares: public links are anonymous access, different concern
CREATE TABLE public_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sketch_id UUID NOT NULL REFERENCES sketches(id),
    token VARCHAR(64) UNIQUE NOT NULL,
    access_level VARCHAR(20) NOT NULL DEFAULT 'view', -- 'view' only for public links
    expires_at TIMESTAMP WITH TIME ZONE,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_public_links_token ON public_links(token);
```

### Sync Log Table

```sql
-- Records every sync event for audit and debugging.
-- NOT for real-time OT — just a history log.
CREATE TABLE sync_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    route_id UUID NOT NULL REFERENCES routes(id),
    user_id UUID NOT NULL REFERENCES users(id),
    action VARCHAR(20) NOT NULL,                    -- 'push', 'pull', 'conflict_resolve'
    version_before INTEGER NOT NULL,
    version_after INTEGER NOT NULL,
    resolution VARCHAR(20),                         -- 'keep_local', 'keep_remote', 'merge', NULL
    client_id UUID NOT NULL,                        -- identifies device/browser instance
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_sync_log_route ON sync_log(route_id);
```

### Refresh Tokens Table

```sql
-- Server-side refresh token tracking for revocation
CREATE TABLE refresh_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash VARCHAR(64) UNIQUE NOT NULL,         -- SHA-256 of the actual token
    client_id UUID NOT NULL,                        -- device identifier
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    revoked BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_refresh_tokens_user ON refresh_tokens(user_id);
```

### Assets Table

```sql
-- User uploaded files (images, attachments)
CREATE TABLE assets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    original_filename VARCHAR(255),
    mime_type VARCHAR(100) NOT NULL,
    size INTEGER NOT NULL,
    hash VARCHAR(64) NOT NULL,                    -- SHA-256 hash for deduplication
    data BYTEA NOT NULL,                          -- file content stored in DB
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_assets_owner ON assets(owner_id);
CREATE INDEX idx_assets_hash ON assets(hash);
CREATE UNIQUE INDEX idx_assets_hash_unique ON assets(hash, owner_id);
```

---

## API Endpoints

### Authentication

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/auth/register` | Register with email/password + invite code |
| POST | `/api/auth/login` | Login with email/password |
| POST | `/api/auth/logout` | Revoke current refresh token (client deletes access token) |
| POST | `/api/auth/refresh` | Exchange refresh token for new access token |
| GET | `/api/auth/me` | Get current user profile |
| PUT | `/api/auth/me` | Update profile (username, avatar) |
| POST | `/api/auth/forgot-password` | Send password reset email |
| POST | `/api/auth/reset-password` | Reset password with token |
| POST | `/api/auth/verify-email` | Verify email with token |
| GET | `/api/auth/oauth/{provider}` | Initiate OAuth flow |
| GET | `/api/auth/oauth/{provider}/callback` | OAuth callback |
| POST | `/api/auth/oauth/{provider}/link` | Link OAuth to existing account |
| DELETE | `/api/auth/oauth/{provider}/unlink` | Unlink OAuth from account |

### Sketches (Protected)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/sketches?page=1&per_page=20` | List user's own sketches (paginated) |
| GET | `/api/sketches/shared` | List sketches shared with me (paginated) |
| POST | `/api/sketches` | Create new sketch |
| GET | `/api/sketches/{id}` | Get sketch metadata + route list |
| PUT | `/api/sketches/{id}` | Update sketch metadata |
| DELETE | `/api/sketches/{id}` | Soft-delete sketch |

### Routes (Protected)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/sketches/{id}/routes` | List routes in a sketch |
| POST | `/api/sketches/{id}/routes` | Create route |
| GET | `/api/routes/{id}` | Get single route with full data |
| PUT | `/api/routes/{id}` | Update route (increments version) |
| DELETE | `/api/routes/{id}` | Soft-delete route |

### Sync (Protected)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/sync/push` | Push local changes (batch of routes) |
| POST | `/api/sync/pull` | Pull remote changes since last sync |
| POST | `/api/sync/resolve/{route_id}` | Resolve a conflict for one route |

### Assets (Protected)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/assets` | Upload file (multipart, max 10MB), returns URL |
| GET | `/assets/{hash}.{ext}` | Download file (requires auth, permission check) |

### Sharing (Protected)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/sketches/{id}/shares` | Share sketch with a user |
| GET | `/api/sketches/{id}/shares` | List who this sketch is shared with |
| PUT | `/api/sketches/{id}/shares/{user_id}` | Update access level |
| DELETE | `/api/sketches/{id}/shares/{user_id}` | Revoke share |
| POST | `/api/sketches/{id}/public-link` | Create read-only public link |
| DELETE | `/api/sketches/{id}/public-link` | Revoke public link |
| GET | `/api/public/{token}` | Access public sketch (no auth required) |

### Invite (Admin-only except validate)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/admin/invites` | Generate invite codes |
| GET | `/api/admin/invites` | List all invite codes with status |
| DELETE | `/api/admin/invites/{id}` | Revoke unused invite code |
| GET | `/api/invite/{code}/validate` | Check if code is valid (public) |

### Trailblazers (Public)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/trailblazers` | List alpha pioneers (sequence + cairn_name + date only) |

### Data Export (Protected)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/export` | Request full data export (async, returns job ID) |
| GET | `/api/export/{job_id}` | Download export when ready (ZIP of GeoJSON + metadata) |

---

## Sync Strategy

### Core Decision: Manual Conflict Resolution at Route Granularity

The sync model is **not** Last-Write-Wins. It is **optimistic locking with manual resolution**, operating at the **individual route** level (not the entire sketch).

### Why Route-Level Granularity

```
Scenario: Alice and Bob both have access to sketch "Weekend Hikes"
         containing Route A and Route B.

  Alice edits Route A → syncs fine
  Bob edits Route B → syncs fine
  Both edit Route A → CONFLICT (true conflict, needs resolution)
```

### Push Flow

```
Client                              Server
  │                                    │
  │  POST /api/sync/push               │
  │  {                                 │
  │    client_id: "...",               │
  │    changes: [                      │
  │      {                             │
  │        route_id: "...",            │
  │        base_version: 3,            │
  │        data: { geojson, meta },    │
  │      },                            │
  │      ...                           │
  │    ]                               │
  │  }                                 │
  │ ──────────────────────────────────►│
  │                                    │  For each route:
  │                                    │    if server_version == base_version:
  │                                    │      accept, set version = base_version + 1
  │                                    │    else:
  │                                    │      mark as conflict
  │                                    │
  │  {                                 │
  │    accepted: ["route-1"],          │
  │    conflicts: [                    │
  │      {                             │
  │        route_id: "route-2",        │
  │        local_version: 3,           │
  │        server_version: 5,          │
  │        server_data: { ... },       │
  │      }                             │
  │    ]                               │
  │  }                                 │
  │ ◄──────────────────────────────────│
  │                                    │
  │  (if conflicts: show UI)           │
  │                                    │
  │  POST /api/sync/resolve/route-2    │
  │  {                                 │
  │    resolution: "keep_local",       │
  │    base_version: 5,               │
  │    data: { ... }                   │
  │  }                                 │
  │ ──────────────────────────────────►│
```

### Pull Flow

```
Client                              Server
  │                                    │
  │  POST /api/sync/pull               │
  │  {                                 │
  │    client_id: "...",               │
  │    last_synced_at: "2025-01-20T..",│
  │    known_versions: {               │
  │      "route-1": 3,                │
  │      "route-2": 5,                │
  │    }                               │
  │  }                                 │
  │ ──────────────────────────────────►│
  │                                    │
  │  {                                 │
  │    updated: [                      │
  │      { route_id, version, data },  │
  │    ],                              │
  │    deleted: ["route-3"],           │
  │    server_time: "2025-01-21T..."   │
  │  }                                 │
  │ ◄──────────────────────────────────│
```

### Sync Triggers

| Trigger | Behavior |
|---------|----------|
| App launch / comes online | Pull remote changes |
| User saves a route | Push after 5-second debounce |
| User manually clicks "Sync" button | Full push + pull |
| App goes to background / page unload | Push pending changes (best-effort via `navigator.sendBeacon` or Tauri background task) |
| Periodic | Pull every 5 minutes if app is idle and online |

### Offline Queue

When the client is offline:

1. All edits are saved to local IndexedDB as normal
2. Each edit is also appended to a `pending_changes` queue in local storage
3. When connectivity is restored:
   - Queue is replayed **in order** against the server
   - Each push may return conflicts
   - Conflicts are collected and shown to the user **once**, after the queue is drained
4. Queue items are removed only after server confirms acceptance

```typescript
interface PendingChange {
    id: string;                    // local UUID
    routeId: string;
    baseVersion: number;           // version at time of local edit
    data: RouteData;
    timestamp: number;             // local timestamp
    status: 'pending' | 'pushing' | 'conflict' | 'accepted';
}
```

---

## Authentication Details

### Token Strategy

```
Access Token (JWT):
  - Stored: memory (Vue ref) — lost on page refresh, re-obtained via refresh token
  - Lifetime: 15 minutes
  - Contains: user_id, role, email_verified
  - Stateless verification (no DB lookup)

Refresh Token:
  - Stored:
      Web → HttpOnly, Secure, SameSite=Strict cookie
      Tauri → platform secure storage (Keychain / Credential Manager)
  - Lifetime: 7 days
  - Server-side tracking in refresh_tokens table for revocation
  - Rotated on each use (old token revoked, new token issued)
```

### Logout

Logout is a **server-side revocation** of the refresh token:

```rust
pub async fn logout(
    State(state): State<AppState>,
    claims: AuthClaims,
    cookie_jar: CookieJar,
) -> Result<CookieJar, AppError> {
    // 1. Extract refresh token from cookie
    let refresh_token = cookie_jar
        .get("refresh_token")
        .ok_or(AppError::NoRefreshToken)?;

    // 2. Revoke in database
    let token_hash = sha256(refresh_token.value());
    sqlx::query!(
        "UPDATE refresh_tokens SET revoked = TRUE WHERE token_hash = $1",
        token_hash
    )
    .execute(&state.db)
    .await?;

    // 3. Clear cookie
    Ok(cookie_jar.remove(Cookie::named("refresh_token")))
    // Client also discards the in-memory access token
}
```

> **Note**: The access token remains technically valid until its 15-minute expiry. This is an accepted trade-off of JWT-based auth. For immediate invalidation of compromised tokens, use the Redis token blacklist (checked in auth middleware).

### Rate Limiting

| Endpoint | Limit |
|----------|-------|
| `POST /api/auth/login` | 5 per minute per IP |
| `POST /api/auth/register` | 3 per hour per IP |
| `POST /api/auth/forgot-password` | 3 per hour per email |
| `GET /api/invite/{code}/validate` | 10 per minute per IP |
| All other authenticated endpoints | 60 per minute per user |

Implemented via Redis sliding window counter in Axum middleware.

### Registration Flow (with Invite Code)

```rust
pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, AppError> {
    // 1. Check invite requirement
    if state.config.require_invite {
        let invite = sqlx::query_as!(InviteRow,
            "SELECT * FROM invite_codes WHERE code = $1",
            body.invite_code.as_deref().unwrap_or("")
        )
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::InvalidInviteCode)?;

        if invite.used {
            return Err(AppError::InviteCodeAlreadyUsed);
        }
        if invite.expires_at.map_or(false, |e| e < Utc::now()) {
            return Err(AppError::InviteCodeExpired);
        }
    }

    // 2. Validate input
    validate_email(&body.email)?;
    validate_username(&body.username)?;
    validate_password(&body.password)?;

    // 3. Create user
    let hashed = bcrypt::hash(&body.password, 12)?;
    let user = sqlx::query_as!(UserRow,
        r#"INSERT INTO users (email, username, hashed_password, invite_code_id, trailblazer_seq)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING *"#,
        body.email,
        body.username,
        hashed,
        invite.as_ref().map(|i| i.id),
        invite.as_ref().map(|i| i.sequence),
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        // handle unique constraint violations
        sqlx::Error::Database(ref db_err) if db_err.constraint() == Some("users_email_key") =>
            AppError::EmailTaken,
        sqlx::Error::Database(ref db_err) if db_err.constraint() == Some("users_username_key") =>
            AppError::UsernameTaken,
        _ => AppError::Internal(e.into()),
    })?;

    // 4. Mark invite as used
    if let Some(invite) = &invite {
        sqlx::query!(
            "UPDATE invite_codes SET used = TRUE, used_by = $1, used_at = NOW() WHERE id = $2",
            user.id, invite.id
        )
        .execute(&state.db)
        .await?;
    }

    // 5. Send verification email (async, don't block response)
    //    During alpha, email verification is optional but encouraged
    tokio::spawn(send_verification_email(user.email.clone(), state.mailer.clone()));

    // 6. Issue tokens
    let access_token = issue_access_token(&user, &state.config)?;
    let refresh_token = issue_and_store_refresh_token(&user, &body.client_id, &state).await?;

    Ok(Json(RegisterResponse {
        user: user.into(),
        access_token,
        // refresh_token is set as HttpOnly cookie, not in JSON body
        trailblazer: invite.as_ref().map(|i| TrailblazerInfo {
            sequence: i.sequence,
            cairn_name: i.cairn_name.clone(),
            origin_coord: i.origin_coord,
        }),
    }))
}
```

---

## Invite Code Generation

```rust
// backend/src/invite/generator.rs

use sha2::{Sha256, Digest};

const TERRAIN: &[&str] = &[
    "ridge", "glacier", "canyon", "summit", "meadow",
    "tundra", "delta", "plateau", "fjord", "ravine",
    "crater", "basin", "cape", "moraine", "col",
];

const WEATHER: &[&str] = &[
    "dawn", "dusk", "frost", "gale", "mist",
    "ember", "aurora", "solstice", "moonrise", "haze",
];

/// Deterministic generation: same sequence + salt always produces the same code.
/// Cannot reverse-engineer sequence from code due to salt.
pub fn generate_invite_code(sequence: i32, salt: &str) -> InviteCodeData {
    let seed = format!("{salt}:{sequence}");
    let hash = Sha256::digest(seed.as_bytes());
    let h = hash.as_slice();

    let weather = WEATHER[h[0] as usize % WEATHER.len()];
    let terrain = TERRAIN[h[1] as usize % TERRAIN.len()];
    let cairn_name = format!("{weather}-{terrain}");
    let code = format!("CAIRN-{sequence:03}-{cairn_name}");

    // Map hash bytes to a real geographic coordinate (easter egg)
    let lat = (u16::from_be_bytes([h[2], h[3]]) as f64 / 65535.0) * 180.0 - 90.0;
    let lng = (u16::from_be_bytes([h[4], h[5]]) as f64 / 65535.0) * 360.0 - 180.0;

    InviteCodeData {
        sequence,
        code,
        cairn_name,
        origin_coord: (lng, lat),
    }
}
```

---

## Client Architecture

### Stores

#### User Store (`user-store.ts`)

```typescript
interface UserState {
    user: User | null;
    accessToken: string | null;         // in-memory only, never persisted
    isAuthenticated: boolean;
    isLoading: boolean;
}

interface User {
    id: string;
    email: string;
    emailVerified: boolean;
    username: string;
    avatarUrl?: string;
    role: 'user' | 'admin';
    settings: UserSettings;
    trailblazerSeq: number | null;      // null for post-alpha users
    cairnName: string | null;
    originCoord: [number, number] | null;
}
```

#### Sync Store (`sync-store.ts`)

```typescript
interface SyncState {
    status: 'idle' | 'pushing' | 'pulling' | 'error' | 'offline';
    lastSyncedAt: number | null;
    pendingChanges: PendingChange[];
    conflicts: ConflictInfo[];
    isOnline: boolean;
}

interface PendingChange {
    id: string;
    routeId: string;
    baseVersion: number;
    data: RouteData;
    timestamp: number;
    status: 'pending' | 'pushing' | 'conflict' | 'accepted';
}

interface ConflictInfo {
    routeId: string;
    routeName: string;
    localVersion: number;
    localData: RouteData;
    serverVersion: number;
    serverData: RouteData;
    localEditedAt: number;
    serverEditedAt: number;
}
```

#### Sharing Store (`sharing-store.ts`)

```typescript
interface SharingState {
    sharedByMe: ShareInfo[];          // sketches I shared with others
    sharedWithMe: SharedSketch[];     // sketches others shared with me
    publicLinks: PublicLinkInfo[];
}

interface SharedSketch {
    sketchId: string;
    sketchName: string;
    ownerUsername: string;
    accessLevel: 'view' | 'edit';
    sharedAt: number;
}
```

### UI Components

#### New Components

```
src/components/
├── auth-dialog.vue              # Login/register with invite code field
├── trailblazer-welcome.vue      # Post-registration ceremony page
├── trailblazer-badge.vue        # Profile badge for alpha users
├── user-menu.vue                # Header dropdown: profile, settings, logout
├── share-dialog.vue             # Share sketch with users
├── sync-status.vue              # Toolbar indicator: synced / syncing / offline / conflict
├── conflict-resolver.vue        # Side-by-side diff for route conflicts
├── migration-dialog.vue         # First-time local→cloud data migration wizard
└── export-dialog.vue            # Data export request UI
```

#### Modified Components

```
src/views/
├── JoinView.vue                 # /join — invite code entry
├── TrailblazersView.vue         # /trailblazers — public alpha pioneers page
├── ProfileView.vue              # /profile — user profile with trailblazer info
├── AuthCallbackView.vue         # /auth/callback — OAuth redirect handler

# Existing views modified:
├── SketchCentreView.vue         # + share button, sync status per route
├── TrackerView.vue              # + sync indicator in toolbar
└── SettingsView.vue             # + Account section, data export
```

### Local-to-Cloud Migration Flow

When a user with existing local data creates an account:

```
┌──────────────────────────────────────────────────────────┐
│                                                          │
│   📦 We found 12 sketches in your local storage.         │
│                                                          │
│   Would you like to upload them to your account?         │
│                                                          │
│   ┌────────────────────────────────────────────────┐     │
│   │ ☑ Weekend Hikes          (3 routes, 2.1 MB)   │     │
│   │ ☑ Tokyo Trip             (1 route,  0.4 MB)   │     │
│   │ ☑ Daily Commute          (8 routes, 5.3 MB)   │     │
│   │ ...                                           │     │
│   └────────────────────────────────────────────────┘     │
│                                                          │
│   [Upload Selected]    [Skip for Now]    [Don't Ask]     │
│                                                          │
│   Local data will be preserved regardless.               │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

Implementation:

```typescript
async function migrateLocalData(selectedSketchIds: string[]) {
    const localSketches = await localStore.getAllSketches();

    for (const sketch of localSketches) {
        if (!selectedSketchIds.includes(sketch.id)) continue;

        // 1. Create sketch on server (generates new server-side UUID)
        const serverSketch = await api.createSketch({
            name: sketch.name,
            description: sketch.description,
        });

        // 2. Upload each route
        for (const route of sketch.routes) {
            await api.createRoute(serverSketch.id, {
                name: route.name,
                geojson: route.geojson,
                metadata: route.metadata,
            });
        }

        // 3. Update local record with server ID mapping
        await localStore.setServerMapping(sketch.id, serverSketch.id);
    }
}
```

---

## Platform Layer Extensions

### ISyncProvider Interface

```typescript
interface ISyncProvider {
    init(): Promise<Result<void, AppError>>;

    // Auth
    authenticate(credentials: AuthCredentials): Promise<Result<AuthTokens, AppError>>;
    refreshToken(): Promise<Result<string, AppError>>;

    // Sync
    pushChanges(changes: PendingChange[]): Promise<Result<PushResult, AppError>>;
    pullChanges(since: number, knownVersions: Map<string, number>): Promise<Result<PullResult, AppError>>;
    resolveConflict(routeId: string, resolution: ConflictResolution): Promise<Result<void, AppError>>;

    // Status
    getStatus(): SyncStatus;
    isOnline(): boolean;
}
```

### Token Storage Abstraction

```typescript
interface ITokenStorage {
    getRefreshToken(): Promise<string | null>;
    setRefreshToken(token: string): Promise<void>;
    clearRefreshToken(): Promise<void>;
}

// Web: HttpOnly cookie (managed by browser, not JS)
class WebTokenStorage implements ITokenStorage {
    // Refresh token lives in HttpOnly cookie — JS cannot read it
    // Server sets/clears it via Set-Cookie header
    async getRefreshToken() { return null; } // not accessible from JS
    async setRefreshToken() { /* no-op, server handles via cookie */ }
    async clearRefreshToken() {
        await fetch('/api/auth/logout', { method: 'POST', credentials: 'include' });
    }
}

// Tauri: secure OS storage
class TauriTokenStorage implements ITokenStorage {
    async getRefreshToken() {
        return await invoke('plugin:store|get', { key: 'refresh_token' });
    }
    async setRefreshToken(token: string) {
        await invoke('plugin:store|set', { key: 'refresh_token', value: token });
    }
    async clearRefreshToken() {
        await invoke('plugin:store|delete', { key: 'refresh_token' });
    }
}
```

---

## Security Considerations

### Authentication

- JWT access tokens: 15-minute expiry, stored in memory only
- Refresh tokens: 7-day expiry, rotated on use, server-tracked for revocation
- Password hashing: bcrypt with cost factor 12
- Rate limiting via Redis sliding window
- Redis token blacklist for immediate access token revocation (emergency use)

### Authorization

- All endpoints verify ownership or share permissions
- Admin endpoints check `role = 'admin'` in JWT claims
- Public links are **read-only** — no write access through public tokens
- Share permissions checked at route level: sketch owner or `edit` access required for writes

### Data Protection

- HTTPS only in production (enforced via HSTS header)
- Input validation on all endpoints (length limits, format checks)
- SQL injection prevention via parameterized queries (sqlx compile-time checked)
- XSS protection via Content-Security-Policy headers
- CSRF protection: SameSite=Strict on cookies + Origin header validation
- Soft delete with 30-day retention before permanent purge

---

## File Structure

```
/trackmaker
├── backend/
│   ├── src/
│   │   ├── main.rs
│   │   ├── config.rs
│   │   ├── error.rs                    # AppError enum, error responses
│   │   ├── middleware/
│   │   │   ├── mod.rs
│   │   │   ├── auth.rs                 # JWT extraction + validation
│   │   │   ├── admin.rs                # Admin role check
│   │   │   └── rate_limit.rs           # Redis-based rate limiting
│   │   ├── db/
│   │   │   ├── mod.rs
│   │   │   └── migrations/
│   │   │       ├── 001_users.sql
│   │   │       ├── 002_oauth_accounts.sql
│   │   │       ├── 003_invite_codes.sql
│   │   │       ├── 004_sketches.sql
│   │   │       ├── 005_routes.sql
│   │   │       ├── 006_shares.sql
│   │   │       ├── 007_public_links.sql
│   │   │       ├── 008_sync_log.sql
│   │   │       └── 009_refresh_tokens.sql
│   │   ├── auth/
│   │   │   ├── mod.rs
│   │   │   ├── handlers.rs             # register, login, logout, refresh, password reset
│   │   │   ├── jwt.rs                  # Token issuance + validation
│   │   │   └── oauth.rs               # Google/GitHub OAuth flows
│   │   ├── invite/
│   │   │   ├── mod.rs
│   │   │   ├── generator.rs            # Deterministic code generation
│   │   │   └── handlers.rs            # Admin CRUD + public validate
│   │   ├── sketches/
│   │   │   ├── mod.rs
│   │   │   └── handlers.rs            # Sketch CRUD
│   │   ├── routes/
│   │   │   ├── mod.rs
│   │   │   └── handlers.rs            # Route CRUD
│   │   ├── sync/
│   │   │   ├── mod.rs
│   │   │   └── handlers.rs            # Push, pull, conflict resolution
│   │   ├── sharing/
│   │   │   ├── mod.rs
│   │   │   └── handlers.rs            # Share CRUD, public links
│   │   └── export/
│   │       ├── mod.rs
│   │       └── handlers.rs            # Data export (async job)
│   ├── Cargo.toml
│   └── .env.example
│
└── src/
    ├── libs/
    │   ├── platform/
    │   │   ├── providers/
    │   │   │   ├── tauri-sync-provider.ts
    │   │   │   ├── web-sync-provider.ts
    │   │   │   ├── tauri-token-storage.ts
    │   │   │   ├── web-token-storage.ts
    │   │   │   └── types.ts
    │   │   └── index.ts
    │   ├── sync/
    │   │   ├── index.ts
    │   │   ├── client.ts               # API client for sync endpoints
    │   │   ├── queue.ts                # Offline pending changes queue
    │   │   └── conflict-resolver.ts    # Conflict detection + resolution logic
    │   └── user/
    │       ├── index.ts
    │       ├── client.ts               # API client for auth/user endpoints
    │       └── types.ts
    ├── store/
    │   ├── user-store.ts
    │   ├── sync-store.ts
    │   └── sharing-store.ts
    ├── components/
    │   ├── auth-dialog.vue
    │   ├── trailblazer-welcome.vue
    │   ├── trailblazer-badge.vue
    │   ├── user-menu.vue
    │   ├── share-dialog.vue
    │   ├── sync-status.vue
    │   ├── conflict-resolver.vue
    │   ├── migration-dialog.vue
    │   └── export-dialog.vue
    ├── views/
    │   ├── JoinView.vue
    │   ├── TrailblazersView.vue
    │   ├── ProfileView.vue
    │   └── AuthCallbackView.vue
    └── router/
        └── index.ts
```

---

## Environment Variables

### Backend (.env)

```env
# Server
HOST=0.0.0.0
PORT=8080
RUST_LOG=info

# Database
DATABASE_URL=postgresql://user:pass@localhost:5432/trackmaker

# Redis
REDIS_URL=redis://localhost:6379

# JWT
JWT_SECRET=<random-64-char-string>
JWT_EXPIRY_MINUTES=15
REFRESH_SECRET=<different-random-64-char-string>
REFRESH_EXPIRY_DAYS=7

# OAuth
GOOGLE_CLIENT_ID=...
GOOGLE_CLIENT_SECRET=...
GITHUB_CLIENT_ID=...
GITHUB_CLIENT_SECRET=...
OAUTH_REDIRECT_BASE=https://your-domain.com

# Invite System
INVITE_SALT=<random-string>
REQUIRE_INVITE=true                         # set to false for public registration

# Email (for verification & password reset)
SMTP_HOST=smtp.example.com
SMTP_PORT=587
SMTP_USER=...
SMTP_PASSWORD=...
FROM_EMAIL=noreply@trackmaker.app
```

### Client (.env)

```env
VITE_API_URL=http://localhost:8080
VITE_OAUTH_GOOGLE_CLIENT_ID=...
VITE_OAUTH_GITHUB_CLIENT_ID=...
```

---

## Implementation Phases

### Phase 1: Foundation (5-8 weeks)

#### Backend
- [ ] Set up Rust project with Axum, PostgreSQL, Redis
- [ ] Implement database schema and migrations (all tables)
- [ ] Implement invite code generation and validation
- [ ] Create admin invite management endpoints
- [ ] Create user registration (with invite code gate) and login
- [ ] Implement JWT issuance + refresh token rotation
- [ ] Implement auth middleware (JWT validation, Redis blacklist check)
- [ ] Implement rate limiting middleware
- [ ] Create sketch CRUD (metadata only)
- [ ] Create route CRUD (granular, with version tracking)
- [ ] Implement sync push/pull at route level
- [ ] Implement conflict detection (version mismatch → return both versions)
- [ ] Implement conflict resolution endpoint
- [ ] Implement sync log recording
- [ ] Implement password reset flow (token via email)
- [ ] Implement email verification (send on register, verify endpoint)
- [ ] Implement data export endpoint (ZIP of GeoJSON)
- [ ] Write integration tests for auth + sync flows

#### Client
- [ ] Create JoinView (`/join`) with invite code input and validation
- [ ] Create auth dialog (login/register with invite code field)
- [ ] Create user store with auth state (access token in memory)
- [ ] Implement token refresh interceptor in API client
- [ ] Create trailblazer welcome page (post-registration)
- [ ] Create `/trailblazers` public page
- [ ] Implement sync store with offline queue
- [ ] Implement sync triggers (app launch, save, periodic, background)
- [ ] Create sync status indicator component
- [ ] Create conflict resolver UI (side-by-side comparison)
- [ ] Create migration dialog (local → cloud first-time upload)
- [ ] Integrate user menu in AppLayout
- [ ] Add Account section in Settings (profile, password change, data export)
- [ ] Add share button placeholder in SketchCentreView (disabled, Phase 2)

**Deliverable**: Users can register with invite code, login, sync routes to cloud with conflict resolution, and export their data.

### Phase 2: Sharing & OAuth (4-6 weeks)

#### Backend
- [ ] Implement OAuth2 flows (Google, GitHub)
- [ ] Implement OAuth account linking/unlinking
- [ ] Implement sketch sharing (user-to-user with access levels)
- [ ] Implement "shared with me" query
- [ ] Implement public read-only link generation
- [ ] Implement permission checking in route CRUD (owner or shared edit access)
- [ ] Add pagination to all list endpoints
- [ ] Implement soft-delete purge job (cron: delete records > 30 days old)

#### Client
- [ ] Add OAuth login buttons (Google, GitHub)
- [ ] Create OAuth callback handler view
- [ ] Create share dialog (search user, set access level)
- [ ] Create "Shared with Me" section in sketch list
- [ ] Create public link generation UI
- [ ] Implement sharing store
- [ ] Add profile page with OAuth account management

**Deliverable**: Users can login via OAuth, share sketches with specific users, and generate read-only public links.

---

## Future Considerations (Not Scheduled)

These are explicitly **out of scope** for the initial release. Each would require its own design document.

- [ ] **Real-time collaborative editing**: Requires CRDT/OT engine (consider Yjs integration), WebSocket infrastructure, presence indicators. This is a fundamentally different architecture from sync — would be designed as a separate system layered on top.
- [ ] Team workspaces
- [ ] Comments on routes
- [ ] Activity history / audit log UI
- [ ] Sketch templates
- [ ] Mobile app (Tauri Mobile / Capacitor)
- [ ] Service Worker for offline web support

---

## Alpha → Public Transition

When Trackmaker exits alpha:

1. Set `REQUIRE_INVITE=false` in backend config — registration opens
2. Remove invite code field from registration UI (or make it optional for referral tracking)
3. Alpha users retain permanently:
   - `trailblazer_seq` and `cairn_name` in their user record
   - 🪨 Trailblazer badge on profile (`trailblazer_seq IS NOT NULL`)
   - Listing on `/trailblazers` page
4. The `/trailblazers` page remains as a permanent record

---

## Deployment

### Prerequisites

- PostgreSQL 15+
- Redis 7+
- Reverse proxy (Caddy recommended for automatic HTTPS)
- SMTP service (for email verification + password reset)

### Docker Compose (Recommended)

```yaml
version: '3.8'

services:
  cairn:
    build: ./backend
    environment:
      - DATABASE_URL=postgresql://trackmaker:pass@db:5432/trackmaker
      - REDIS_URL=redis://redis:6379
    ports:
      - "8080:8080"
    depends_on:
      - db
      - redis

  db:
    image: postgres:15-alpine
    environment:
      POSTGRES_DB: trackmaker
      POSTGRES_USER: trackmaker
      POSTGRES_PASSWORD: pass
    volumes:
      - pgdata:/var/lib/postgresql/data

  redis:
    image: redis:7-alpine
    volumes:
      - redisdata:/data

  caddy:
    image: caddy:2-alpine
    ports:
      - "443:443"
      - "80:80"
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile

volumes:
  pgdata:
  redisdata:
```

### Backend Dockerfile

```dockerfile
FROM rust:1.75-alpine AS builder
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static
WORKDIR /app
COPY . .
RUN cargo build --release

FROM alpine:3.19
RUN apk add --no-cache ca-certificates
RUN adduser -D appuser
COPY --from=builder /app/target/release/cairn /usr/local/bin/
USER appuser
EXPOSE 8080
CMD ["cairn", "serve"]
```

---

## Testing Strategy

### Backend

| Layer | Tool | Coverage |
|-------|------|----------|
| Unit | `#[cfg(test)]` modules | Auth logic, invite generation, conflict detection |
| Integration | `axum::test` + testcontainers | Full API flows with real PostgreSQL |
| Load | k6 or drill | Sync endpoint under concurrent push/pull |

### Client

| Layer | Tool | Coverage |
|-------|------|----------|
| Unit | Vitest | Stores, sync queue logic, conflict detection |
| Component | Vue Test Utils | Auth dialog, conflict resolver, migration wizard |
| E2E | Playwright | Register → sync → conflict → resolve full flow |

---

## Timeline Summary

| Phase | Duration | Deliverables |
|-------|----------|-------------|
| Phase 1 | 5-8 weeks | Auth, invite system, route-level sync, conflict resolution, data export |
| Phase 2 | 4-6 weeks | OAuth, sharing, public links, pagination |
| **Total** | **9-14 weeks** | **Full featured backend with invite-gated alpha** |

---

Last updated: Jan. 18, 2026.
