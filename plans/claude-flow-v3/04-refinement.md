# Claude-Flow v3 - SPARC Refinement

## Overview

This document outlines the Test-Driven Development (TDD) approach, iterative refinement strategies, and quality assurance processes for Claude-Flow v3.

---

## 1. Test-Driven Development Strategy

### 1.1 Test Pyramid

```
                    ┌──────────────┐
                   /│    E2E       │\
                  / │   Tests      │ \
                 /  │   (5%)       │  \
                /   └──────────────┘   \
               /    ┌──────────────┐    \
              /     │ Integration  │     \
             /      │   Tests      │      \
            /       │   (20%)      │       \
           /        └──────────────┘        \
          /         ┌──────────────┐         \
         /          │    Unit      │          \
        /           │   Tests      │           \
       /            │   (75%)      │            \
      /             └──────────────┘             \
     └───────────────────────────────────────────┘

     Distribution:
     • Unit Tests: 75% (Fast, isolated, per-function)
     • Integration Tests: 20% (Cross-component, NAPI bindings)
     • E2E Tests: 5% (Full workflows, CLI scenarios)
```

### 1.2 Test Categories

| Category | Scope | Framework | Coverage Target |
|----------|-------|-----------|-----------------|
| Rust Unit | Core crates | `cargo test` | 90%+ |
| TypeScript Unit | API layer | Jest | 85%+ |
| NAPI Integration | JS ↔ Rust bridge | Jest + Rust | 80%+ |
| WASM Integration | Browser runtime | Playwright | 75%+ |
| E2E Workflows | CLI + MCP | Custom harness | 70%+ |
| Performance | Latency/throughput | Criterion.rs | Key paths |

---

## 2. Unit Test Specifications

### 2.1 Vector Database Tests

