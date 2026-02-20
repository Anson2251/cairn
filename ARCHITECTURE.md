# Cairn Backend Architecture

## Overview

Cairn is a Rust-based backend service for Trackmaker that provides user authentication, route sharing, and cloud synchronization. It follows an **offline-first** design where local storage remains primary and cloud sync is opt-in.

## System Architecture

```mermaid
flowchart TB
    subgraph Client["Client (Vue 3 + Tauri/Web)"]
        UI["UI Components"]
        Stores["Pinia Stores"]
        SyncClient["Sync Client"]
        LocalDB["IndexedDB (Local)"]
        TokenStorage["Token Storage"]
    end
    
    subgraph CDN["CDN / Edge"]
        StaticAssets["Static Assets"]
    end
    
    subgraph Cairn["Cairn Backend (Rust/Axum)"]
        APIGateway["API Gateway"]
        
        subgraph Middleware["Middleware Layer"]
            RateLimit["Rate Limiting (Redis)"]
            Auth["JWT Auth"]
            CORS["CORS"]
        end
        
        subgraph Handlers["Route Handlers"]
            AuthHandlers["Auth"]
            SketchHandlers["Sketches"]
            RouteHandlers["Routes"]
            SyncHandlers["Sync"]
            ShareHandlers["Sharing"]
            InviteHandlers["Invites"]
            ExportHandlers["Export"]
        end
        
        subgraph Services["Service Layer"]
            AuthService["Auth Service"]
            SyncService["Sync Service"]
            ShareService["Share Service"]
            InviteService["Invite Service"]
        end
        
        subgraph Infrastructure["Infrastructure"]
            JWT["JWT Manager"]
            InviteGen["Invite Generator"]
            ConflictResolver["Conflict Resolver"]
        end
    end
    
    subgraph Data["Data Layer"]
        PostgreSQL[("PostgreSQL")]
        Redis[("Redis")]
    end
    
    subgraph Email["External Services"]
        SMTP["SMTP Server"]
        OAuth["OAuth Providers"]
    end
    
    UI --> Stores
    Stores --> SyncClient
    SyncClient --> LocalDB
    SyncClient --> TokenStorage
    SyncClient -.->|"HTTPS/API"| APIGateway
    
    APIGateway --> Middleware
    Middleware --> Handlers
    Handlers --> Services
    Services --> Infrastructure
    
    Services --> PostgreSQL
    Middleware --> Redis
    AuthService -.->|"Send Email"| SMTP
    AuthService -.->|"OAuth Flow"| OAuth
```

## Database Schema

```mermaid
erDiagram
    USERS ||--o{ SKETCHES : owns
    USERS ||--o{ USER_OAUTH_ACCOUNTS : has
    USERS ||--o{ REFRESH_TOKENS : has
    USERS ||--o{ SHARES : creates
    USERS ||--o{ SHARES : receives
    
    INVITE_CODES ||--o| USERS : used_by
    
    SKETCHES ||--o{ ROUTES : contains
    SKETCHES ||--o{ SHARES : shared_via
    SKETCHES ||--o{ PUBLIC_LINKS : has
    
    ROUTES ||--o{ SYNC_LOG : generates
    
    USERS {
        uuid id PK
        string email UK
        boolean email_verified
        string username UK
        string hashed_password
        string avatar_url
        string role
        jsonb settings
        uuid invite_code_id FK
        int trailblazer_seq
        timestamp deleted_at
        timestamp created_at
        timestamp updated_at
    }
    
    USER_OAUTH_ACCOUNTS {
        uuid id PK
        uuid user_id FK
        string provider
        string provider_id
        string provider_email
        text access_token
        timestamp created_at
    }
    
    INVITE_CODES {
        uuid id PK
        int sequence UK
        string code UK
        string cairn_name
        point origin_coord
        text memo
        boolean used
        uuid used_by FK
        timestamp used_at
        timestamp expires_at
        timestamp created_at
    }
    
    SKETCHES {
        uuid id PK
        uuid user_id FK
        string name
        text description
        boolean is_public
        timestamp deleted_at
        timestamp created_at
        timestamp updated_at
    }
    
    ROUTES {
        uuid id PK
        uuid sketch_id FK
        string name
        text description
        jsonb geojson
        jsonb metadata
        text notes
        int version
        timestamp deleted_at
        timestamp created_at
        timestamp updated_at
    }
    
    SHARES {
        uuid id PK
        uuid sketch_id FK
        uuid user_id FK
        string access_level
        timestamp created_at
        uuid created_by FK
    }
    
    PUBLIC_LINKS {
        uuid id PK
        uuid sketch_id FK
        string token UK
        string access_level
        timestamp expires_at
        uuid created_by FK
        timestamp created_at
    }
    
    SYNC_LOG {
        uuid id PK
        uuid route_id FK
        uuid user_id FK
        string action
        int version_before
        int version_after
        string resolution
        uuid client_id
        timestamp created_at
    }
    
    REFRESH_TOKENS {
        uuid id PK
        uuid user_id FK
        string token_hash UK
        uuid client_id
        timestamp expires_at
        boolean revoked
        timestamp created_at
    }
```

