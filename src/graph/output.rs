use petgraph::graph::DiGraph;
use serde_json::{json, Value};

use super::builder::{EdgeKind, NodeKind};

/// Serialize the graph to a consumer-friendly JSON shape.
///
/// Do NOT use petgraph's built-in serde output — it produces opaque numeric
/// node IDs in a schema that D3, Gephi, and Cytoscape don't understand.
/// Walk node_indices() and edge_indices() manually instead.
///
/// Target shape:
/// ```json
/// {
///   "input": "...",
///   "nodes": [
///     { "id": 0, "type": "token", "surface": "ゴミ", ... },
///     { "id": 4, "type": "pattern", "name": "しか", ... }
///   ],
///   "edges": [
///     { "source": 0, "target": 1, "type": "sequence" },
///     { "source": 1, "target": 4, "type": "pattern_span" }
///   ]
/// }
/// ```
///
/// ## Your job
///
/// Implement the walk:
///   for idx in graph.node_indices() → serialize graph[idx] with its NodeIndex as id
///   for edge in graph.edge_indices() → serialize with source, target, type
pub fn to_json(graph: &DiGraph<NodeKind, EdgeKind>, input: &str) -> Value {
    // TODO: implement JSON serialization
    // Hint:
    //   graph.node_indices()  → iterator of NodeIndex
    //   graph[node_idx]       → &NodeKind
    //   graph.edge_indices()  → iterator of EdgeIndex
    //   graph.edge_endpoints(edge_idx) → Option<(NodeIndex, NodeIndex)>
    //   graph[edge_idx]       → &EdgeKind
    //   NodeIndex.index()     → usize  (use as the id field)

    json!({
        "input": input,
        "nodes": [],
        "edges": []
    })
}

/// Serialize the graph to Graphviz DOT format.
///
/// Token nodes: labeled with their surface form.
/// Pattern nodes: labeled with "name (jlpt)".
/// Edge labels: "seq", "span", "end".
///
/// ## Your job
///
/// Walk node_indices() and edge_indices() to build a DOT string.
/// A minimal valid DOT file looks like:
///   digraph { 0 [label="ゴミ"]; 1 [label="しか"]; 0 -> 1 [label="seq"]; }
pub fn to_dot(graph: &DiGraph<NodeKind, EdgeKind>) -> String {
    // TODO: implement DOT serialization
    let _ = graph;
    "digraph {}".to_string()
}
