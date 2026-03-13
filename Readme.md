
# Social API — Rust Microservice

A high-performance, horizontally scalable microservice built with Rust, designed to handle social interactions (Likes, with future support for Comments/Bookmarks) across the BeInCrypto ecosystem.

## 1. Architectural Overview & Project Structure

The service is built using **Hexagonal Architecture (Ports and Adapters)** and **Domain-Driven Design (DDD)** principles. This ensures the core business logic remains isolated from external dependencies like PostgreSQL, Redis, or third-party HTTP APIs.

### Key Layers:

* **Domain:** Pure business logic and entities (e.g., `Like`, `UserId`).
* **Application:** Orchestrates use cases (e.g., `LikeService`) and manages Pub/Sub for SSE.
* **Infrastructure:** Adapters for Persistence (PostgreSQL), Caching (Redis), and External Validation (Content/Profile APIs).
* **API:** Axum-based REST handlers, Middleware (Auth, Rate Limiting), and SSE streaming.


```text
src/
├── api/             # Axum handlers, SSE, and Middlewares
├── application/     # Application services and SSE Pub/Sub
├── domain/          # Core entities and Port definitions
├── infrastructure/  # DB, Redis, and External API adapters
└── config.rs        # Centralized configuration management
```
---

## 2. Technical Decisions & Justifications

### 2.1 Database Schema & Counting Strategy & Data Integrity

To meet the requirement of tracking likes across multiple timeframes (24h, 7d, all-time), we opted for **Direct Aggregation with a Multi-Layered Cache Strategy** instead of Materialized Views.

* **Decision:** We perform `COUNT(*)` queries on the indexed `likes` table.
* **Justification:** Given the 7-day development constraint, this approach avoids the complexity of manual cache invalidation or database trigger overhead. To protect the database from high-concurrency read pressure, we utilize **Singleflight** (request collapsing) and **Redis caching**. This provides the best balance between implementation speed and system reliability.
 **Efficient Batching (N+1 Prevention):** Batch count endpoints use PostgreSQL's `UNNEST` function. This allows us to pass arrays of IDs in a single query, which the database unrolls and joins efficiently, avoiding the overhead of multiple distinct SELECT calls.
**Cursor-Based Pagination:** The user history endpoint utilizes cursor-based pagination (Base64 encoded timestamp + ID) instead of Offset/Limit. This enables $O(1)$ index seeking, ensuring performance remains consistent even as the database grows to millions of rows.
**Idempotency:** Duplicate "Like" requests are handled at the database level using a unique composite constraint and `ON CONFLICT DO NOTHING` logic, ensuring data integrity without extra application-level checks.

### 2.2 Consistency & Staleness Window

The system is designed for **Eventual Consistency** for aggregate data and **Strong Consistency** for individual user actions.

* **Maximum Staleness:** * For individual Like/Unlike actions, we use a **Write-Through cache**, ensuring immediate consistency for the user.
* For batch counts (24h/7d) and leaderboards, the maximum staleness window is defined by the `CACHE_TTL` (max **300 seconds** for 7-day windows, lower for 24-hour windows).
* **Content:** 1 hour (stability).
* **Sessions:** 5 minutes (security and rapid revocation).
* **Errors (Negative Cache):** 5 minutes (spam prevention).
**Decision:** This window significantly reduces DB load while remaining well within acceptable product requirements for social metrics.


### 2.3 Concurrency & Thundering Herd Protection

To handle 50M+ monthly pageviews, the service implements:

* **Thundering Herd Protection (Singleflight):** To prevent multiple concurrent requests from hitting external APIs for the same resource simultaneously, the system uses a Singleflight pattern. By utilizing `DashMap` for locking and recursive `Box::pin` calls, only the first request performs the external call while others wait to consume the result from the cache.

* **Canary Pattern (Leaderboard):** For high-traffic endpoints like the Leaderboard, we use a "Canary TTL" with background refresh with Background Refresh** and a Distributed Lock.
**Why:** A naive caching strategy drops the cache when TTL expires, forcing the next request to hit the DB. Under high load, this causes a "Cache Stampede" (Thundering Herd), crushing the database. 
**How it works:** 
1. The data key has a very long TTL, while a separate "Canary" key has a short TTL (e.g., 60 seconds).
2. When a request comes in, we fetch both. If the Canary is missing (expired), the data is considered "stale".
3. The system returns the stale data to the user immediately (ensuring < 5ms response time) but asynchronously acquires a Redis Lock.
4. The worker holding the lock fetches fresh data from PostgreSQL in the background, updates the cache, and resets the Canary.

* **Negative Caching:** We cache `404 Not Found` results for both users and content. This protects the system and external dependencies from "cache miss" attacks where malicious actors flood the API with non-existent IDs.
* **Raw Key Performance:** To meet the strict <10ms read path requirement, we prioritize direct key-to-value mapping over expensive cryptographic hashing for cache keys, minimizing CPU cycles and memory allocations per request.

