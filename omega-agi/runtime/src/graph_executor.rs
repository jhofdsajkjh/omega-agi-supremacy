//! # Graph Executor
//!
//! A directed acyclic graph (DAG) based task executor. Tasks are organized as
//! nodes in a graph with dependency edges. The executor processes nodes in
//! topological order, propagating results along edges.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use parking_lot::RwLock;
use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// Unique node identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(u64);

impl NodeId {
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        NodeId(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Node#{}", self.0)
    }
}

/// The result of executing a single node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeResult {
    /// The node that produced this result.
    pub node_id: NodeId,
    /// Whether the node execution succeeded.
    pub success: bool,
    /// Output data (JSON-serialized).
    pub output: Vec<u8>,
    /// Error message if execution failed.
    pub error: Option<String>,
    /// Execution time in microseconds.
    pub duration_us: u64,
}

impl NodeResult {
    /// Create a successful node result.
    pub fn ok(node_id: NodeId, output: Vec<u8>, duration_us: u64) -> Self {
        Self {
            node_id,
            success: true,
            output,
            error: None,
            duration_us,
        }
    }

    /// Create a failed node result.
    pub fn err(node_id: NodeId, error: impl Into<String>, duration_us: u64) -> Self {
        Self {
            node_id,
            success: false,
            output: Vec::new(),
            error: Some(error.into()),
            duration_us,
        }
    }

    /// Deserialize the output into a typed value.
    pub fn decode<T: serde::de::DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_slice(&self.output)
            .context("Failed to deserialize node output")
    }
}

/// A task node in the execution graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskNode {
    /// Unique identifier.
    pub id: NodeId,
    /// Human-readable name.
    pub name: String,
    /// Node type (e.g., "transform", "filter", "aggregate").
    pub node_type: String,
    /// Input parameters (JSON-serialized).
    pub params: Vec<u8>,
    /// Dependencies (node IDs that must complete before this node can run).
    pub dependencies: Vec<NodeId>,
}

impl TaskNode {
    /// Create a new task node.
    pub fn new(name: impl Into<String>, node_type: impl Into<String>) -> Self {
        Self {
            id: NodeId::new(),
            name: name.into(),
            node_type: node_type.into(),
            params: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    /// Add a dependency on another node.
    pub fn depends_on(mut self, node_id: NodeId) -> Self {
        self.dependencies.push(node_id);
        self
    }

    /// Set parameters for this node.
    pub fn with_params<T: Serialize>(mut self, params: &T) -> Result<Self> {
        self.params = serde_json::to_vec(params)
            .context("Failed to serialize node params")?;
        Ok(self)
    }

    /// Deserialize the parameters.
    pub fn decode_params<T: serde::de::DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_slice(&self.params)
            .context("Failed to deserialize node params")
    }
}

/// Errors specific to the graph executor.
#[derive(Debug, thiserror::Error)]
pub enum GraphExecutorError {
    /// A cycle was detected in the graph.
    #[error("cycle detected in task graph")]
    CycleDetected,

    /// A node references a dependency that doesn't exist.
    #[error("missing dependency: node {node} depends on {missing}")]
    MissingDependency { node: String, missing: String },

    /// A node failed during execution.
    #[error("node '{node}' failed: {error}")]
    NodeFailed { node: String, error: String },

    /// The graph is empty.
    #[error("task graph is empty")]
    EmptyGraph,
}

/// Statistics about graph execution.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphStats {
    pub total_nodes: usize,
    pub executed_nodes: usize,
    pub failed_nodes: usize,
    pub skipped_nodes: usize,
    pub total_duration_us: u64,
    pub has_cycle: bool,
}

/// A directed acyclic graph of tasks to be executed.
pub struct TaskGraph {
    nodes: HashMap<NodeId, TaskNode>,
    edges: Vec<(NodeId, NodeId)>, // (from, to) meaning "from" must run before "to"
}