```rust
// crates/claude-flow-vector/tests/hnsw_tests.rs

#[cfg(test)]
mod hnsw_tests {
    use claude_flow_vector::*;

    // ====================================================================
    // TEST GROUP: Initialization
    // ====================================================================

    #[test]
    fn test_create_hnsw_index_with_default_config() {
        // GIVEN: Default HNSW configuration
        let config = HnswConfig::default();

        // WHEN: Creating index
        let index = HnswIndex::new(config, DistanceMetric::Cosine);

        // THEN: Index should be empty and configured correctly
        assert!(index.is_ok());
        let index = index.unwrap();
        assert_eq!(index.len(), 0);
        assert_eq!(index.dimensions(), 0); // Not set until first insert
    }

    #[test]
    fn test_create_hnsw_index_with_custom_config() {
        // GIVEN: Custom configuration
        let config = HnswConfig {
            m: 64,
            ef_construction: 400,
            ef_search: 200,
            max_elements: 1_000_000,
        };

        // WHEN: Creating index
        let index = HnswIndex::new(config, DistanceMetric::Euclidean);

        // THEN: Index should use custom parameters
        assert!(index.is_ok());
    }

    #[test]
    fn test_reject_invalid_config() {
        // GIVEN: Invalid configuration (m = 0)
        let config = HnswConfig {
            m: 0,
            ..Default::default()
        };

        // WHEN: Creating index
        let result = HnswIndex::new(config, DistanceMetric::Cosine);

        // THEN: Should return error
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("m must be > 0"));
    }

    // ====================================================================
    // TEST GROUP: Insert Operations
    // ====================================================================

    #[test]
    fn test_insert_single_vector() {
        // GIVEN: Empty index
        let mut index = create_test_index(384);

        // WHEN: Inserting a vector
        let vector = vec![0.1; 384];
        let id = index.insert(&vector, Some("test-id".to_string()));

        // THEN: Vector should be indexed
        assert!(id.is_ok());
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn test_insert_batch_vectors() {
        // GIVEN: Empty index
        let mut index = create_test_index(384);

        // WHEN: Batch inserting 1000 vectors
        let vectors: Vec<Vec<f32>> = (0..1000)
            .map(|i| vec![i as f32 / 1000.0; 384])
            .collect();

        let ids = index.insert_batch(&vectors);

        // THEN: All vectors should be indexed
        assert!(ids.is_ok());
        assert_eq!(index.len(), 1000);
    }

    #[test]
    fn test_reject_wrong_dimension_vector() {
        // GIVEN: Index configured for 384 dimensions
        let mut index = create_test_index(384);
        index.insert(&vec![0.1; 384], None).unwrap();

        // WHEN: Inserting wrong dimension vector
        let wrong_vector = vec![0.1; 512]; // 512 instead of 384
        let result = index.insert(&wrong_vector, None);

        // THEN: Should return dimension mismatch error
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("dimension"));
    }

    #[test]
    fn test_reject_nan_values() {
        // GIVEN: Index
        let mut index = create_test_index(384);

        // WHEN: Inserting vector with NaN
        let mut vector = vec![0.1; 384];
        vector[0] = f32::NAN;
        let result = index.insert(&vector, None);

        // THEN: Should reject NaN values
        assert!(result.is_err());
    }

    // ====================================================================
    // TEST GROUP: Search Operations
    // ====================================================================

    #[test]
    fn test_search_returns_k_results() {
        // GIVEN: Index with 100 vectors
        let mut index = create_test_index(384);
        populate_index(&mut index, 100);

        // WHEN: Searching for top 10
        let query = vec![0.5; 384];
        let results = index.search(&query, 10);

        // THEN: Should return exactly 10 results
        assert!(results.is_ok());
        assert_eq!(results.unwrap().len(), 10);
    }

    #[test]
    fn test_search_results_ordered_by_similarity() {
        // GIVEN: Index with vectors
        let mut index = create_test_index(384);
        populate_index(&mut index, 100);

        // WHEN: Searching
        let query = vec![0.5; 384];
        let results = index.search(&query, 10).unwrap();

        // THEN: Results should be ordered by decreasing similarity
        for i in 1..results.len() {
            assert!(results[i - 1].score >= results[i].score);
        }
    }

    #[test]
    fn test_search_empty_index_returns_empty() {
        // GIVEN: Empty index
        let index = create_test_index(384);

        // WHEN: Searching
        let query = vec![0.5; 384];
        let results = index.search(&query, 10);

        // THEN: Should return empty results
        assert!(results.is_ok());
        assert!(results.unwrap().is_empty());
    }

    #[test]
    fn test_search_k_larger_than_index() {
        // GIVEN: Index with 5 vectors
        let mut index = create_test_index(384);
        populate_index(&mut index, 5);

        // WHEN: Searching for top 100
        let query = vec![0.5; 384];
        let results = index.search(&query, 100);

        // THEN: Should return only 5 results
        assert!(results.is_ok());
        assert_eq!(results.unwrap().len(), 5);
    }

    // ====================================================================
    // TEST GROUP: Distance Metrics
    // ====================================================================

    #[test]
    fn test_cosine_similarity_normalized() {
        // GIVEN: Two normalized vectors
        let a = normalize(&[1.0, 0.0, 0.0]);
        let b = normalize(&[1.0, 0.0, 0.0]);

        // WHEN: Computing cosine distance
        let distance = cosine_distance(&a, &b);

        // THEN: Should be 0 (identical vectors)
        assert!((distance - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_orthogonal_vectors() {
        // GIVEN: Orthogonal vectors
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];

        // WHEN: Computing cosine distance
        let distance = cosine_distance(&a, &b);

        // THEN: Should be 1 (orthogonal)
        assert!((distance - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_euclidean_distance() {
        // GIVEN: Two vectors
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![3.0, 4.0, 0.0];

        // WHEN: Computing Euclidean distance
        let distance = euclidean_distance(&a, &b);

        // THEN: Should be 5 (3-4-5 triangle)
        assert!((distance - 5.0).abs() < 1e-6);
    }

    // ====================================================================
    // TEST GROUP: Quantization
    // ====================================================================

    #[test]
    fn test_f16_quantization_roundtrip() {
        // GIVEN: Original vector
        let original = vec![0.1, 0.5, 0.9, -0.3];

        // WHEN: Quantizing to f16 and back
        let quantized = quantize_f16(&original);
        let reconstructed = dequantize_f16(&quantized);

        // THEN: Should be approximately equal
        for (a, b) in original.iter().zip(reconstructed.iter()) {
            assert!((a - b).abs() < 0.01); // Allow small error
        }
    }

    #[test]
    fn test_pq8_reduces_memory() {
        // GIVEN: 384-dimensional vector (1536 bytes)
        let vector = vec![0.5; 384];

        // WHEN: Quantizing to PQ8
        let quantized = quantize_pq8(&vector, 8); // 8 subvectors

        // THEN: Should be ~8x smaller
        assert!(quantized.len() < vector.len() * 4 / 4); // bytes vs f32
    }

    // ====================================================================
    // HELPER FUNCTIONS
    // ====================================================================

    fn create_test_index(dims: usize) -> HnswIndex {
        HnswIndex::new(
            HnswConfig {
                m: 16,
                ef_construction: 100,
                ef_search: 50,
                max_elements: 10_000,
            },
            DistanceMetric::Cosine,
        )
        .unwrap()
    }

    fn populate_index(index: &mut HnswIndex, count: usize) {
        for i in 0..count {
            let vector: Vec<f32> = (0..384)
                .map(|j| ((i + j) as f32) / 1000.0)
                .collect();
            index.insert(&vector, None).unwrap();
        }
    }

    fn normalize(v: &[f32]) -> Vec<f32> {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter().map(|x| x / norm).collect()
    }
}
```

