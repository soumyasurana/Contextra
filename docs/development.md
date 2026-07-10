# Development Guide

## Overview

This document defines the development standards for Contextra.

The primary objective is to maintain a codebase that remains modular, testable, observable, and easy to evolve.

Every contribution should preserve the architectural principles described throughout the documentation.

---

# Philosophy

Contextra follows a simple rule:

> **Business logic belongs in libraries. Transport belongs in services.**

Libraries implement domain behavior.

Services expose that behavior to users.

This separation must never be violated.

---

# Workspace Organization

The repository is organized into four major areas.

```text
services/
```

Deployable applications.

Examples:

- Gateway
- Worker
- CLI

---

```text
libs/
```

Reusable business logic.

Libraries implement:

- Storage
- Retrieval
- Memory
- Context
- Providers
- Embeddings
- Orchestration

---

```text
proto/
```

Protocol Buffer definitions.

---

```text
sdk/
```

Official client SDKs.

---

# Library Rules

Every library must satisfy the following principles.

## Single Responsibility

A library owns one domain.

If a feature belongs to multiple domains, create a shared abstraction rather than introducing cross-domain coupling.

---

## Transport Independence

Libraries must not depend on:

- Axum
- Hyper
- HTTP
- gRPC
- CLI
- Docker

Libraries should be executable inside tests without networking.

---

## Dependency Direction

Dependencies always point downward.

```text
Gateway

↓

Orchestration

↓

Context

↓

Memory
Retrieval

↓

Embeddings

↓

Providers

↓

Storage

↓

Foundation Libraries
```

Circular dependencies are prohibited.

---

## Public API

Every library should expose a small, stable public API.

Implementation details remain private.

---

# Services

Services should remain thin.

Responsibilities include:

- Configuration
- Dependency injection
- Authentication
- Middleware
- Request validation
- Calling libraries

Business logic belongs in libraries.

---

# Error Handling

Every library returns a shared error type.

Avoid:

```rust
panic!()
unwrap()
expect()
```

Errors should propagate through `Result`.

---

# Configuration

Configuration should never be hardcoded.

All configurable values belong in:

- Environment variables
- TOML configuration
- Secrets management

Libraries should receive configuration through dependency injection.

---

# Logging

Use structured logging.

Every important operation should emit:

- Request ID
- Trace ID
- Duration
- Status
- Error (if applicable)

Sensitive information must never be logged.

---

# Testing

Every feature should include tests.

## Unit Tests

Located inside the corresponding library.

Purpose:

- Business logic
- Algorithms
- Validation

---

## Integration Tests

Located in:

```text
tests/integration/
```

Verify interaction between multiple libraries.

---

## End-to-End Tests

Located in:

```text
tests/e2e/
```

Test complete workflows.

---

## Benchmarks

Located in:

```text
tests/benchmarks/
```

Measure latency and throughput.

---

# Code Style

Use:

- `cargo fmt`
- `cargo clippy`

Every commit should compile cleanly.

Warnings should be treated as bugs whenever practical.

---

# Documentation

Public APIs require documentation.

Complex algorithms should include design comments explaining *why* they exist, not merely *what* they do.

Architectural decisions belong in `docs/`, not in source code comments.

---

# Commits

Commits should represent a single logical change.

Preferred format:

```text
feat(context): implement context assembler

fix(storage): rollback failed transactions

refactor(retrieval): simplify reranking pipeline

docs(api): update authentication section
```

Avoid unrelated changes in the same commit.

---

# Pull Requests

Every pull request should:

- Compile successfully
- Pass tests
- Follow formatting rules
- Include documentation updates when necessary

Architectural changes should also include an ADR (Architecture Decision Record).

---

# Performance

Performance is considered a feature.

When introducing changes:

- Avoid unnecessary allocations
- Minimize cloning
- Prefer borrowing
- Benchmark critical paths
- Measure before optimizing

Premature optimization should be avoided, but unnecessary inefficiencies should not be introduced.

---

# Observability

Every major workflow should emit:

- Traces
- Metrics
- Structured logs

If a production issue cannot be diagnosed through telemetry, observability is considered incomplete.

---

# Security

Never:

- Commit secrets
- Log credentials
- Disable TLS verification
- Trust unvalidated input

Security is the default, not an optional feature.

---

# Architectural Principles

Every contribution should reinforce the following principles.

- Separation of concerns
- Strong typing
- Provider independence
- Storage abstraction
- Modular design
- Dependency inversion
- Observability
- Testability

When in doubt, choose the design that keeps libraries independent and responsibilities clear.

---

# Summary

Contextra is intended to be a long-lived platform.

Consistency is more valuable than cleverness.

A small, well-structured implementation is preferred over a larger, more complex one.

Every contribution should improve the maintainability, clarity, and reliability of the platform.