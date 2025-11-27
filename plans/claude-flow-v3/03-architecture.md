# Claude-Flow v3 - SPARC Architecture

## Overview

This document defines the system architecture for Claude-Flow v3, detailing component structure, data flow, integration patterns, and deployment topology.

---

## 1. High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Claude-Flow v3 Architecture                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                        JavaScript/TypeScript API                      │   │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐     │   │
│  │  │   Swarm    │  │   Memory   │  │   Task     │  │   Neural   │     │   │
│  │  │    API     │  │    API     │  │    API     │  │    API     │     │   │
│  │  └─────┬──────┘  └─────┬──────┘  └─────┬──────┘  └─────┬──────┘     │   │
│  └────────┼────────────────┼────────────────┼────────────────┼──────────┘   │
│           │                │                │                │              │
│  ┌────────┴────────────────┴────────────────┴────────────────┴──────────┐   │
│  │                    Platform Abstraction Layer                         │   │
│  │  ┌─────────────────────────┐  ┌─────────────────────────┐            │   │
│  │  │    NAPI-RS Bindings     │  │     WASM Bindings       │            │   │
│  │  │   (Native Performance)  │  │   (Universal Fallback)  │            │   │
│  │  └───────────┬─────────────┘  └───────────┬─────────────┘            │   │
│  └──────────────┼────────────────────────────┼──────────────────────────┘   │
│                 │                            │                              │
│  ┌──────────────┴────────────────────────────┴──────────────────────────┐   │
│  │                         Rust Core Engine                              │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                │   │
│  │  │  Vector DB   │  │    Graph     │  │     GNN      │                │   │
│  │  │    Core      │  │   Engine     │  │   Engine     │                │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘                │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                │   │
│  │  │    Swarm     │  │   Memory     │  │    Task      │                │   │
│  │  │Orchestration │  │ Management   │  │Orchestration │                │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘                │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                │   │
│  │  │    Raft      │  │    QUIC      │  │   Router     │                │   │
│  │  │  Consensus   │  │    Sync      │  │  (TinyDancer)│                │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘                │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Component Architecture

### 2.1 Rust Workspace Structure