### 2.2 Swarm Orchestration Tests

```rust
// crates/claude-flow-core/tests/swarm_tests.rs

#[cfg(test)]
mod swarm_tests {
    use claude_flow_core::swarm::*;
    use tokio::test;

    // ====================================================================
    // TEST GROUP: Swarm Initialization
    // ====================================================================

    #[tokio::test]
    async fn test_create_swarm_with_mesh_topology() {
        // GIVEN: Mesh topology configuration
        let config = SwarmConfig {
            topology: Topology::Mesh,
            max_agents: 100,
            ..Default::default()
        };

        // WHEN: Creating swarm
        let swarm = Swarm::new(config).await;

        // THEN: Swarm should be initialized with mesh topology
        assert!(swarm.is_ok());
        let swarm = swarm.unwrap();
        assert_eq!(swarm.topology_type(), "mesh");
        assert_eq!(swarm.agent_count(), 0);
    }

    #[tokio::test]
    async fn test_create_swarm_with_hierarchical_topology() {
        // GIVEN: Hierarchical topology configuration
        let config = SwarmConfig {
            topology: Topology::Hierarchical { levels: 3 },
            max_agents: 100,
            ..Default::default()
        };

        // WHEN: Creating swarm
        let swarm = Swarm::new(config).await;

        // THEN: Swarm should be initialized with hierarchy
        assert!(swarm.is_ok());
    }

    // ====================================================================
    // TEST GROUP: Agent Lifecycle
    // ====================================================================

    #[tokio::test]
    async fn test_spawn_agent() {
        // GIVEN: Empty swarm
        let swarm = create_test_swarm().await;

        // WHEN: Spawning an agent
        let agent_id = swarm.spawn_agent(AgentConfig {
            agent_type: "worker".to_string(),
            ..Default::default()
        }).await;

        // THEN: Agent should be registered
        assert!(agent_id.is_ok());
        assert_eq!(swarm.agent_count(), 1);
    }

    #[tokio::test]
    async fn test_spawn_agent_respects_capacity() {
        // GIVEN: Swarm with max 2 agents
        let swarm = create_swarm_with_capacity(2).await;

        // WHEN: Spawning 3 agents
        swarm.spawn_agent(default_agent_config()).await.unwrap();
        swarm.spawn_agent(default_agent_config()).await.unwrap();
        let result = swarm.spawn_agent(default_agent_config()).await;

        // THEN: Third spawn should fail
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("capacity"));
    }

    #[tokio::test]
    async fn test_despawn_agent() {
        // GIVEN: Swarm with one agent
        let swarm = create_test_swarm().await;
        let agent_id = swarm.spawn_agent(default_agent_config()).await.unwrap();

        // WHEN: Despawning the agent
        let result = swarm.despawn_agent(&agent_id).await;

        // THEN: Agent should be removed
        assert!(result.is_ok());
        assert_eq!(swarm.agent_count(), 0);
    }

    #[tokio::test]
    async fn test_agent_health_monitoring() {
        // GIVEN: Swarm with agent
        let swarm = create_test_swarm().await;
        let agent_id = swarm.spawn_agent(default_agent_config()).await.unwrap();

        // WHEN: Checking health after heartbeat
        swarm.report_heartbeat(&agent_id).await.unwrap();
        let health = swarm.get_agent_health(&agent_id).await;

        // THEN: Agent should be healthy
        assert!(health.is_ok());
        assert_eq!(health.unwrap().status, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_agent_marked_unhealthy_on_missed_heartbeats() {
        // GIVEN: Swarm with agent and short heartbeat timeout
        let config = SwarmConfig {
            heartbeat_timeout: Duration::from_millis(100),
            ..Default::default()
        };
        let swarm = Swarm::new(config).await.unwrap();
        let agent_id = swarm.spawn_agent(default_agent_config()).await.unwrap();

        // WHEN: Waiting longer than timeout without heartbeat
        tokio::time::sleep(Duration::from_millis(200)).await;

        // THEN: Agent should be marked unhealthy
        let health = swarm.get_agent_health(&agent_id).await.unwrap();
        assert_eq!(health.status, HealthStatus::Unhealthy);
    }

    // ====================================================================
    // TEST GROUP: Task Distribution
    // ====================================================================

    #[tokio::test]
    async fn test_distribute_task_to_available_agent() {
        // GIVEN: Swarm with one ready agent
        let swarm = create_test_swarm().await;
        swarm.spawn_agent(default_agent_config()).await.unwrap();

        // WHEN: Distributing a task
        let task = Task {
            id: "task-1".to_string(),
            payload: serde_json::json!({"action": "test"}),
            ..Default::default()
        };
        let assignment = swarm.distribute_task(task).await;

        // THEN: Task should be assigned
        assert!(assignment.is_ok());
        assert!(assignment.unwrap().agent_id.is_some());
    }

    #[tokio::test]
    async fn test_distribute_task_with_capability_filter() {
        // GIVEN: Swarm with agents having different capabilities
        let swarm = create_test_swarm().await;

        swarm.spawn_agent(AgentConfig {
            agent_type: "coder".to_string(),
            capabilities: vec!["code".to_string()],
            ..Default::default()
        }).await.unwrap();

        swarm.spawn_agent(AgentConfig {
            agent_type: "reviewer".to_string(),
            capabilities: vec!["review".to_string()],
            ..Default::default()
        }).await.unwrap();

        // WHEN: Distributing task requiring "review" capability
        let task = Task {
            required_capability: Some("review".to_string()),
            ..Default::default()
        };
        let assignment = swarm.distribute_task(task).await.unwrap();

        // THEN: Should be assigned to reviewer
        let agent = swarm.get_agent(&assignment.agent_id.unwrap()).await.unwrap();
        assert_eq!(agent.agent_type, "reviewer");
    }

    #[tokio::test]
    async fn test_round_robin_load_balancing() {
        // GIVEN: Swarm with 3 agents and round-robin balancing
        let config = SwarmConfig {
            load_balancing: LoadBalancing::RoundRobin,
            ..Default::default()
        };
        let swarm = Swarm::new(config).await.unwrap();

        let agent_ids: Vec<_> = futures::future::join_all(
            (0..3).map(|_| swarm.spawn_agent(default_agent_config()))
        ).await.into_iter().map(|r| r.unwrap()).collect();

        // WHEN: Distributing 6 tasks
        let mut assignments = Vec::new();
        for _ in 0..6 {
            let assignment = swarm.distribute_task(Task::default()).await.unwrap();
            assignments.push(assignment.agent_id.unwrap());
        }

        // THEN: Each agent should get 2 tasks
        for agent_id in &agent_ids {
            let count = assignments.iter().filter(|a| *a == agent_id).count();
            assert_eq!(count, 2);
        }
    }

    // ====================================================================
    // TEST GROUP: Consensus
    // ====================================================================

    #[tokio::test]
    async fn test_consensus_proposal_with_quorum() {
        // GIVEN: Swarm with 3 agents (quorum = 2)
        let config = SwarmConfig {
            enable_consensus: true,
            ..Default::default()
        };
        let swarm = Swarm::new(config).await.unwrap();

        for _ in 0..3 {
            swarm.spawn_agent(default_agent_config()).await.unwrap();
        }

        // WHEN: Proposing a value
        let result = swarm.propose_consensus("test-value".to_string()).await;

        // THEN: Should achieve consensus
        assert!(result.is_ok());
        assert!(result.unwrap().committed);
    }

    #[tokio::test]
    async fn test_consensus_fails_without_quorum() {
        // GIVEN: Swarm with only 1 agent (needs 2 for quorum of 3)
        let config = SwarmConfig {
            enable_consensus: true,
            consensus_quorum: 3,
            ..Default::default()
        };
        let swarm = Swarm::new(config).await.unwrap();
        swarm.spawn_agent(default_agent_config()).await.unwrap();

        // WHEN: Proposing a value
        let result = swarm.propose_consensus("test-value".to_string()).await;

        // THEN: Should fail to achieve quorum
        assert!(result.is_err() || !result.unwrap().committed);
    }

    // ====================================================================
    // HELPER FUNCTIONS
    // ====================================================================

    async fn create_test_swarm() -> Swarm {
        Swarm::new(SwarmConfig::default()).await.unwrap()
    }

    async fn create_swarm_with_capacity(max_agents: usize) -> Swarm {
        Swarm::new(SwarmConfig {
            max_agents,
            ..Default::default()
        }).await.unwrap()
    }

    fn default_agent_config() -> AgentConfig {
        AgentConfig {
            agent_type: "worker".to_string(),
            ..Default::default()
        }
    }
}
```