impl TaskGraph {
    /// Create a new empty task graph.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
        }
    }

    /// Add a node to the graph.
    pub fn add_node(&mut self, node: TaskNode) -> NodeId {
        let id = node.id;
        // Register edges from dependencies
        for &dep_id in &node.dependencies {
            self.edges.push((dep_id, id));
        }
        self.nodes.insert(id, node);
        id
    }

    /// Get a node by ID.
    pub fn get_node(&self, id: NodeId) -> Option<&TaskNode> {
        self.nodes.get(&id)
    }

    /// Get all node IDs.
    pub fn node_ids(&self) -> Vec<NodeId> {
        self.nodes.keys().copied().collect()
    }

    /// Get the number of nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the graph is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Build a petgraph DiGraph for topological analysis.
    fn build_petgraph(&self) -> Result<(DiGraph<NodeId, ()>, HashMap<NodeId, NodeIndex>)> {
        let mut graph = DiGraph::new();
        let mut id_map = HashMap::new();

        // Add all nodes
        for &id in self.nodes.keys() {
            let idx = graph.add_node(id);
            id_map.insert(id, idx);
        }

        // Add edges
        for &(from, to) in &self.edges {
            let from_idx = id_map.get(&from).ok_or_else(|| {
                GraphExecutorError::MissingDependency {
                    node: to.to_string(),
                    missing: from.to_string(),
                }
            })?;
            let to_idx = id_map.get(&to).ok_or_else(|| {
                GraphExecutorError::MissingDependency {
                    node: from.to_string(),
                    missing: to.to_string(),
                }
            })?;
            graph.add_edge(*from_idx, *to_idx, ());
        }

        Ok((graph, id_map))
    }

    /// Check if the graph contains a cycle.
    pub fn has_cycle(&self) -> bool {
        self.build_petgraph()
            .map(|(g, _)| toposort(&g, None).is_err())
            .unwrap_or(false)
    }

    /// Get nodes in topological execution order.
    pub fn topological_order(&self) -> Result<Vec<NodeId>> {
        let (graph, id_map) = self.build_petgraph()?;
        let sorted = toposort(&graph, None).map_err(|_| GraphExecutorError::CycleDetected)?;

        Ok(sorted.iter().map(|idx| graph[*idx]).collect())
    }
}

impl Default for TaskGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// The graph executor processes a TaskGraph and executes nodes in topological order.
pub struct GraphExecutor {
    /// Handler function type stored as a string identifier for dispatch.
    handlers: Arc<RwLock<HashMap<String, Box<dyn Fn(&TaskNode, &[NodeResult]) -> Result<Vec<u8>> + Send + Sync>>>>,
}

impl GraphExecutor {
    /// Create a new graph executor.
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a handler for a node type.
    pub fn register_handler<F>(&self, node_type: impl Into<String>, handler: F)
    where
        F: Fn(&TaskNode, &[NodeResult]) -> Result<Vec<u8>> + Send + Sync + 'static,
    {
        self.handlers
            .write()
            .insert(node_type.into(), Box::new(handler));
    }

    /// Execute the task graph.
    ///
    /// Processes nodes in topological order. If a node fails, its dependents
    /// are skipped. Returns a map of node results.
    pub fn execute(&self, graph: &TaskGraph) -> Result<HashMap<NodeId, NodeResult>> {
        if graph.is_empty() {
            return Err(GraphExecutorError::EmptyGraph.into());
        }

        let order = graph.topological_order()?;
        let mut results: HashMap<NodeId, NodeResult> = HashMap::new();
        let mut failed: HashSet<NodeId> = HashSet::new();

        let total_start = Instant::now();

        for node_id in &order {
            let node = graph.get_node(*node_id).context("Node not found")?;

            // Check if any dependency failed
            let deps_failed = node
                .dependencies
                .iter()
                .any(|dep| failed.contains(dep));

            if deps_failed {
                debug!(node = %node_id, "Skipping node due to failed dependency");
                results.insert(
                    *node_id,
                    NodeResult::err(*node_id, "skipped: dependency failed", 0),
                );
                failed.insert(*node_id);
                continue;
            }

            // Gather dependency results
            let dep_results: Vec<NodeResult> = node
                .dependencies
                .iter()
                .filter_map(|dep| results.get(dep).cloned())
                .collect();

            let start = Instant::now();

            // Find handler
            let handlers = self.handlers.read();
            let handler = handlers.get(&node.node_type).ok_or_else(|| {
                GraphExecutorError::NodeFailed {
                    node: node.name.clone(),
                    error: format!("no handler registered for node type '{}'", node.node_type),
                }
            })?;

            match handler(node, &dep_results) {
                Ok(output) => {
                    let duration_us = start.elapsed().as_micros() as u64;
                    debug!(node = %node_id, duration_us, "Node executed successfully");
                    results.insert(*node_id, NodeResult::ok(*node_id, output, duration_us));
                }
                Err(e) => {
                    let duration_us = start.elapsed().as_micros() as u64;
                    let error_msg = e.to_string();
                    warn!(node = %node_id, error = %error_msg, "Node execution failed");
                    results.insert(
                        *node_id,
                        NodeResult::err(*node_id, &error_msg, duration_us),
                    );
                    failed.insert(*node_id);
                }
            }
        }

        let total_duration = total_start.elapsed().as_micros() as u64;
        info!(
            total_nodes = graph.len(),
            executed = results.len(),
            failed = failed.len(),
            total_duration_us = total_duration,
            "Graph execution complete"
        );

        Ok(results)
    }