```
claude-flow-v3/
├── Cargo.toml                      # Workspace definition
├── crates/
│   ├── claude-flow-core/           # Core orchestration engine
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── swarm/              # Swarm orchestration
│   │   │   │   ├── mod.rs
│   │   │   │   ├── topology.rs     # Mesh, hierarchical, adaptive
│   │   │   │   ├── agent.rs        # Agent lifecycle
│   │   │   │   ├── balancer.rs     # Load balancing
│   │   │   │   └── health.rs       # Health monitoring
│   │   │   ├── memory/             # Memory management
│   │   │   │   ├── mod.rs
│   │   │   │   ├── hierarchical.rs # Memory tiers
│   │   │   │   ├── federated.rs    # Cross-node sync
│   │   │   │   └── namespace.rs    # Multi-tenancy
│   │   │   ├── task/               # Task orchestration
│   │   │   │   ├── mod.rs
│   │   │   │   ├── dag.rs          # DAG execution
│   │   │   │   ├── queue.rs        # Priority queue
│   │   │   │   ├── retry.rs        # Retry logic
│   │   │   │   └── circuit.rs      # Circuit breaker
│   │   │   ├── consensus/          # Distributed consensus
│   │   │   │   ├── mod.rs
│   │   │   │   ├── raft.rs         # Raft protocol
│   │   │   │   └── gossip.rs       # Gossip protocol
│   │   │   └── protocol/           # Communication
│   │   │       ├── mod.rs
│   │   │       ├── message.rs      # Message types
│   │   │       └── pubsub.rs       # Pub/sub
│   │   └── Cargo.toml
│   │
│   ├── claude-flow-vector/         # Vector database (RuVector-based)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── hnsw.rs             # HNSW index
│   │   │   ├── storage.rs          # Storage backends
│   │   │   ├── quantization.rs     # Compression
│   │   │   └── agentdb.rs          # AgentDB compatibility
│   │   └── Cargo.toml
│   │
│   ├── claude-flow-graph/          # Graph engine
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── cypher/             # Query language
│   │   │   ├── graph.rs            # Graph structures
│   │   │   └── hyperedge.rs        # Multi-agent relations
│   │   └── Cargo.toml
│   │
│   ├── claude-flow-gnn/            # Neural network engine
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── layer.rs            # GNN layers
│   │   │   ├── training.rs         # Training loops
│   │   │   └── reflexion.rs        # Self-learning
│   │   └── Cargo.toml
│   │
│   ├── claude-flow-router/         # AI routing (TinyDancer)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── fastgrnn.rs         # FastGRNN model
│   │   │   └── cascade.rs          # Model cascading
│   │   └── Cargo.toml
│   │
│   ├── claude-flow-node/           # NAPI-RS bindings
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── swarm.rs            # Swarm API
│   │   │   ├── memory.rs           # Memory API
│   │   │   ├── task.rs             # Task API
│   │   │   └── vector.rs           # Vector API
│   │   ├── build.rs
│   │   └── Cargo.toml
│   │
│   └── claude-flow-wasm/           # WASM bindings
│       ├── src/
│       │   ├── lib.rs
│       │   └── indexeddb.rs        # Browser storage
│       └── Cargo.toml
│
├── npm/
│   ├── claude-flow/                # Main npm package
│   │   ├── package.json
│   │   ├── src/
│   │   │   ├── index.ts            # Entry point
│   │   │   ├── loader.ts           # Platform detection
│   │   │   ├── types.ts            # TypeScript definitions
│   │   │   └── compat/             # v2.x compatibility
│   │   │       └── agentdb.ts
│   │   └── tsconfig.json
│   │
│   └── platforms/                  # Platform-specific binaries
│       ├── darwin-arm64/
│       ├── darwin-x64/
│       ├── linux-arm64-gnu/
│       ├── linux-x64-gnu/
│       └── win32-x64-msvc/
│
├── tests/
│   ├── rust/                       # Rust unit tests
│   ├── integration/                # Integration tests
│   └── benchmarks/                 # Performance benchmarks
│
└── docs/
    ├── api/                        # API documentation
    └── migration/                  # v2.x migration guide
```

### 2.2 Crate Dependency Graph

```
                    ┌─────────────────────┐
                    │  claude-flow-node   │
                    │    (NAPI-RS)        │
                    └──────────┬──────────┘
                               │
            ┌──────────────────┼──────────────────┐
            │                  │                  │
            ▼                  ▼                  ▼
┌───────────────────┐ ┌───────────────────┐ ┌───────────────────┐
│ claude-flow-core  │ │ claude-flow-gnn   │ │ claude-flow-router│
│  (Orchestration)  │ │   (Learning)      │ │   (Routing)       │
└─────────┬─────────┘ └─────────┬─────────┘ └─────────┬─────────┘
          │                     │                     │
          ├─────────────────────┼─────────────────────┤
          │                     │                     │
          ▼                     ▼                     ▼
┌───────────────────┐ ┌───────────────────┐ ┌───────────────────┐
│claude-flow-vector │ │ claude-flow-graph │ │   ruvector-*      │
│  (Vector DB)      │ │  (Graph Engine)   │ │ (Base Libraries)  │
└───────────────────┘ └───────────────────┘ └───────────────────┘
```

---

## 3. Data Flow Architecture

### 3.1 Request Processing Flow