### 2.3 TypeScript API Tests

```typescript
// npm/claude-flow/tests/api.test.ts

import { ClaudeFlow, SwarmConfig, VectorEntry } from '../src';

describe('ClaudeFlow API', () => {
    let flow: ClaudeFlow;

    beforeEach(async () => {
        flow = await ClaudeFlow.create();
    });

    afterEach(async () => {
        await flow.close();
    });

    // ================================================================
    // TEST GROUP: Vector Database
    // ================================================================

    describe('VectorDB', () => {
        test('should insert and retrieve vector', async () => {
            // GIVEN: A vector entry
            const entry: VectorEntry = {
                vector: new Float32Array([0.1, 0.2, 0.3]),
                metadata: { label: 'test' }
            };

            // WHEN: Inserting and retrieving
            const id = await flow.vectorDb.insert(entry);
            const retrieved = await flow.vectorDb.get(id);

            // THEN: Should match
            expect(retrieved).toBeDefined();
            expect(retrieved!.metadata.label).toBe('test');
        });

        test('should perform similarity search', async () => {
            // GIVEN: Multiple vectors
            const vectors = [
                { vector: new Float32Array([1, 0, 0]), metadata: { idx: 0 } },
                { vector: new Float32Array([0, 1, 0]), metadata: { idx: 1 } },
                { vector: new Float32Array([0, 0, 1]), metadata: { idx: 2 } },
            ];

            for (const v of vectors) {
                await flow.vectorDb.insert(v);
            }

            // WHEN: Searching for similar vector
            const query = new Float32Array([0.9, 0.1, 0]);
            const results = await flow.vectorDb.search({ vector: query, k: 2 });

            // THEN: First result should be closest
            expect(results).toHaveLength(2);
            expect(results[0].entry.metadata.idx).toBe(0);
        });

        test('should handle batch insert', async () => {
            // GIVEN: 1000 vectors
            const entries = Array.from({ length: 1000 }, (_, i) => ({
                vector: new Float32Array(Array(384).fill(i / 1000)),
                metadata: { idx: i }
            }));

            // WHEN: Batch inserting
            const ids = await flow.vectorDb.insertBatch(entries);

            // THEN: All should be inserted
            expect(ids).toHaveLength(1000);
        });
    });

    // ================================================================
    // TEST GROUP: Swarm Orchestration
    // ================================================================

    describe('Swarm', () => {
        test('should initialize swarm', async () => {
            // GIVEN: Swarm configuration
            const config: SwarmConfig = {
                topology: 'mesh',
                maxAgents: 10
            };

            // WHEN: Initializing
            await flow.swarm.init(config);

            // THEN: Swarm should be active
            const status = await flow.swarm.status();
            expect(status.active).toBe(true);
            expect(status.topology).toBe('mesh');
        });

        test('should spawn and despawn agents', async () => {
            // GIVEN: Initialized swarm
            await flow.swarm.init({ topology: 'mesh', maxAgents: 10 });

            // WHEN: Spawning agents
            const agent1 = await flow.swarm.spawnAgent({ type: 'worker' });
            const agent2 = await flow.swarm.spawnAgent({ type: 'worker' });

            // THEN: Should have 2 agents
            expect(await flow.swarm.agentCount()).toBe(2);

            // WHEN: Despawning one
            await flow.swarm.despawnAgent(agent1);

            // THEN: Should have 1 agent
            expect(await flow.swarm.agentCount()).toBe(1);
        });

        test('should distribute tasks', async () => {
            // GIVEN: Swarm with agent
            await flow.swarm.init({ topology: 'mesh', maxAgents: 10 });
            await flow.swarm.spawnAgent({ type: 'worker' });

            // WHEN: Submitting task
            const assignment = await flow.swarm.submitTask({
                type: 'test',
                payload: { action: 'process' }
            });

            // THEN: Task should be assigned
            expect(assignment.agentId).toBeDefined();
            expect(assignment.status).toBe('assigned');
        });
    });

    // ================================================================
    // TEST GROUP: Memory Management
    // ================================================================

    describe('Memory', () => {
        test('should store and retrieve memory', async () => {
            // GIVEN: Memory entry
            const key = 'test-key';
            const value = { data: 'test-value', timestamp: Date.now() };

            // WHEN: Storing and retrieving
            await flow.memory.store(key, value);
            const retrieved = await flow.memory.retrieve(key);

            // THEN: Should match
            expect(retrieved).toEqual(value);
        });

        test('should perform semantic search on memory', async () => {
            // GIVEN: Multiple memory entries
            await flow.memory.store('entry-1', { content: 'JavaScript programming' });
            await flow.memory.store('entry-2', { content: 'Python development' });
            await flow.memory.store('entry-3', { content: 'Cooking recipes' });

            // WHEN: Searching semantically
            const results = await flow.memory.search('coding languages', { k: 2 });

            // THEN: Should return relevant entries
            expect(results).toHaveLength(2);
            expect(results.some(r => r.key === 'entry-1')).toBe(true);
            expect(results.some(r => r.key === 'entry-2')).toBe(true);
        });

        test('should support namespaces', async () => {
            // GIVEN: Entries in different namespaces
            await flow.memory.store('key', { data: 'ns1' }, { namespace: 'ns1' });
            await flow.memory.store('key', { data: 'ns2' }, { namespace: 'ns2' });

            // WHEN: Retrieving from each namespace
            const val1 = await flow.memory.retrieve('key', { namespace: 'ns1' });
            const val2 = await flow.memory.retrieve('key', { namespace: 'ns2' });

            // THEN: Should be isolated
            expect(val1.data).toBe('ns1');
            expect(val2.data).toBe('ns2');
        });
    });

    // ================================================================
    // TEST GROUP: AgentDB Compatibility
    // ================================================================

    describe('AgentDB Compatibility', () => {
        test('should support legacy insert API', async () => {
            // GIVEN: Legacy AgentDB-style entry
            const entry = {
                id: 'legacy-id',
                embedding: [0.1, 0.2, 0.3],
                metadata: { source: 'legacy' }
            };

            // WHEN: Using legacy API
            const id = await flow.agentDb.insert(entry);

            // THEN: Should work
            expect(id).toBe('legacy-id');
        });

        test('should support legacy search API', async () => {
            // GIVEN: Legacy entries
            await flow.agentDb.insert({
                embedding: [1, 0, 0],
                metadata: { idx: 0 }
            });

            // WHEN: Using legacy search
            const results = await flow.agentDb.search([0.9, 0.1, 0], 1);

            // THEN: Should return results
            expect(results).toHaveLength(1);
        });
    });
});
```

