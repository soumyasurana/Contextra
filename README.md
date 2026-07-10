# Contextra

<p align="center">
  <h3 align="center">A Context Engineering Platform for AI Applications</h3>
</p>

<p align="center">
  Build intelligent AI applications by managing the complete lifecycle of context—from knowledge ingestion and semantic retrieval to memory, orchestration, and provider execution.
</p>

<p align="center">
![Rust](https://img.shields.io/badge/Rust-2024-orange?logo=rust)
![License](https://img.shields.io/badge/license-MIT-blue)
![Status](https://img.shields.io/badge/status-Active%20Development-success)
![Architecture](https://img.shields.io/badge/Architecture-Modular%20Workspace-blueviolet)
![Microservices](https://img.shields.io/badge/Future-Microservices-informational)
</p>

---

## Overview

Contextra is a modular, high-performance platform for building AI-powered applications.

Unlike traditional Retrieval-Augmented Generation (RAG) frameworks, Contextra treats **context** as a first-class engineering problem.

It provides reusable infrastructure for:

- Document ingestion
- Semantic retrieval
- Long-term memory
- Prompt management
- Context optimization
- AI provider abstraction
- Workflow orchestration

The platform is implemented in **Rust** and follows a modular workspace architecture designed to evolve into independently deployable microservices.

---

# Why Contextra?

Most AI frameworks focus on one part of the pipeline.

Examples include:

- Retrieval
- Vector databases
- Prompt templates
- Agent execution

Contextra focuses on **the complete context lifecycle**.

Instead of asking:

> "How do I retrieve documents?"

Contextra asks:

> "What is the best possible context for this model given everything the system knows?"

That shift makes retrieval only one component of a much larger Context Engineering Platform.

---

# Architecture

```text
                   Client Applications
                           │
                           ▼
                    Gateway Service
                           │
                           ▼
                    Orchestration
                           │
           ┌───────────────┼───────────────┐
           ▼               ▼               ▼
       Context         Retrieval       Memory
           │               │               │
           └───────┬───────┴───────┬───────┘
                   ▼               ▼
              Embeddings      Providers
                       │
                       ▼
                    Storage
```

Business logic is implemented inside reusable libraries.

Services remain thin wrappers around those libraries.

---

# Features

## Context Engine

- Context assembly
- Context ranking
- Context compression
- Prompt optimization

## Retrieval

- Semantic retrieval
- Hybrid retrieval
- Metadata filtering
- Result reranking

## Memory

- Short-term memory
- Long-term memory
- Episodic memory
- Semantic memory

## Provider Abstraction

Supports multiple providers through common interfaces.

Planned integrations include:

- OpenAI
- Anthropic
- Google Gemini
- Ollama
- Voyage AI

## Storage

- PostgreSQL
- Redis
- Qdrant
- Blob storage

## Observability

- Structured logging
- Distributed tracing
- OpenTelemetry
- Metrics

---

# Workspace Structure

```text
contextra/

services/
libs/
proto/
sdk/
configs/
deployments/
docs/
tests/
```

## Services

Deployable applications.

Examples:

- Gateway
- Worker
- CLI

## Libraries

Reusable business logic.

Examples:

- Context
- Retrieval
- Memory
- Storage
- Providers
- Embeddings
- Orchestration

---

# Project Status

Current development phase:

```text
Phase 1

✓ Workspace
✓ Documentation
✓ Architecture

⏳ Foundation Libraries

⏳ Storage

⏳ Providers

⏳ Embeddings

⏳ Retrieval

⏳ Memory

⏳ Context Engine

⏳ Gateway

⏳ SDKs
```

---

# Technology Stack

## Language

- Rust

## Async Runtime

- Tokio

## HTTP

- Axum

## Configuration

- config-rs
- dotenvy

## Database

- PostgreSQL

## Cache

- Redis

## Vector Database

- Qdrant

## Observability

- tracing
- OpenTelemetry

## Serialization

- Serde

---

# Documentation

Project documentation is available under the `docs/` directory.

| Document | Description |
|-----------|-------------|
| architecture.md | High-level architecture |
| workspace.md | Workspace organization |
| libraries.md | Library responsibilities |
| services.md | Service architecture |
| context-engine.md | Context Engine |
| storage.md | Storage layer |
| providers.md | Provider abstraction |
| retrieval.md | Retrieval pipeline |
| memory.md | Memory system |
| orchestration.md | Workflow execution |
| api.md | API design |
| development.md | Development guidelines |

---

# Design Principles

Contextra is built around several core principles.

- Context-first architecture
- Provider independence
- Modular design
- Strong typing
- Storage abstraction
- Dependency inversion
- Observability by default
- Production-ready infrastructure

---

# Roadmap

## Phase 1

Workspace foundation

## Phase 2

Foundation libraries

- Errors
- Types
- Configuration
- Telemetry

## Phase 3

Storage layer

## Phase 4

Provider abstraction

## Phase 5

Embeddings

## Phase 6

Retrieval

## Phase 7

Memory

## Phase 8

Context Engine

## Phase 9

Gateway

## Phase 10

SDKs

---

# Contributing

Contributions are welcome.

Before contributing, please read:

- `docs/development.md`
- `docs/architecture.md`

---

# License

This project is licensed under the MIT License.

See the `LICENSE` file for details.