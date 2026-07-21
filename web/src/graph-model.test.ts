import { describe, expect, it } from "vitest";
import fixture from "../../tests/fixtures/analysis-soshite.json";
import type { AnalysisDocument } from "./types";
import { buildOrderedTree } from "./graph-model";

function documentCopy(): AnalysisDocument {
  return structuredClone(fixture) as AnalysisDocument;
}

describe("buildOrderedTree", () => {
  it("preserves edge order recursively", () => {
    const root = buildOrderedTree(documentCopy().tree);
    expect(root.node.id).toBe("sentence-0");
    expect(root.children.map((child) => child.node.id)).toEqual([
      "match-0-0",
      "match-1-3",
    ]);
    expect(root.children[1]?.children.map((child) => child.node.id)).toEqual([
      "token-1",
      "token-2",
      "token-3",
    ]);
  });

  it("rejects duplicate node IDs", () => {
    const document = documentCopy();
    document.tree.nodes.push(structuredClone(document.tree.nodes[0]!));
    expect(() => buildOrderedTree(document.tree)).toThrow(
      "duplicate tree node: sentence-0",
    );
  });

  it("rejects missing roots and edge references", () => {
    const missingRoot = documentCopy();
    missingRoot.tree.root_id = "missing";
    expect(() => buildOrderedTree(missingRoot.tree)).toThrow(
      "missing tree root: missing",
    );

    const missingChild = documentCopy();
    missingChild.tree.edges[0]!.child_id = "missing";
    expect(() => buildOrderedTree(missingChild.tree)).toThrow(
      "missing tree child: missing",
    );
  });

  it("rejects multiple parents", () => {
    const document = documentCopy();
    document.tree.edges.push({
      parent_id: "match-0-0",
      child_id: "token-1",
      order: 1,
    });
    expect(() => buildOrderedTree(document.tree)).toThrow(
      "multiple parents for tree node: token-1",
    );
  });

  it("rejects cycles and disconnected nodes", () => {
    const cycle = documentCopy();
    cycle.tree.edges.push({
      parent_id: "token-0",
      child_id: "sentence-0",
      order: 0,
    });
    expect(() => buildOrderedTree(cycle.tree)).toThrow("tree contains a cycle");

    const disconnected = documentCopy();
    disconnected.tree.edges = disconnected.tree.edges.filter(
      (edge) => edge.child_id !== "token-3",
    );
    expect(() => buildOrderedTree(disconnected.tree)).toThrow(
      "disconnected tree node: token-3",
    );
  });
});