```
┌────────────┐     ┌────────────┐     ┌────────────┐     ┌────────────┐
│   User     │────▶│ TypeScript │────▶│  NAPI-RS   │────▶│ Rust Core  │
│  Request   │     │    API     │     │  Binding   │     │  Engine    │
└────────────┘     └────────────┘     └────────────┘     └─────┬──────┘
                                                               │
                   ┌───────────────────────────────────────────┘
                   │
                   ▼
    ┌──────────────────────────────────────────────────────────────┐
    │                    Request Router                             │
    │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐     │
    │  │  Swarm   │  │  Memory  │  │   Task   │  │  Neural  │     │
    │  │ Handler  │  │ Handler  │  │ Handler  │  │ Handler  │     │
    │  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘     │
    └───────┼─────────────┼─────────────┼─────────────┼────────────┘
            │             │             │             │
            ▼             ▼             ▼             ▼
    ┌──────────────────────────────────────────────────────────────┐
    │                    Execution Layer                            │
    │  ┌─────────────────────────────────────────────────────────┐ │
    │  │              Async Task Pool (Tokio)                    │ │
    │  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐   │ │
    │  │  │ Worker  │  │ Worker  │  │ Worker  │  │ Worker  │   │ │
    │  │  │   #1    │  │   #2    │  │   #3    │  │   #N    │   │ │
    │  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘   │ │
    │  └─────────────────────────────────────────────────────────┘ │
    └──────────────────────────────────────────────────────────────┘
            │             │             │             │
            ▼             ▼             ▼             ▼
    ┌──────────────────────────────────────────────────────────────┐
    │                    Storage Layer                              │
    │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐     │
    │  │  Vector  │  │  Graph   │  │   Raft   │  │  Cache   │     │
    │  │ Storage  │  │ Storage  │  │   Log    │  │  (LRU)   │     │
    │  └──────────┘  └──────────┘  └──────────┘  └──────────┘     │
    └──────────────────────────────────────────────────────────────┘
```

### 3.2 Swarm Communication Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Swarm Communication Architecture                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│    ┌──────────────┐          ┌──────────────┐          ┌──────────────┐    │
│    │   Agent A    │◀────────▶│  Coordinator │◀────────▶│   Agent B    │    │
│    │  (Worker)    │          │   (Leader)   │          │  (Worker)    │    │
│    └──────┬───────┘          └──────┬───────┘          └──────┬───────┘    │
│           │                         │                         │            │
│           │    ┌────────────────────┼────────────────────┐    │            │
│           │    │                    │                    │    │            │
│           ▼    ▼                    ▼                    ▼    ▼            │
│    ┌──────────────────────────────────────────────────────────────┐        │
│    │                    Message Bus (Pub/Sub)                      │        │
│    │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐         │        │
│    │  │  tasks  │  │  status │  │consensus│  │  health │         │        │
│    │  │  topic  │  │  topic  │  │  topic  │  │  topic  │         │        │
│    │  └─────────┘  └─────────┘  └─────────┘  └─────────┘         │        │
│    └──────────────────────────────────────────────────────────────┘        │
│           │                         │                         │            │
│           ▼                         ▼                         ▼            │
│    ┌──────────────────────────────────────────────────────────────┐        │
│    │                    Shared Memory Layer                        │        │
│    │  ┌──────────────────────────────────────────────────────┐    │        │
│    │  │           Vector DB (Agent State & Context)           │    │        │
│    │  │  ┌─────────┐  ┌─────────┐  ┌─────────┐              │    │        │
│    │  │  │Namespace│  │Namespace│  │Namespace│              │    │        │
│    │  │  │Agent A  │  │Agent B  │  │ Shared  │              │    │        │
│    │  │  └─────────┘  └─────────┘  └─────────┘              │    │        │
│    │  └──────────────────────────────────────────────────────┘    │        │
│    └──────────────────────────────────────────────────────────────┘        │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Distributed Architecture