---

## 3. Integration Test Specifications

### 3.1 NAPI-RS Bridge Tests

```rust
// tests/integration/napi_bridge_test.rs

use std::process::Command;

#[test]
fn test_napi_binding_loads_on_platform() {
    // Run Node.js test that loads the native binding
    let output = Command::new("node")
        .args(["--eval", r#"
            const binding = require('@claude-flow/core');
            console.log('loaded:', typeof binding.VectorDB);
            process.exit(binding.VectorDB ? 0 : 1);
        "#])
        .output()
        .expect("Failed to run Node.js");

    assert!(output.status.success(), "NAPI binding failed to load");
}

#[test]
fn test_napi_async_operations() {
    let output = Command::new("node")
        .args(["--eval", r#"
            const { VectorDB } = require('@claude-flow/core');

            (async () => {
                const db = new VectorDB({ dimensions: 3 });
                const id = await db.insert({
                    vector: new Float32Array([0.1, 0.2, 0.3])
                });
                console.log('inserted:', id);
                process.exit(id ? 0 : 1);
            })().catch(e => {
                console.error(e);
                process.exit(1);
            });
        "#])
        .output()
        .expect("Failed to run Node.js");

    assert!(output.status.success(), "NAPI async operation failed");
}

#[test]
fn test_napi_error_propagation() {
    let output = Command::new("node")
        .args(["--eval", r#"
            const { VectorDB } = require('@claude-flow/core');

            (async () => {
                const db = new VectorDB({ dimensions: 3 });
                try {
                    // Invalid: wrong dimensions
                    await db.insert({
                        vector: new Float32Array([0.1, 0.2]) // 2 instead of 3
                    });
                    process.exit(1); // Should have thrown
                } catch (e) {
                    console.log('error caught:', e.message);
                    process.exit(e.message.includes('dimension') ? 0 : 1);
                }
            })();
        "#])
        .output()
        .expect("Failed to run Node.js");

    assert!(output.status.success(), "Error propagation failed");
}
```