## Authentication Flow

```mermaid
sequenceDiagram
    actor User
    participant Client
    participant Cairn
    participant Redis
    participant PostgreSQL
    
    %% Registration with Invite
    User->>Client: Enter invite code
    Client->>Cairn: GET /api/invite/{code}/validate
    Cairn->>PostgreSQL: Check invite code
    PostgreSQL-->>Cairn: Valid
    Cairn-->>Client: {valid: true}
    
    User->>Client: Register (email, password, invite)
    Client->>Cairn: POST /api/auth/register
    Cairn->>PostgreSQL: Create user, mark invite used
    Cairn->>PostgreSQL: Store refresh token hash
    Cairn-->>Client: {user, access_token} + Set-Cookie: refresh_token
    
    %% Login
    User->>Client: Login
    Client->>Cairn: POST /api/auth/login
    Cairn->>Redis: Check rate limit
    Redis-->>Cairn: OK
    Cairn->>PostgreSQL: Verify credentials
    PostgreSQL-->>Cairn: User found
    Cairn->>PostgreSQL: Store refresh token hash
    Cairn-->>Client: {user, access_token} + Set-Cookie: refresh_token
    
    %% Authenticated Request
    Client->>Cairn: GET /api/sketches (Authorization: Bearer {access_token})
    Cairn->>Cairn: Validate JWT
    Cairn->>PostgreSQL: Fetch sketches
    PostgreSQL-->>Cairn: Sketch list
    Cairn-->>Client: {sketches}
    
    %% Token Refresh
    Note over Client,Cairn: Access token expires (15 min)
    Client->>Cairn: POST /api/auth/refresh (Cookie: refresh_token)
    Cairn->>PostgreSQL: Verify refresh token hash
    PostgreSQL-->>Cairn: Valid
    Cairn->>PostgreSQL: Revoke old, store new hash
    Cairn-->>Client: {access_token} + Set-Cookie: refresh_token
    
    %% Logout
    User->>Client: Logout
    Client->>Cairn: POST /api/auth/logout
    Cairn->>PostgreSQL: Revoke refresh token
    Cairn-->>Client: Clear cookie
```

## Sync Flow

```mermaid
sequenceDiagram
    actor User
    participant Client
    participant LocalDB["Local IndexedDB"]
    participant Cairn
    participant PostgreSQL
    
    %% Push Changes
    User->>Client: Edit route
    Client->>LocalDB: Save changes locally
    Client->>Client: Add to pending queue
    
    Note over Client: After 5s debounce / manual sync
    
    Client->>Cairn: POST /api/sync/push
    Note right of Client: {changes: [{route_id, base_version, data}]}
    
    loop For each route
        Cairn->>PostgreSQL: Get current version
        PostgreSQL-->>Cairn: server_version
        
        alt server_version == base_version
            Cairn->>PostgreSQL: Update route, version++
            Cairn->>Cairn: Mark as accepted
        else Version mismatch
            Cairn->>Cairn: Mark as conflict
        end
    end
    
    Cairn-->>Client: {accepted: [...], conflicts: [...]}
    
    alt Has conflicts
        Client->>User: Show conflict resolver UI
        User->>Client: Choose resolution (keep_local/keep_remote/merge)
        Client->>Cairn: POST /api/sync/resolve/{route_id}
        Cairn->>PostgreSQL: Apply resolution, version++
        Cairn-->>Client: Success
    end
    
    %% Pull Changes
    Client->>Cairn: POST /api/sync/pull
    Note right of Client: {last_synced_at, known_versions}
    Cairn->>PostgreSQL: Get routes updated since last_sync
    PostgreSQL-->>Cairn: Updated routes
    Cairn-->>Client: {updated: [...], deleted: [...]}
    Client->>LocalDB: Apply changes locally
```