### 4.1 Cluster Topology

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Claude-Flow v3 Cluster                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Region: US-East                    Region: US-West                          │
│  ┌─────────────────────┐           ┌─────────────────────┐                  │
│  │  ┌───────────────┐  │           │  ┌───────────────┐  │                  │
│  │  │    Leader     │  │◀─────────▶│  │   Follower    │  │                  │
│  │  │  (Raft Node)  │  │   QUIC    │  │  (Raft Node)  │  │                  │
│  │  └───────┬───────┘  │           │  └───────┬───────┘  │                  │
│  │          │          │           │          │          │                  │
│  │  ┌───────┴───────┐  │           │  ┌───────┴───────┐  │                  │
│  │  │    Agents     │  │           │  │    Agents     │  │                  │
│  │  │  ┌───┐ ┌───┐  │  │           │  │  ┌───┐ ┌───┐  │  │                  │
│  │  │  │ A │ │ B │  │  │           │  │  │ C │ │ D │  │  │                  │
│  │  │  └───┘ └───┘  │  │           │  │  └───┘ └───┘  │  │                  │
│  │  └───────────────┘  │           │  └───────────────┘  │                  │
│  │          │          │           │          │          │                  │
│  │  ┌───────┴───────┐  │           │  ┌───────┴───────┐  │                  │
│  │  │  Vector DB    │◀─┼───────────┼─▶│  Vector DB    │  │                  │
│  │  │  (Replicated) │  │  Gossip   │  │  (Replicated) │  │                  │
│  │  └───────────────┘  │           │  └───────────────┘  │                  │
│  └─────────────────────┘           └─────────────────────┘                  │
│                                                                              │
│                    ┌─────────────────────────┐                              │
│                    │    Region: EU-West      │                              │
│                    │  ┌───────────────────┐  │                              │
│                    │  │     Follower      │  │                              │
│                    │  │   (Raft Node)     │  │                              │
│                    │  └─────────┬─────────┘  │                              │
│                    │            │            │                              │
│                    │  ┌─────────┴─────────┐  │                              │
│                    │  │      Agents       │  │                              │
│                    │  │   ┌───┐ ┌───┐     │  │                              │
│                    │  │   │ E │ │ F │     │  │                              │
│                    │  │   └───┘ └───┘     │  │                              │
│                    │  └───────────────────┘  │                              │
│                    └─────────────────────────┘                              │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Consensus Protocol (Raft)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Raft Consensus Flow                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  1. Leader Election                                                          │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │  Follower ──(timeout)──▶ Candidate ──(majority votes)──▶ Leader      │   │
│  │      ▲                       │                              │         │   │
│  │      │                       │                              │         │   │
│  │      └───(loses election)────┘                              │         │   │
│  │      └───────────────────(higher term)──────────────────────┘         │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  2. Log Replication                                                          │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                                                                        │   │
│  │  Client ──(request)──▶ Leader ──(AppendEntries)──▶ Followers          │   │
│  │                           │                              │             │   │
│  │                           │         ┌────────────────────┘             │   │
│  │                           │         │                                  │   │
│  │                           ▼         ▼                                  │   │
│  │                     ┌─────────────────────┐                            │   │
│  │                     │  Quorum Achieved?   │                            │   │
│  │                     │  (N/2 + 1 ACKs)     │                            │   │
│  │                     └──────────┬──────────┘                            │   │
│  │                                │                                       │   │
│  │                     ┌──────────┴──────────┐                            │   │
│  │                     │                     │                            │   │
│  │                   Yes                    No                            │   │
│  │                     │                     │                            │   │
│  │                     ▼                     ▼                            │   │
│  │              Commit Entry          Wait/Retry                          │   │
│  │              Apply to FSM                                              │   │
│  │                                                                        │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  3. State Machine                                                            │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │  Raft Log: [Entry1] [Entry2] [Entry3] [Entry4] [Entry5]               │   │
│  │                               ▲         ▲                             │   │
│  │                               │         │                             │   │
│  │                          committed   replicated                       │   │
│  │                                                                        │   │
│  │  State Machine: { swarm_config, agent_registry, task_assignments }    │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Memory Architecture