### 3.2 Cross-Component Integration

```typescript
// tests/integration/cross-component.test.ts

import { ClaudeFlow } from '../src';

describe('Cross-Component Integration', () => {
    let flow: ClaudeFlow;

    beforeAll(async () => {
        flow = await ClaudeFlow.create({
            vectorDb: { dimensions: 384 },
            swarm: { maxAgents: 100 }
        });
    });

    afterAll(async () => {
        await flow.close();
    });

    test('agent stores and retrieves from vector memory', async () => {
        // Initialize swarm
        await flow.swarm.init({ topology: 'mesh', maxAgents: 10 });
        const agentId = await flow.swarm.spawnAgent({ type: 'worker' });

        // Agent stores embedding
        const embedding = new Float32Array(384).fill(0.5);
        const memoryId = await flow.memory.store(
            'agent-context',
            { thought: 'Processing user request...' },
            {
                namespace: `agent:${agentId}`,
                embedding
            }
        );

        // Agent retrieves context
        const context = await flow.memory.search(
            'processing request',
            { namespace: `agent:${agentId}`, k: 5 }
        );

        expect(context).toHaveLength(1);
        expect(context[0].value.thought).toContain('Processing');
    });

    test('task completion updates agent metrics', async () => {
        // Initialize
        await flow.swarm.init({ topology: 'mesh', maxAgents: 10 });
        const agentId = await flow.swarm.spawnAgent({ type: 'worker' });

        // Submit and complete task
        const task = await flow.swarm.submitTask({
            type: 'test',
            payload: {}
        });

        await flow.swarm.completeTask(task.id, { success: true });

        // Check metrics
        const metrics = await flow.swarm.getAgentMetrics(agentId);
        expect(metrics.tasksCompleted).toBe(1);
        expect(metrics.successRate).toBe(1.0);
    });

    test('federated memory sync between namespaces', async () => {
        // Store in namespace A
        await flow.memory.store('shared-key', { data: 'value-a' }, {
            namespace: 'ns-a',
            federated: true
        });

        // Configure federation
        await flow.memory.configureFederation({
            namespaces: ['ns-a', 'ns-b'],
            syncInterval: 100
        });

        // Wait for sync
        await new Promise(r => setTimeout(r, 200));

        // Should be available in namespace B
        const synced = await flow.memory.retrieve('shared-key', {
            namespace: 'ns-b',
            allowFederated: true
        });

        expect(synced.data).toBe('value-a');
    });
});
```