* **Self-Healing (Cache Warming):** If Redis restarts (Cold Start), the first request will fail the cache check, fetch the data from PostgreSQL, and immediately rehydrate Redis. This ensures the cache is always "warm" for subsequent requests.
To handle Redis availability without compromising performance, the service implements a Lock-Free Background Monitor.
    Atomic Swapping: Using ArcSwap, the system maintains a shared connection pointer. The background monitor health-checks the connection every 10 seconds.
    Zero-Lock Reads: Unlike a standard RwLock, concurrent read requests (Likes/Counts) are never blocked by the reconnection logic.
    Automatic Recovery: If a Redis node fails and recovers (tested via k6 fault injection), the service automatically restores the connection without requiring a manual restart, ensuring maximum uptime.

### 4. Scalable Distributed Systems

* **Distributed SSE Event Streaming:** Server-Sent Events are synchronized across multiple replicas via Redis Pub/Sub. When a "Like" is registered on one node, the event is published to Redis; all other replicas listen and route the event to their locally connected clients through memory-efficient `DashMap` broadcast channels.
* **Atomic Rate Limiting:** Enforced via custom Lua scripts executed directly in Redis. This ensures atomic "Token Bucket" calculations across a Kubernetes cluster, preventing race conditions without requiring multiple network round-trips.
* **Circuit Breaker:** A custom implementation protects the service from cascading failures of external Content or Profile APIs. If a dependency fails, the breaker enters an "Open" state to fail fast. Crucially, the system is designed to allow read paths to continue serving cached data even when external validation is unavailable.


### 5. Observability & Security & Resilience

* **Prometheus Metrics:** Real-time tracking of request latencies and error rates. A background task (`tokio::spawn`) handles the periodic upkeep of the metrics recorder. Middleware also includes path normalization to prevent "cardinality explosion" in Prometheus caused by unique IDs in URLs.
 Exported at `/metrics`, including latency histograms and request counters.
* **Structured Logging:** Using `tracing-subscriber` for JSON-formatted logs, ready for ELK/Grafana Loki integration.

* **Graceful Shutdown:** The service listens for `SIGTERM/SIGINT`. Upon receiving a signal, it stops accepting new connections, allows a **30-second drain period** for active requests (including SSE streams), and explicitly closes PostgreSQL and Redis connection pools to prevent data corruption.

PostgreSQL and external APIs, prioritizing availability over latency.
* **Graceful Degradation:** The system treats Redis as an optimization. If the cache layer goes offline, the service automatically falls back t

### 6. Rust Best Practices

* **Functional Pipelines:** Intensive use of functional patterns (e.g., `.ok().flatten().and_then(...)`) to maintain a "Zero Pyramid" code structure, enhancing readability by avoiding nested match/if-let blocks.
* **Dependency Injection:** Full decoupling via `Arc<dyn Trait>`, allowing for seamless swapping of database or cache providers during testing or infrastructure upgrades.


## Known Limitations

Due to the 7-day delivery constraint , the following trade-offs were accepted:

* **Testing Strategy:** Current focus is on high-level Integration and E2E/load tests to ensure system cohesiveness. Granular unit testing for individual domain componentsis has not been done
* **Configuration:** Circuit Breaker thresholds and certain TTLs are currently static at startup. Moving these to a dynamic configuration system would allow for runtime tuning without service restarts.
* **Layer Leakage:** In specific areas (such as shared configuration structs, Dto, Errors ), strict hexagonal isolation was relaxed to accelerate development speed while maintaining overall architectural integrity.

## 3. Bonus Deliverables & Future Scalability

* **Load Testing:** A `k6` load test script is included in the repository to validate the performance requirements.
* **gRPC Ready:** Thanks to the Hexagonal Architecture, adding a gRPC adapter would only require implementing a new `API` layer without touching the business logic.


## 5. How to Run

**Multi-stage Dockerfile:** Uses `debian:bookworm-slim` for the final image to minimize the attack surface.
**Non-Root User:** The application runs under a dedicated `appuser` (UID 1000), following the principle of least privilege.

### Prerequisites
* Docker & Docker Compose
* `curl` and `jq` (for running the smoke tests)

### Starting the Service
[cite_start]To start the entire system (API, Postgres, Redis, and mock services), run the following command:
```bash
docker compose up --build
```

-   The service will be available at `http://localhost:8080`.
    

### Quick Smoke Tests

Once the containers are up and running, you can test the system using these commands:

-   **Check Readiness:**
    

Bash

```
curl -s http://localhost:8080/health/ready | jq .

```

-   **Get Like Count (Public):**
    

Bash

```
curl -s http://localhost:8080/v1/likes/post/731b0395-4888-4822-b516-05b4b7bf2089/count | jq .

```

-   **Like Content (Authenticated):**
    

Bash

```
curl -X POST http://localhost:8080/v1/likes \
  -H "Authorization: Bearer tok_user_1" \
  -H "Content-Type: application/json" \
  -d '{"content_type":"post","content_id":"731b0395-4888-4822-b516-05b4b7bf2089"}' | jq .
```
## 5. Development & Testing

### 5.1 Running Integration Tests

The integration suite tests the full flow: **API → Database → Redis → External Mocks**.

> [!IMPORTANT]
> 
> **Sequential Execution Required:** Integration tests must be run using a **single thread** (`--test-threads=1`). Since tests interact with a real shared PostgreSQL database and Redis instance, parallel execution would cause race conditions, data pollution, and unexpected state resets (e.g., one test flushing Redis while another is reading it).

To run the tests 

Bash

```
cargo test -- --test-threads=1 --nocapture

```

## License
This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

