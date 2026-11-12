# REST HTTP Transport: Analysis and Decision Record

**Status:** Resolved — Option A (Custom HTTP/1.1 on Tokio + tokio-rustls; direct dependencies approved without dynamic runtime fetching).

## 1. Architectural Problem

Selection of the underlying transport client for dispatching REST requests to Discord endpoints (`/api/v9/*`):
- Option A: Native asynchronous HTTP/1.1 client built on `tokio` and `tokio-rustls`.
- Option B: Full-featured HTTP client library (`reqwest` with rustls backend).

## 2. Decision Criteria

- Zero unverified external binary dependencies.
- Strict control over socket reuse, connection pooling, and latency.
- Deterministic behavior under network throttling and proxy configurations.

## 3. Selected Approach: Option A

Implementation utilizes `tokio` asynchronous TCP streams with `tokio-rustls` for TLS negotiation, adhering to HTTP/1.1 connection reuse contracts and rate limiting headers (`X-RateLimit-*`).\n