## Sharing Flow

```mermaid
sequenceDiagram
    actor Owner
    actor Recipient
    participant Client
    participant Cairn
    participant PostgreSQL
    participant Email
    
    %% Share Sketch
    Owner->>Client: Click share button
    Client->>Client: Open share dialog
    Owner->>Client: Enter recipient email
    Client->>Cairn: POST /api/sketches/{id}/shares
    Cairn->>PostgreSQL: Check owner permission
    PostgreSQL-->>Cairn: Owner confirmed
    Cairn->>PostgreSQL: Create share record
    Cairn-->>Client: Share created
    
    %% Recipient Access
    Recipient->>Client: View shared sketches
    Client->>Cairn: GET /api/sketches/shared
    Cairn->>PostgreSQL: Get shares for user
    PostgreSQL-->>Cairn: Shared sketches
    Cairn-->>Client: {shared_sketches}
    
    %% Access Control Check
    Recipient->>Client: Edit shared route
    Client->>Cairn: PUT /api/routes/{id}
    Cairn->>PostgreSQL: Check share permissions
    PostgreSQL-->>Cairn: edit access confirmed
    Cairn->>PostgreSQL: Update route
    Cairn-->>Client: Success
    
    %% Public Link
    Owner->>Client: Create public link
    Client->>Cairn: POST /api/sketches/{id}/public-link
    Cairn->>PostgreSQL: Create public link record
    Cairn-->>Client: {token}
    
    Note over Recipient,Cairn: Anyone with link can access
    Recipient->>Cairn: GET /api/public/{token}
    Cairn->>PostgreSQL: Validate token
    PostgreSQL-->>Cairn: Valid, read-only access
    Cairn-->>Recipient: Sketch data
```

## Module Structure

```mermaid
flowchart TB
    subgraph Entry["Entry Point"]
        main["main.rs"]
        config["config.rs"]
        error["error.rs"]
    end
    
    subgraph Middleware["Middleware"]
        mw_mod["mod.rs"]
        mw_auth["auth.rs"]
        mw_admin["admin.rs"]
        mw_rate["rate_limit.rs"]
    end
    
    subgraph Database["Database Layer"]
        db_mod["mod.rs"]
        migrations["migrations/"]
    end
    
    subgraph Modules["Feature Modules"]
        subgraph Auth["Auth Module"]
            auth_mod["mod.rs"]
            auth_handlers["handlers.rs"]
            auth_jwt["jwt.rs"]
            auth_oauth["oauth.rs"]
        end
        
        subgraph Invite["Invite Module"]
            invite_mod["mod.rs"]
            invite_gen["generator.rs"]
            invite_handlers["handlers.rs"]
        end
        
        subgraph Sketches["Sketches Module"]
            sketch_mod["mod.rs"]
            sketch_handlers["handlers.rs"]
        end
        
        subgraph Routes["Routes Module"]
            route_mod["mod.rs"]
            route_handlers["handlers.rs"]
        end
        
        subgraph Sync["Sync Module"]
            sync_mod["mod.rs"]
            sync_handlers["handlers.rs"]
        end
        
        subgraph Sharing["Sharing Module"]
            share_mod["mod.rs"]
            share_handlers["handlers.rs"]
        end
        
        subgraph Export["Export Module"]
            export_mod["mod.rs"]
            export_handlers["handlers.rs"]
        end
    end
    
    main --> config
    main --> error
    main --> Middleware
    main --> Modules
    main --> Database
    
    mw_mod --> mw_auth
    mw_mod --> mw_admin
    mw_mod --> mw_rate
    
    Auth --> Middleware
    Invite --> Auth
    Sketches --> Auth
    Routes --> Auth
    Sync --> Auth
    Sharing --> Auth
    Export --> Auth
```

## Request Flow

```mermaid
flowchart LR
    Request["HTTP Request"] --> CORS["CORS Middleware"]
    CORS --> RateLimit["Rate Limit Middleware"]
    RateLimit --> Routing["Router"]
    
    Routing --> Public["Public Endpoints"]
    Routing --> Protected["Protected Endpoints"]
    
    Protected --> Auth["Auth Middleware"]
    Auth --> Permission["Permission Check"]
    
    Permission --> Handler["Route Handler"]
    Public --> Handler
    
    Handler --> Service["Service Layer"]
    Service --> Database["Database"]
    
    Handler --> Response["HTTP Response"]
    Service --> Response
    Auth --> Error["Error Response"]
    RateLimit --> Error
    
    style Protected fill:#f9f,stroke:#333
    style Auth fill:#ff9,stroke:#333
```