### 5.1 Hierarchical Memory Structure

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    Hierarchical Memory Architecture                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Access Speed    Memory Tier           Capacity      Persistence             │
│  ──────────────────────────────────────────────────────────────────────────  │
│                                                                              │
│  < 1µs          ┌─────────────────┐                                         │
│  ◀────────────▶ │   IMMEDIATE     │   10-100 items   In-Memory (LRU)        │
│                 │  (BoundedDeque) │                                         │
│                 └────────┬────────┘                                         │
│                          │ promote                                          │
│                          ▼                                                  │
│  < 10µs         ┌─────────────────┐                                         │
│  ◀────────────▶ │   SHORT-TERM    │   1K-10K items   In-Memory (TTL)        │
│                 │   (TimedCache)  │                                         │
│                 └────────┬────────┘                                         │
│                          │ promote                                          │
│                          ▼                                                  │
│  < 100µs        ┌─────────────────┐                                         │
│  ◀────────────▶ │   LONG-TERM     │   100K-10M items File-backed (redb)     │
│                 │   (VectorDB)    │                                         │
│                 └────────┬────────┘                                         │
│                          │ embed                                            │
│                          ▼                                                  │
│  < 1ms          ┌─────────────────┐                                         │
│  ◀────────────▶ │    SEMANTIC     │   1M-100M items  HNSW Index             │
│                 │ (EmbeddingIndex)│                                         │
│                 └─────────────────┘                                         │
│                                                                              │
│  ──────────────────────────────────────────────────────────────────────────  │
│                                                                              │
│  Promotion Rules:                                                            │
│  • IMMEDIATE → SHORT-TERM: On LRU eviction                                  │
│  • SHORT-TERM → LONG-TERM: On importance >= 0.7                             │
│  • LONG-TERM → SEMANTIC: Automatic (embedding computed on insert)           │
│                                                                              │
│  Eviction Policy:                                                            │
│  • IMMEDIATE: LRU (Least Recently Used)                                     │
│  • SHORT-TERM: TTL (Time-To-Live, default 1 hour)                           │
│  • LONG-TERM: Importance score threshold (keep top 80%)                     │
│  • SEMANTIC: None (append-only with periodic compaction)                    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Federated Memory Sync

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     Federated Memory Synchronization                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Node A (Leader)              Node B (Follower)           Node C (Follower) │
│  ┌──────────────┐             ┌──────────────┐            ┌──────────────┐  │
│  │ Memory State │             │ Memory State │            │ Memory State │  │
│  │ ┌──────────┐ │             │ ┌──────────┐ │            │ ┌──────────┐ │  │
│  │ │VectorClock│ │             │ │VectorClock│ │            │ │VectorClock│ │  │
│  │ │ A:5      │ │             │ │ A:4      │ │            │ │ A:5      │ │  │
│  │ │ B:3      │ │             │ │ B:3      │ │            │ │ B:2      │ │  │
│  │ │ C:2      │ │             │ │ C:2      │ │            │ │ C:2      │ │  │
│  │ └──────────┘ │             │ └──────────┘ │            │ └──────────┘ │  │
│  └──────┬───────┘             └──────┬───────┘            └──────┬───────┘  │
│         │                            │                           │          │
│         │     QUIC Sync Message      │                           │          │
│         ├───────────────────────────▶│                           │          │
│         │  {                         │                           │          │
│         │    changes: [Entry@A:5],   │                           │          │
│         │    vector_clock: {A:5,B:3} │                           │          │
│         │  }                         │                           │          │
│         │                            │                           │          │
│         │◀───────────────────────────┤                           │          │
│         │  ACK + local changes       │                           │          │
│         │                            │                           │          │
│         │                            │     Gossip Protocol       │          │
│         │                            ├──────────────────────────▶│          │
│         │                            │  Propagate A:5 change     │          │
│         │                            │                           │          │
│                                                                              │
│  Conflict Resolution (Last-Write-Wins with Vector Clock):                   │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │  IF local_vc[key] < remote_vc[key]:                                   │   │
│  │      Apply remote value (remote is newer)                            │   │
│  │  ELSE IF local_vc[key] > remote_vc[key]:                              │   │
│  │      Keep local value (local is newer)                               │   │
│  │  ELSE IF local_vc[key] == remote_vc[key] AND values differ:          │   │
│  │      CRDT merge OR last-write-wins (configurable)                    │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Platform Binding Architecture