    /// Get statistics about a completed execution.
    pub fn stats(results: &HashMap<NodeId, NodeResult>, total_nodes: usize) -> GraphStats {
        let executed = results.len();
        let failed = results.values().filter(|r| !r.success).count();
        let skipped = results.values().filter(|r| r.error.as_deref() == Some("skipped: dependency failed")).count();
        let total_duration: u64 = results.values().map(|r| r.duration_us).sum();

        GraphStats {
            total_nodes,
            executed_nodes: executed,
            failed_nodes: failed,
            skipped_nodes: skipped,
            total_duration_us: total_duration,
            has_cycle: false,
        }
    }
}

impl Default for GraphExecutor {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_unique() {
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_node_id_display() {
        let id = NodeId::new();
        let display = format!("{}", id);
        assert!(display.starts_with("Node#"));
    }

    #[test]
    fn test_task_node_creation() {
        let node = TaskNode::new("transform", "transform");
        assert_eq!(node.name, "transform");
        assert_eq!(node.node_type, "transform");
        assert!(node.dependencies.is_empty());
        assert!(node.params.is_empty());
    }

    #[test]
    fn test_task_node_with_params() {
        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct Params {
            factor: i32,
        }
        let node = TaskNode::new("scale", "transform")
            .with_params(&Params { factor: 2 })
            .unwrap();
        let decoded: Params = node.decode_params().unwrap();
        assert_eq!(decoded.factor, 2);
    }

    #[test]
    fn test_task_node_dependencies() {
        let dep = TaskNode::new("dep", "source");
        let dep_id = dep.id;
        let node = TaskNode::new("child", "transform").depends_on(dep_id);
        assert_eq!(node.dependencies, vec![dep_id]);
    }

    #[test]
    fn test_task_graph_add_node() {
        let mut graph = TaskGraph::new();
        let node = TaskNode::new("a", "source");
        let id = graph.add_node(node);
        assert_eq!(graph.len(), 1);
        assert!(graph.get_node(id).is_some());
    }

    #[test]
    fn test_task_graph_empty() {
        let graph = TaskGraph::new();
        assert!(graph.is_empty());
        assert_eq!(graph.len(), 0);
    }

    #[test]
    fn test_task_graph_topological_order() {
        let mut graph = TaskGraph::new();
        let a = graph.add_node(TaskNode::new("a", "source"));
        let b = graph.add_node(TaskNode::new("b", "source"));
        let c = graph.add_node(TaskNode::new("c", "merge").depends_on(a).depends_on(b));

        let order = graph.topological_order().unwrap();
        let c_pos = order.iter().position(|&id| id == c).unwrap();
        let a_pos = order.iter().position(|&id| id == a).unwrap();
        let b_pos = order.iter().position(|&id| id == b).unwrap();

        // c must come after both a and b
        assert!(c_pos > a_pos);
        assert!(c_pos > b_pos);
    }

    #[test]
    fn test_task_graph_cycle_detection() {
        let mut graph = TaskGraph::new();
        let a = graph.add_node(TaskNode::new("a", "source"));
        let b = graph.add_node(TaskNode::new("b", "source"));
        // Manually create a cycle by adding conflicting edges
        graph.edges.push((a, b));
        graph.edges.push((b, a));

        assert!(graph.has_cycle());
    }

    #[test]
    fn test_graph_executor_simple() {
        let executor = GraphExecutor::new();
        executor.register_handler("source", |_node, _deps| {
            Ok(serde_json::to_vec(&vec![1, 2, 3])?)
        });

        let mut graph = TaskGraph::new();
        graph.add_node(TaskNode::new("data", "source"));

        let results = executor.execute(&graph).unwrap();
        assert_eq!(results.len(), 1);

        let node_ids = graph.node_ids();
        let result = results.get(&node_ids[0]).unwrap();
        assert!(result.success);
        let data: Vec<i32> = result.decode().unwrap();
        assert_eq!(data, vec![1, 2, 3]);
    }