## Data Flow (Offline-First)

```mermaid
flowchart TB
    subgraph Local["Local Device"]
        UI["User Interface"]
        State["Pinia Store"]
        Queue["Pending Changes Queue"]
        IDB["IndexedDB"]
        ConflictUI["Conflict Resolver UI"]
    end
    
    subgraph Network["Network Layer"]
        OnlineCheck{"Online Check"}
        API["API Client"]
    end
    
    subgraph Server["Cairn Server"]
        SyncAPI["Sync Endpoints"]
        ConflictDet["Conflict Detection"]
        DB[("PostgreSQL")]
    end
    
    UI --> State
    State --> IDB
    State --> Queue
    
    State --> OnlineCheck
    OnlineCheck -->|Offline| Queue
    OnlineCheck -->|Online| API
    
    Queue --> API
    API --> SyncAPI
    SyncAPI --> ConflictDet
    ConflictDet --> DB
    
    SyncAPI -->|Conflicts| API
    API -->|Conflicts| ConflictUI
    ConflictUI -->|Resolution| API
    
    DB -->|Updates| SyncAPI
    SyncAPI -->|Pull Response| API
    API --> State
    State --> IDB
```

## Security Layers

```mermaid
flowchart TB
    subgraph Perimeter["Perimeter Security"]
        HTTPS["HTTPS/TLS"]
        CORS["CORS Policy"]
        HSTS["HSTS Header"]
    end
    
    subgraph Transport["Transport Security"]
        RateLimit["Rate Limiting"]
        InputVal["Input Validation"]
        SizeLimit["Request Size Limits"]
    end
    
    subgraph Auth["Authentication"]
        JWTVal["JWT Validation"]
        TokenRot["Token Rotation"]
        RefreshRevoke["Refresh Token Revocation"]
    end
    
    subgraph Authz["Authorization"]
        OwnerCheck["Ownership Check"]
        SharePerm["Share Permission Check"]
        RoleCheck["Role Check"]
    end
    
    subgraph Data["Data Security"]
        PasswordHash["Password Hashing (bcrypt)"]
        SoftDelete["Soft Delete"]
        FieldEnc["Field-level Encryption"]
    end
    
    Perimeter --> Transport
    Transport --> Auth
    Auth --> Authz
    Authz --> Data
```

## Deployment Architecture

```mermaid
flowchart TB
    subgraph Users["Users"]
        Web["Web Browser"]
        Desktop["Desktop App (Tauri)"]
    end
    
    subgraph Edge["Edge Layer"]
        CDN["CDN (Static Assets)"]
        LB["Load Balancer"]
    end
    
    subgraph App["Application Layer"]
        C1["Cairn Instance 1"]
        C2["Cairn Instance 2"]
        C3["Cairn Instance N"]
    end
    
    subgraph Data["Data Layer"]
        PostgreSQL[("PostgreSQL<br/>Primary")]
        PGReplica[("PostgreSQL<br/>Replica")]
        Redis[("Redis<br/>Cluster")]
    end
    
    subgraph External["External Services"]
        SMTP["SMTP Server"]
        OAuth["OAuth Providers"]
    end
    
    Web --> CDN
    Desktop --> LB
    Web --> LB
    
    LB --> C1
    LB --> C2
    LB --> C3
    
    C1 --> PostgreSQL
    C2 --> PostgreSQL
    C3 --> PostgreSQL
    
    C1 --> Redis
    C2 --> Redis
    C3 --> Redis
    
    PostgreSQL --> PGReplica
    
    C1 -.-> SMTP
    C1 -.-> OAuth
```

## Technology Stack

| Layer | Technology | Purpose |
|-------|------------|---------|
| **Runtime** | Rust + Tokio | High-performance async runtime |
| **Web Framework** | Axum | HTTP server, routing, middleware |
| **Database** | PostgreSQL | Primary data store |
| **Cache** | Redis | Sessions, rate limiting, token blacklist |
| **ORM** | SQLx | Type-safe SQL with compile-time checks |
| **Auth** | JWT + bcrypt | Token-based auth, password hashing |
| **Serialization** | Serde | JSON handling |
| **Validation** | Validator | Input validation |
| **Logging** | Tracing | Structured logging |
| **Config** | config-rs | Environment-based configuration |

## API Endpoint Structure