---

## 4. Performance Benchmarks

### 4.1 Rust Benchmarks (Criterion)

```rust
// benches/vector_benchmarks.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use claude_flow_vector::*;

fn benchmark_hnsw_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("hnsw_insert");

    for size in [1000, 10_000, 100_000].iter() {
        group.bench_with_input(BenchmarkId::new("vectors", size), size, |b, &size| {
            let mut index = create_benchmark_index();

            b.iter(|| {
                let vector: Vec<f32> = (0..384).map(|_| rand::random()).collect();
                black_box(index.insert(&vector, None))
            });
        });
    }

    group.finish();
}

fn benchmark_hnsw_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("hnsw_search");

    for size in [1000, 10_000, 100_000].iter() {
        // Prepare index
        let mut index = create_benchmark_index();
        for _ in 0..*size {
            let vector: Vec<f32> = (0..384).map(|_| rand::random()).collect();
            index.insert(&vector, None).unwrap();
        }

        for k in [10, 100].iter() {
            group.bench_with_input(
                BenchmarkId::new(format!("size_{}_k_{}", size, k), size),
                &(size, k),
                |b, _| {
                    let query: Vec<f32> = (0..384).map(|_| rand::random()).collect();
                    b.iter(|| black_box(index.search(&query, *k)))
                }
            );
        }
    }

    group.finish();
}

fn benchmark_quantization(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantization");

    let vector: Vec<f32> = (0..384).map(|_| rand::random()).collect();

    group.bench_function("f16_encode", |b| {
        b.iter(|| black_box(quantize_f16(&vector)))
    });

    group.bench_function("pq8_encode", |b| {
        b.iter(|| black_box(quantize_pq8(&vector, 8)))
    });

    group.finish();
}

fn benchmark_distance_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("distance_metrics");

    let a: Vec<f32> = (0..384).map(|_| rand::random()).collect();
    let b: Vec<f32> = (0..384).map(|_| rand::random()).collect();

    group.bench_function("cosine_384", |b_iter| {
        b_iter.iter(|| black_box(cosine_distance(&a, &b)))
    });

    group.bench_function("euclidean_384", |b_iter| {
        b_iter.iter(|| black_box(euclidean_distance(&a, &b)))
    });

    group.bench_function("dot_product_384", |b_iter| {
        b_iter.iter(|| black_box(dot_product(&a, &b)))
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_hnsw_insert,
    benchmark_hnsw_search,
    benchmark_quantization,
    benchmark_distance_metrics
);
criterion_main!(benches);

fn create_benchmark_index() -> HnswIndex {
    HnswIndex::new(
        HnswConfig {
            m: 32,
            ef_construction: 200,
            ef_search: 100,
            max_elements: 1_000_000,
        },
        DistanceMetric::Cosine,
    ).unwrap()
}
```