### 6.1 NAPI-RS Binding Layer

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        NAPI-RS Binding Architecture                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  JavaScript Runtime                                                          │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │  const flow = require('@claude-flow/core');                           │   │
│  │                                                                        │   │
│  │  // Zero-copy Float32Array                                            │   │
│  │  const vector = new Float32Array([0.1, 0.2, ...]);                    │   │
│  │                                                                        │   │
│  │  // Async operation                                                   │   │
│  │  const id = await flow.vectorDb.insert({ vector, metadata });         │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                              │                                               │
│                              │ NAPI-RS FFI                                   │
│                              ▼                                               │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │  #[napi]                                                              │   │
│  │  pub struct VectorDB {                                                │   │
│  │      inner: Arc<RwLock<CoreVectorDB>>,  // Thread-safe wrapper        │   │
│  │  }                                                                    │   │
│  │                                                                        │   │
│  │  #[napi]                                                              │   │
│  │  impl VectorDB {                                                      │   │
│  │      #[napi]                                                          │   │
│  │      pub async fn insert(&self, entry: JsVectorEntry) -> Result<String> {│
│  │          let db = self.inner.clone();                                 │   │
│  │          tokio::task::spawn_blocking(move || {                        │   │
│  │              let db = db.read().unwrap();                             │   │
│  │              db.insert(entry.to_core()?)                              │   │
│  │          }).await?                                                    │   │
│  │      }                                                                │   │
│  │  }                                                                    │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                              │                                               │
│                              │ Rust Core                                     │
│                              ▼                                               │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │  pub struct CoreVectorDB {                                            │   │
│  │      storage: Box<dyn VectorStorage>,                                 │   │
│  │      index: Box<dyn VectorIndex>,                                     │   │
│  │  }                                                                    │   │
│  │                                                                        │   │
│  │  impl CoreVectorDB {                                                  │   │
│  │      pub fn insert(&self, entry: VectorEntry) -> Result<VectorId> {   │   │
│  │          // High-performance Rust implementation                      │   │
│  │      }                                                                │   │
│  │  }                                                                    │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Platform Detection & Loading

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     Platform Detection & Loading Flow                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                    require('@claude-flow/core')                      │    │
│  └────────────────────────────────┬────────────────────────────────────┘    │
│                                   │                                          │
│                                   ▼                                          │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │              Detect Platform: os.platform() + os.arch()              │    │
│  │                                                                       │    │
│  │              linux-x64 │ linux-arm64 │ darwin-x64 │ darwin-arm64     │    │
│  │                        │ win32-x64                                   │    │
│  └────────────────────────────────┬────────────────────────────────────┘    │
│                                   │                                          │
│                    ┌──────────────┴──────────────┐                          │
│                    │                             │                          │
│                    ▼                             ▼                          │
│  ┌─────────────────────────────┐  ┌─────────────────────────────┐          │
│  │   Try Native Package        │  │   Platform Not Supported    │          │
│  │   @claude-flow/core-{plat}  │  │   Fall back to WASM         │          │
│  └──────────────┬──────────────┘  └──────────────┬──────────────┘          │
│                 │                                │                          │
│       ┌─────────┴─────────┐                      │                          │
│       │                   │                      │                          │
│       ▼                   ▼                      │                          │
│  ┌─────────┐         ┌─────────┐                 │                          │
│  │ Success │         │ Failure │                 │                          │
│  │  (100%) │         │ (WASM)  │                 │                          │
│  └────┬────┘         └────┬────┘                 │                          │
│       │                   │                      │                          │
│       │                   └──────────────────────┘                          │
│       │                              │                                      │
│       ▼                              ▼                                      │
│  ┌──────────────┐           ┌──────────────────────────────────┐           │
│  │ Return Native│           │    Load @claude-flow/wasm        │           │
│  │   Binding    │           │                                  │           │
│  │              │           │  ┌────────────────────────────┐  │           │
│  │ Performance: │           │  │ Check SIMD Support:        │  │           │
│  │   100%       │           │  │  detectSIMD() → true/false │  │           │
│  └──────────────┘           │  │                            │  │           │
│                             │  │ Load appropriate bundle:   │  │           │
│                             │  │  SIMD: pkg-simd/ (70-80%)  │  │           │
│                             │  │  Base: pkg/ (40-50%)       │  │           │
│                             │  └────────────────────────────┘  │           │
│                             └──────────────────────────────────┘           │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 7. NPM Package Structure

