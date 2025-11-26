# RuVector

[![MIT License](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/ruvector-core.svg)](https://crates.io/crates/ruvector-core)
[![npm](https://img.shields.io/npm/v/ruvector.svg)](https://www.npmjs.com/package/ruvector)
[![Rust](https://img.shields.io/badge/rust-1.77%2B-orange.svg)](https://www.rust-lang.org)
[![Build](https://img.shields.io/github/actions/workflow/status/ruvnet/ruvector/ci.yml?branch=main)](https://github.com/ruvnet/ruvector/actions)
[![Docs](https://img.shields.io/badge/docs-latest-brightgreen.svg)](./docs/)

**A vector database that learns.** Store embeddings, query with Cypher, and let the index improve itself through Graph Neural Networks.

```bash
npx ruvector
```

## What Problem Does RuVector Solve?

Traditional vector databases just store and search. When you ask "find similar items," they return results but never get smarter.

**RuVector is different:**

1. **Store vectors** like any vector DB (embeddings from OpenAI, Cohere, etc.)
2. **Query with Cypher** like Neo4j (`MATCH (a)-[:SIMILAR]->(b) RETURN b`)
3. **The index learns** — GNN layers make search results improve over time

Think of it as: **Pinecone + Neo4j + PyTorch** in one Rust package.

## Quick Start

### Node.js / Browser

```bash
# Install
npm install ruvector

# Or try instantly
npx ruvector
```

```javascript
const ruvector = require('ruvector');

// Vector search
const db = new ruvector.VectorDB(128);
db.insert('doc1', embedding1);
const results = db.search(queryEmbedding, 10);

// Graph queries (Cypher)
db.execute("CREATE (a:Person {name: 'Alice'})-[:KNOWS]->(b:Person {name: 'Bob'})");
db.execute("MATCH (p:Person)-[:KNOWS]->(friend) RETURN friend.name");

// GNN-enhanced search
const layer = new ruvector.GNNLayer(128, 256, 4);
const enhanced = layer.forward(query, neighbors, weights);

// Compression (2-32x memory savings)
const compressed = ruvector.compress(embedding, 0.3);
```

### Rust

```bash
cargo add ruvector-graph ruvector-gnn
```

```rust
use ruvector_graph::{GraphDB, NodeBuilder};
use ruvector_gnn::{RuvectorLayer, differentiable_search};

let db = GraphDB::new();

let doc = NodeBuilder::new("doc1")
    .label("Document")
    .property("embedding", vec![0.1, 0.2, 0.3])
    .build();
db.create_node(doc)?;

// GNN layer
let layer = RuvectorLayer::new(128, 256, 4, 0.1);
let enhanced = layer.forward(&query, &neighbors, &weights);
```

## Features

| Feature | What It Does | Why It Matters |
|---------|--------------|----------------|
| **Vector Search** | HNSW index, <0.5ms latency | Fast enough for real-time apps |
| **Cypher Queries** | `MATCH`, `WHERE`, `CREATE`, `RETURN` | Familiar Neo4j syntax |
| **GNN Layers** | Neural network on index topology | Search improves with usage |
| **Hyperedges** | Connect 3+ nodes at once | Model complex relationships |
| **Tensor Compression** | f32→f16→PQ8→PQ4→Binary | 2-32x memory reduction |
| **Differentiable Search** | Soft attention k-NN | End-to-end trainable |
| **WASM/Browser** | Full client-side support | Run AI search offline |

## Comparison

| Feature | RuVector | Pinecone | Qdrant | Milvus | ChromaDB |
|---------|----------|----------|--------|--------|----------|
| **Latency (p50)** | <0.5ms | ~2ms | ~1ms | ~5ms | ~50ms |
| **Memory (1M vec)** | 200MB* | 2GB | 1.5GB | 1GB | 3GB |
| **Graph Queries** | ✅ Cypher | ❌ | ❌ | ❌ | ❌ |
| **Hyperedges** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Self-Learning (GNN)** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Auto-Compression** | ✅ 2-32x | ❌ | ❌ | ✅ | ❌ |
| **Browser/WASM** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Differentiable** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Open Source** | ✅ MIT | ❌ | ✅ | ✅ | ✅ |

*With PQ8 compression

## How the GNN Works

Traditional vector search:
```
Query → HNSW Index → Top K Results
```

RuVector with GNN:
```
Query → HNSW Index → GNN Layer → Enhanced Results
                ↑                      │
                └──── learns from ─────┘
```

The GNN layer:
1. Takes your query and its nearest neighbors
2. Applies multi-head attention to weigh which neighbors matter
3. Updates representations based on graph structure
4. Returns better-ranked results

Over time, frequently-accessed paths get reinforced, making common queries faster and more accurate.

## Compression Tiers

RuVector automatically compresses cold data:

| Access Frequency | Format | Compression | Example |
|-----------------|--------|-------------|---------|
| Hot (>80%) | f32 | 1x | Active queries |
| Warm (40-80%) | f16 | 2x | Recent docs |
| Cool (10-40%) | PQ8 | 8x | Older content |
| Cold (1-10%) | PQ4 | 16x | Archives |
| Archive (<1%) | Binary | 32x | Rarely used |

## Use Cases

**RAG (Retrieval-Augmented Generation)**
```javascript
const context = ruvector.search(questionEmbedding, 5);
const prompt = `Context: ${context.join('\n')}\n\nQuestion: ${question}`;
```

**Recommendation Systems**
```cypher
MATCH (user:User)-[:VIEWED]->(item:Product)
MATCH (item)-[:SIMILAR_TO]->(rec:Product)
RETURN rec ORDER BY rec.score DESC LIMIT 10
```

**Knowledge Graphs**
```cypher
MATCH (concept:Concept)-[:RELATES_TO*1..3]->(related)
RETURN related
```

## Installation

| Platform | Command |
|----------|---------|
| **npm** | `npm install ruvector` |
| **Browser/WASM** | `npm install ruvector-wasm` |
| **Rust** | `cargo add ruvector-core ruvector-graph ruvector-gnn` |

## Documentation

| Topic | Link |
|-------|------|
| Getting Started | [docs/guide/GETTING_STARTED.md](./docs/guide/GETTING_STARTED.md) |
| Cypher Reference | [docs/api/CYPHER_REFERENCE.md](./docs/api/CYPHER_REFERENCE.md) |
| GNN Architecture | [docs/gnn-layer-implementation.md](./docs/gnn-layer-implementation.md) |
| Node.js API | [crates/ruvector-gnn-node/README.md](./crates/ruvector-gnn-node/README.md) |
| WASM API | [crates/ruvector-gnn-wasm/README.md](./crates/ruvector-gnn-wasm/README.md) |
| Performance Tuning | [docs/optimization/PERFORMANCE_TUNING_GUIDE.md](./docs/optimization/PERFORMANCE_TUNING_GUIDE.md) |
| API Reference | [docs/api/](./docs/api/) |

## Project Structure

```
crates/
├── ruvector-core/       # Vector DB engine (HNSW, storage)
├── ruvector-graph/      # Graph DB + Cypher parser
├── ruvector-gnn/        # GNN layers, compression, training
├── ruvector-gnn-wasm/   # WebAssembly bindings
└── ruvector-gnn-node/   # Node.js bindings (napi-rs)
```

## Contributing

We welcome contributions! See [CONTRIBUTING.md](./docs/development/CONTRIBUTING.md).

```bash
# Run tests
cargo test --workspace

# Run benchmarks
cargo bench --workspace

# Build WASM
cargo build -p ruvector-gnn-wasm --target wasm32-unknown-unknown
```

## License

MIT License — free for commercial and personal use.

---

<div align="center">

**Built by [rUv](https://ruv.io)** • [GitHub](https://github.com/ruvnet/ruvector) • [npm](https://npmjs.com/package/ruvector) • [Docs](./docs/)

*Vector search that gets smarter over time.*

</div>