### 4.2 Performance Targets

| Benchmark | Target | Measurement |
|-----------|--------|-------------|
| `hnsw_insert` (100k vectors) | < 100µs/op | Criterion |
| `hnsw_search` (k=10, 100k vectors) | < 100µs | Criterion |
| `hnsw_search` (k=100, 100k vectors) | < 500µs | Criterion |
| `cosine_distance` (384 dims) | < 200ns | Criterion |
| `f16_quantize` (384 dims) | < 1µs | Criterion |
| `batch_insert` (1k vectors) | < 50ms | Jest |
| `swarm_spawn` (100 agents) | < 1s | Jest |
| `memory_sync` (1k entries) | < 100ms | Jest |

---

## 5. Iterative Refinement Process

### 5.1 Development Cycles

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        TDD Development Cycle                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐   │
│  │  RED    │───▶│  GREEN  │───▶│REFACTOR │───▶│BENCHMARK│───▶│ COMMIT  │   │
│  │(Write  │    │(Minimal │    │(Improve │    │(Verify  │    │(Document│   │
│  │ Test)  │    │ Pass)   │    │ Design) │    │ Perf)   │    │ Changes)│   │
│  └─────────┘    └─────────┘    └─────────┘    └─────────┘    └─────────┘   │
│       │                                                           │         │
│       └───────────────────────────────────────────────────────────┘         │
│                            Next Feature                                      │
│                                                                              │
│  Cycle Duration: 30 min - 2 hours                                           │
│  Commits: After each GREEN/REFACTOR phase                                   │
│  Benchmarks: Run on significant changes                                     │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Code Review Checklist

```markdown
## Pull Request Checklist

### Code Quality
- [ ] All tests pass (`cargo test`, `npm test`)
- [ ] No new warnings (`cargo clippy`, `npm run lint`)
- [ ] Type coverage maintained (`npm run typecheck`)
- [ ] Documentation updated for public APIs

### Performance
- [ ] Benchmarks show no regression (< 5% variance)
- [ ] Memory usage within bounds
- [ ] No unnecessary allocations in hot paths

### Security
- [ ] Input validation for all external data
- [ ] No hardcoded secrets
- [ ] Resource limits enforced

### Compatibility
- [ ] AgentDB API compatibility maintained
- [ ] WASM fallback tested
- [ ] Cross-platform CI passes

### Testing
- [ ] Unit tests for new functions (90%+ coverage)
- [ ] Integration tests for cross-component changes
- [ ] Edge cases covered (empty, max size, errors)
```

### 5.3 Regression Prevention

```yaml
# .github/workflows/ci.yml

name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        node: [18, 20, 22]

    runs-on: ${{ matrix.os }}

    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: ${{ matrix.node }}

      - name: Rust Tests
        run: cargo test --all-features

      - name: Build NAPI
        run: npm run build:napi

      - name: TypeScript Tests
        run: npm test

      - name: Integration Tests
        run: npm run test:integration

  benchmarks:
    runs-on: ubuntu-latest
    if: github.event_name == 'pull_request'

    steps:
      - uses: actions/checkout@v4

      - name: Run Benchmarks
        run: cargo bench -- --save-baseline pr

      - name: Compare with main
        run: |
          git fetch origin main
          git checkout origin/main
          cargo bench -- --save-baseline main
          git checkout -
          cargo bench -- --baseline main --compare

      - name: Post Results
        uses: actions/github-script@v7
        with:
          script: |
            // Post benchmark comparison as PR comment
```

---

## 6. Quality Gates

### 6.1 Merge Requirements

| Gate | Requirement | Tool |
|------|-------------|------|
| Tests | 100% pass | `cargo test`, `npm test` |
| Coverage | > 80% | `cargo-llvm-cov`, `jest --coverage` |
| Linting | 0 errors | `cargo clippy`, `eslint` |
| Types | 100% | `cargo check`, `tsc --noEmit` |
| Benchmarks | No regression > 10% | Criterion |
| Security | No high/critical | `cargo audit`, `npm audit` |

### 6.2 Release Criteria

| Criteria | Description |
|----------|-------------|
| All tests pass | CI green on all platforms |
| Documentation complete | API docs, migration guide |
| Performance validated | Benchmarks meet targets |
| Backward compatibility | AgentDB API tests pass |
| Security audit | No known vulnerabilities |
| CHANGELOG updated | Version and changes documented |

---

*Document Version: 1.0.0*
*Last Updated: 2025-11-27*
