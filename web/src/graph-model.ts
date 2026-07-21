import type { AnalysisTree, AnalysisTreeNode } from "./types";

export interface OrderedTreeNode {
  node: AnalysisTreeNode;
  children: OrderedTreeNode[];
}

export function buildOrderedTree(tree: AnalysisTree): OrderedTreeNode {
  const nodes = new Map<string, AnalysisTreeNode>();
  for (const node of tree.nodes) {
    if (nodes.has(node.id)) {
      throw new Error(`duplicate tree node: ${node.id}`);
    }
    nodes.set(node.id, node);
  }
  if (!nodes.has(tree.root_id)) {
    throw new Error(`missing tree root: ${tree.root_id}`);
  }

  const children = new Map<string, Array<{ id: string; order: number }>>();
  const parents = new Map<string, string>();
  for (const edge of tree.edges) {
    if (!nodes.has(edge.parent_id)) {
      throw new Error(`missing tree parent: ${edge.parent_id}`);
    }
    if (!nodes.has(edge.child_id)) {
      throw new Error(`missing tree child: ${edge.child_id}`);
    }
    if (parents.has(edge.child_id)) {
      throw new Error(`multiple parents for tree node: ${edge.child_id}`);
    }
    parents.set(edge.child_id, edge.parent_id);
    const siblings = children.get(edge.parent_id) ?? [];
    siblings.push({ id: edge.child_id, order: edge.order });
    children.set(edge.parent_id, siblings);
  }

  const state = new Map<string, "visiting" | "visited">();
  const detectCycle = (id: string): void => {
    if (state.get(id) === "visiting") {
      throw new Error("tree contains a cycle");
    }
    if (state.get(id) === "visited") {
      return;
    }
    state.set(id, "visiting");
    for (const child of children.get(id) ?? []) {
      detectCycle(child.id);
    }
    state.set(id, "visited");
  };
  for (const id of nodes.keys()) {
    detectCycle(id);
  }

  const reachable = new Set<string>();
  const build = (id: string): OrderedTreeNode => {
    reachable.add(id);
    const orderedChildren = [...(children.get(id) ?? [])].sort(
      (left, right) => left.order - right.order,
    );
    return {
      node: nodes.get(id)!,
      children: orderedChildren.map((child) => build(child.id)),
    };
  };
  const root = build(tree.root_id);
  for (const id of nodes.keys()) {
    if (!reachable.has(id)) {
      throw new Error(`disconnected tree node: ${id}`);
    }
  }
  return root;
}