```
/api
├── /auth
│   ├── POST /register
│   ├── POST /login
│   ├── POST /logout
│   ├── POST /refresh
│   ├── GET  /me
│   ├── PUT  /me
│   ├── POST /forgot-password
│   ├── POST /reset-password
│   ├── POST /verify-email
│   ├── GET  /oauth/{provider}
│   ├── GET  /oauth/{provider}/callback
│   ├── POST /oauth/{provider}/link
│   └── DELETE /oauth/{provider}/unlink
├── /sketches
│   ├── GET    / (list)
│   ├── GET    /shared (list shared)
│   ├── POST   / (create)
│   ├── GET    /{id} (get)
│   ├── PUT    /{id} (update)
│   ├── DELETE /{id} (delete)
│   ├── GET    /{id}/routes (list routes)
│   ├── POST   /{id}/routes (create route)
│   ├── POST   /{id}/shares (share)
│   ├── GET    /{id}/shares (list shares)
│   ├── POST   /{id}/public-link (create link)
│   └── DELETE /{id}/public-link (revoke link)
├── /routes
│   ├── GET    /{id} (get)
│   ├── PUT    /{id} (update)
│   └── DELETE /{id} (delete)
├── /sync
│   ├── POST /push
│   ├── POST /pull
│   └── POST /resolve/{route_id}
├── /sharing
│   ├── PUT    /sketches/{id}/shares/{user_id}
│   └── DELETE /sketches/{id}/shares/{user_id}
├── /invite
│   └── GET /{code}/validate
├── /admin
│   ├── POST /invites (create)
│   ├── GET  /invites (list)
│   └── DELETE /invites/{id} (revoke)
├── /public
│   └── GET /{token} (access public sketch)
├── /trailblazers
│   └── GET / (list)
└── /export
    ├── POST / (request)
    └── GET  /{job_id} (download)
```

## Key Design Decisions

### 1. Offline-First Architecture
- Local IndexedDB is the source of truth
- Cloud sync is opt-in and additive
- Changes are queued when offline and replayed when connected
- Conflict resolution is manual at the route level

### 2. Route-Level Granularity
- Routes are the atomic unit of sync, not sketches
- Multiple users can edit different routes in the same sketch simultaneously
- Conflicts only occur when the same route is edited concurrently

### 3. Version-Based Optimistic Locking
- Each route has a version counter
- Push includes `base_version` — if server version differs, it's a conflict
- No automatic merging — user decides resolution

### 4. Token Strategy
- Short-lived access tokens (15 min) in memory only
- Long-lived refresh tokens (7 days) in HttpOnly cookies / secure storage
- Refresh tokens are rotated and tracked server-side for revocation

### 5. Soft Delete with Retention
- All deletions are soft (set `deleted_at`)
- 30-day retention before permanent purge
- Allows for data recovery and audit trails

## Scalability Considerations

### Horizontal Scaling
- Stateless application servers (no session state)
- JWT auth allows any instance to validate tokens
- Database is the only shared state

### Database Scaling
- Read replicas for GET endpoints
- Connection pooling via SQLx
- Proper indexing on all query patterns

### Caching Strategy
- Redis for rate limiting counters
- Token blacklist for emergency revocation
- Future: Route-level caching for public links

### Sync Optimization
- Batch push/pull operations
- Delta sync (only changed fields)
- Compression for large GeoJSON payloads

## Monitoring & Observability

```mermaid
flowchart TB
    subgraph App["Cairn Application"]
        Tracing["Tracing Instrumentation"]
        Metrics["Prometheus Metrics"]
        Health["Health Checks"]
    end
    
    subgraph Observability["Observability Stack"]
        Tempo["Tempo (Traces)"]
        Prometheus["Prometheus (Metrics)"]
        Grafana["Grafana (Dashboards)"]
        Alerts["Alert Manager"]
    end
    
    Tracing --> Tempo
    Metrics --> Prometheus
    Health --> Prometheus
    
    Tempo --> Grafana
    Prometheus --> Grafana
    Prometheus --> Alerts
```

### Key Metrics
- Request latency (p50, p95, p99)
- Error rate by endpoint
- Sync queue depth
- Conflict rate
- Token refresh rate
- Database connection pool usage

## Next Steps

1. **Phase 1**: Core auth, invite system, basic sync
2. **Phase 2**: OAuth, sharing, public links
3. **Phase 3**: Advanced features, optimization

---

*This architecture document serves as the blueprint for implementing the Cairn backend service.*