    #[test]
    fn test_graph_executor_with_dependencies() {
        let executor = GraphExecutor::new();

        executor.register_handler("source", |_node, _deps| {
            Ok(serde_json::to_vec(&42i32)?)
        });

        executor.register_handler("double", |_node, deps| {
            let input: i32 = deps[0].decode()?;
            Ok(serde_json::to_vec(&(input * 2))?)
        });

        let mut graph = TaskGraph::new();
        let source = graph.add_node(TaskNode::new("src", "source"));
        graph.add_node(TaskNode::new("dbl", "double").depends_on(source));

        let results = executor.execute(&graph).unwrap();
        assert!(results.values().all(|r| r.success));

        // Find the double node result
        let double_result = results.values().find(|r| r.node_id != source);
        assert!(double_result.is_some());
        let output: i32 = double_result.unwrap().decode().unwrap();
        assert_eq!(output, 84);
    }

    #[test]
    fn test_graph_executor_failure_skips_dependents() {
        let executor = GraphExecutor::new();

        executor.register_handler("fail", |_node, _deps| {
            anyhow::bail!("intentional failure")
        });

        executor.register_handler("dependent", |_node, _deps| {
            Ok(serde_json::to_vec(&"should not run")?)
        });

        let mut graph = TaskGraph::new();
        let fail_node = graph.add_node(TaskNode::new("fail", "fail"));
        graph.add_node(TaskNode::new("dep", "dependent").depends_on(fail_node));

        let results = executor.execute(&graph).unwrap();

        // The dependent should be skipped
        let dep_result = results.values().find(|r| r.node_id != fail_node).unwrap();
        assert!(!dep_result.success);
        assert!(dep_result.error.as_ref().unwrap().contains("skipped"));
    }

    #[test]
    fn test_graph_executor_empty_graph() {
        let executor = GraphExecutor::new();
        let graph = TaskGraph::new();
        let result = executor.execute(&graph);
        assert!(result.is_err());
    }

    #[test]
    fn test_graph_executor_unknown_node_type() {
        let executor = GraphExecutor::new();
        let mut graph = TaskGraph::new();
        graph.add_node(TaskNode::new("unknown", "nonexistent_type"));

        let results = executor.execute(&graph);
        assert!(results.is_err());
    }

    #[test]
    fn test_graph_executor_stats() {
        let executor = GraphExecutor::new();
        executor.register_handler("source", |_node, _deps| {
            Ok(serde_json::to_vec(&"ok")?)
        });

        let mut graph = TaskGraph::new();
        graph.add_node(TaskNode::new("a", "source"));
        graph.add_node(TaskNode::new("b", "source"));

        let results = executor.execute(&graph).unwrap();
        let stats = GraphExecutor::stats(&results, graph.len());

        assert_eq!(stats.total_nodes, 2);
        assert_eq!(stats.executed_nodes, 2);
        assert_eq!(stats.failed_nodes, 0);
        assert!(!stats.has_cycle);
    }

    #[test]
    fn test_node_result_ok() {
        let id = NodeId::new();
        let result = NodeResult::ok(id, b"output".to_vec(), 100);
        assert!(result.success);
        assert!(result.error.is_none());
        assert_eq!(result.duration_us, 100);
    }

    #[test]
    fn test_node_result_err() {
        let id = NodeId::new();
        let result = NodeResult::err(id, "failed badly", 50);
        assert!(!result.success);
        assert_eq!(result.error.as_deref(), Some("failed badly"));
    }

    #[test]
    fn test_node_result_decode() {
        let id = NodeId::new();
        let data = serde_json::to_vec(&vec!["x", "y"]).unwrap();
        let result = NodeResult::ok(id, data, 10);
        let decoded: Vec<String> = result.decode().unwrap();
        assert_eq!(decoded, vec!["x", "y"]);
    }

    #[test]
    fn test_graph_executor_error_display() {
        let err = GraphExecutorError::CycleDetected;
        assert!(format!("{}", err).contains("cycle"));

        let err = GraphExecutorError::EmptyGraph;
        assert!(format!("{}", err).contains("empty"));

        let err = GraphExecutorError::MissingDependency {
            node: "B".to_string(),
            missing: "A".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("B") && msg.contains("A"));
    }
}
