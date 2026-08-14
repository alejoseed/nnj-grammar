import type {
  AnalysisDocument,
  AnalysisTree,
  AnalysisTreeNode,
  AnalyzedToken,
  TreeNodeKind,
} from "./types";

export interface OrderedTreeNode {
  node: AnalysisTreeNode;
  children: OrderedTreeNode[];
}

export interface GraphNode {
  id: string;
  kind: TreeNodeKind;
  primaryLabel: string;
  secondaryLabel: string;
  children: GraphNode[];
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

function uniqueMap<T>(
  items: T[],
  idOf: (item: T) => string,
  label: string,
): Map<string, T> {
  const result = new Map<string, T>();
  for (const item of items) {
    const id = idOf(item);
    if (result.has(id)) {
      throw new Error(`duplicate ${label}: ${id}`);
    }
    result.set(id, item);
  }
  return result;
}

function spanTokens(
  node: AnalysisTreeNode,
  tokensByPosition: Map<number, AnalyzedToken>,
): AnalyzedToken[] {
  if (
    node.token_start === null ||
    node.token_end === null ||
    node.token_end < node.token_start
  ) {
    throw new Error(
      `invalid token span: ${String(node.token_start)}..${String(node.token_end)}`,
    );
  }
  const tokens: AnalyzedToken[] = [];
  for (let position = node.token_start; position <= node.token_end; position += 1) {
    const token = tokensByPosition.get(position);
    if (!token) {
      throw new Error(`invalid token span: ${node.token_start}..${node.token_end}`);
    }
    tokens.push(token);
  }
  return tokens;
}

export function buildGraphModel(document: AnalysisDocument): GraphNode {
  const ordered = buildOrderedTree(document.tree);
  if (ordered.node.kind !== "document") {
    throw new Error("tree root must be a document");
  }
  const tokensById = uniqueMap(
    document.tokens,
    (token) => token.id,
    "analyzed token",
  );
  const tokensByPosition = new Map(
    document.tokens.map((token) => [token.position, token]),
  );
  if (tokensByPosition.size !== document.tokens.length) {
    throw new Error("duplicate token position");
  }
  const matchesById = uniqueMap(
    document.primary_matches,
    (matched) => matched.id,
    "primary match",
  );
  const secondaryById = uniqueMap(
    document.secondary_matches,
    (matched) => matched.id,
    "secondary match",
  );

  const convert = ({ node, children }: OrderedTreeNode): GraphNode => {
    for (const secondaryId of node.secondary_match_ids) {
      if (!secondaryById.has(secondaryId)) {
        throw new Error(`missing secondary match: ${secondaryId}`);
      }
    }
    const attached = node.match_ids.map((matchId) => {
      const matched = matchesById.get(matchId);
      if (!matched) {
        throw new Error(`missing primary match: ${matchId}`);
      }
      // The node covers the match, never the reverse.
      if (
        node.token_start === null ||
        node.token_end === null ||
        matched.token_start < node.token_start ||
        matched.token_end > node.token_end
      ) {
        throw new Error(`match span escapes its tree node: ${matched.id}`);
      }
      return matched;
    });

    let primaryLabel = "";
    let secondaryLabel = "";
    if (node.kind === "sentence" || node.kind === "bunsetsu") {
      primaryLabel = spanTokens(node, tokensByPosition)
        .map((token) => token.surface)
        .join("");
      // A sentence label would duplicate its bunsetsu children; only show it
      // when a match is attached (a span no bunsetsu could cover).
      if (node.kind === "sentence" && attached.length === 0) {
        primaryLabel = "";
      }
      secondaryLabel = attached
        .map((matched) => matched.meaning_en.trim())
        .find((meaning) => meaning !== "") ?? "";
    } else if (node.kind === "token") {
      const token = tokensById.get(node.token_id ?? "");
      if (!token) {
        throw new Error(`missing analyzed token: ${String(node.token_id)}`);
      }
      if (node.token_start !== token.position || node.token_end !== token.position) {
        throw new Error(`tree and token positions differ: ${token.id}`);
      }
      primaryLabel = token.surface;
      secondaryLabel =
        token.glosses.find((gloss) => gloss.gloss.trim())?.gloss.trim() ??
        (token.reading !== token.surface ? token.reading : "");
    }

    return {
      id: node.id,
      kind: node.kind,
      primaryLabel,
      secondaryLabel,
      children: children.map(convert),
    };
  };

  return convert(ordered);
}