```
@claude-flow/                        # npm scope
├── core                             # Main package
│   ├── package.json
│   │   ├── optionalDependencies:
│   │   │   ├── @claude-flow/core-linux-x64-gnu
│   │   │   ├── @claude-flow/core-linux-arm64-gnu
│   │   │   ├── @claude-flow/core-darwin-x64
│   │   │   ├── @claude-flow/core-darwin-arm64
│   │   │   └── @claude-flow/core-win32-x64-msvc
│   │   └── dependencies:
│   │       └── @claude-flow/wasm    # Fallback
│   ├── src/
│   │   ├── index.ts                 # Entry point
│   │   ├── loader.ts                # Platform detection
│   │   ├── swarm.ts                 # Swarm API
│   │   ├── memory.ts                # Memory API
│   │   ├── task.ts                  # Task API
│   │   └── types.ts                 # TypeScript definitions
│   └── native/                      # Pre-built binaries (CI committed)
│       ├── linux-x64-gnu/
│       │   └── claude-flow.node
│       ├── linux-arm64-gnu/
│       │   └── claude-flow.node
│       ├── darwin-x64/
│       │   └── claude-flow.node
│       ├── darwin-arm64/
│       │   └── claude-flow.node
│       └── win32-x64-msvc/
│           └── claude-flow.node
│
├── core-linux-x64-gnu               # Platform-specific packages
│   ├── package.json                 # os: ["linux"], cpu: ["x64"]
│   └── claude-flow.node
├── core-linux-arm64-gnu
├── core-darwin-x64
├── core-darwin-arm64
├── core-win32-x64-msvc
│
├── wasm                             # WASM fallback
│   ├── package.json
│   ├── pkg/                         # Base WASM build
│   │   ├── claude_flow_wasm.js
│   │   ├── claude_flow_wasm.d.ts
│   │   └── claude_flow_wasm_bg.wasm
│   └── pkg-simd/                    # SIMD-enabled build
│       └── ...
│
└── claude-flow                      # Convenience wrapper (npx claude-flow@v3)
    └── package.json                 # CLI entry point
```

---

## 8. Security Architecture

### 8.1 Input Validation

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Security Validation Layer                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  External Input                                                              │
│       │                                                                      │
│       ▼                                                                      │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                    Validation Pipeline                               │    │
│  │                                                                       │    │
│  │  1. Size Limits                                                       │    │
│  │     ├── Vector dimensions: max 65,536                                │    │
│  │     ├── Metadata size: max 1MB                                       │    │
│  │     ├── Agent count: max 10,000                                      │    │
│  │     └── Query k: max 10,000                                          │    │
│  │                                                                       │    │
│  │  2. Type Validation                                                  │    │
│  │     ├── Vectors: Float32, no NaN/Infinity                            │    │
│  │     ├── IDs: UUID v4/v7 format                                       │    │
│  │     └── Metadata: Valid JSON, sanitized                              │    │
│  │                                                                       │    │
│  │  3. Rate Limiting                                                    │    │
│  │     ├── Per-agent: 1000 ops/sec                                      │    │
│  │     ├── Per-swarm: 10,000 ops/sec                                    │    │
│  │     └── Global: 100,000 ops/sec                                      │    │
│  │                                                                       │    │
│  │  4. Resource Limits                                                  │    │
│  │     ├── Memory: Configurable cap (default 512MB)                     │    │
│  │     ├── CPU: Timeout for operations (default 30s)                    │    │
│  │     └── Storage: Configurable cap (default 10GB)                     │    │
│  │                                                                       │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│       │                                                                      │
│       ▼                                                                      │
│  Validated Input → Core Processing                                          │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 8.2 Namespace Isolation

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Multi-Tenant Isolation                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────┐     │
│  │                         Namespace Manager                          │     │
│  │                                                                     │     │
│  │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐    │     │
│  │  │  Namespace: A   │  │  Namespace: B   │  │  Namespace: C   │    │     │
│  │  │                 │  │                 │  │                 │    │     │
│  │  │  ┌───────────┐  │  │  ┌───────────┐  │  │  ┌───────────┐  │    │     │
│  │  │  │ Vector DB │  │  │  │ Vector DB │  │  │  │ Vector DB │  │    │     │
│  │  │  │(Isolated) │  │  │  │(Isolated) │  │  │  │(Isolated) │  │    │     │
│  │  │  └───────────┘  │  │  └───────────┘  │  │  └───────────┘  │    │     │
│  │  │                 │  │                 │  │                 │    │     │
│  │  │  ┌───────────┐  │  │  ┌───────────┐  │  │  ┌───────────┐  │    │     │
│  │  │  │  Memory   │  │  │  │  Memory   │  │  │  │  Memory   │  │    │     │
│  │  │  │(Isolated) │  │  │  │(Isolated) │  │  │  │(Isolated) │  │    │     │
│  │  │  └───────────┘  │  │  └───────────┘  │  │  └───────────┘  │    │     │
│  │  │                 │  │                 │  │                 │    │     │
│  │  │  Quota: 100MB   │  │  Quota: 500MB   │  │  Quota: 1GB     │    │     │
│  │  │  Agents: 100    │  │  Agents: 500    │  │  Agents: 1000   │    │     │
│  │  └─────────────────┘  └─────────────────┘  └─────────────────┘    │     │
│  │                                                                     │     │
│  │  Cross-Namespace Access: DENIED (unless explicit federation)       │     │
│  └────────────────────────────────────────────────────────────────────┘     │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 9. Deployment Topologies

### 9.1 Single Node (Development)

```
┌─────────────────────────────┐
│    Single Node Deployment   │
│                             │
│  ┌───────────────────────┐  │
│  │   Claude-Flow v3      │  │
│  │                       │  │
│  │  ┌─────────────────┐  │  │
│  │  │  Swarm (Local)  │  │  │
│  │  └─────────────────┘  │  │
│  │  ┌─────────────────┐  │  │
│  │  │  Memory (Mem)   │  │  │
│  │  └─────────────────┘  │  │
│  │  ┌─────────────────┐  │  │
│  │  │  Vector (HNSW)  │  │  │
│  │  └─────────────────┘  │  │
│  │                       │  │
│  │  Storage: ~/.claude-flow/│
│  └───────────────────────┘  │
│                             │
│  Resources:                 │
│  • CPU: 2+ cores            │
│  • RAM: 4GB+                │
│  • Disk: 10GB+              │
└─────────────────────────────┘
```

### 9.2 Multi-Node (Production)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     Multi-Node Production Deployment                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Load Balancer (Nginx/HAProxy)                                              │
│         │                                                                    │
│         ├──────────────┬──────────────┬──────────────┐                      │
│         │              │              │              │                      │
│         ▼              ▼              ▼              ▼                      │
│  ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐                 │
│  │  Node 1  │   │  Node 2  │   │  Node 3  │   │  Node N  │                 │
│  │ (Leader) │   │(Follower)│   │(Follower)│   │(Follower)│                 │
│  │          │   │          │   │          │   │          │                 │
│  │ Raft: L  │◀─▶│ Raft: F  │◀─▶│ Raft: F  │◀─▶│ Raft: F  │                 │
│  │          │   │          │   │          │   │          │                 │
│  └────┬─────┘   └────┬─────┘   └────┬─────┘   └────┬─────┘                 │
│       │              │              │              │                        │
│       └──────────────┴──────────────┴──────────────┘                        │
│                           │                                                  │
│                           ▼                                                  │
│               ┌───────────────────────┐                                     │
│               │   Shared Storage      │                                     │
│               │   (S3/GCS/NFS)        │                                     │
│               └───────────────────────┘                                     │
│                                                                              │
│  Kubernetes Resources:                                                       │
│  • StatefulSet: 3-5 replicas                                                │
│  • PersistentVolumeClaim: 100GB per node                                    │
│  • Service: ClusterIP + LoadBalancer                                        │
│  • ConfigMap: Cluster configuration                                          │
│  • Secret: Authentication credentials                                        │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

*Document Version: 1.0.0*
*Last Updated: 2025-11-27*
