import { describe, expect, it } from "vitest";
import fixture from "../../tests/fixtures/analysis-soshite.json";
import type { AnalysisDocument } from "./types";
import { buildGraphModel, buildOrderedTree } from "./graph-model";

// Fixture tree: document-0 → sentence-0 → [bunsetsu-0-0 そして][bunsetsu-0-1 なによりも]
// Node indices: 0=document-0 1=sentence-0 2=bunsetsu-0-0 3=token-0
//               4=bunsetsu-0-1 5=token-1 6=token-2 7=token-3
function documentCopy(): AnalysisDocument {
  return structuredClone(fixture) as AnalysisDocument;
}

describe("buildOrderedTree", () => {
  it("preserves edge order recursively", () => {
    const root = buildOrderedTree(documentCopy().tree);
    expect(root.node.id).toBe("document-0");
    expect(root.children.map((child) => child.node.id)).toEqual(["sentence-0"]);
    expect(
      root.children[0]?.children.map((child) => child.node.id),
    ).toEqual(["bunsetsu-0-0", "bunsetsu-0-1"]);
    expect(
      root.children[0]?.children[1]?.children.map((child) => child.node.id),
    ).toEqual(["token-1", "token-2", "token-3"]);
  });

  it("rejects duplicate node IDs", () => {
    const document = documentCopy();
    document.tree.nodes.push(structuredClone(document.tree.nodes[0]!));
    expect(() => buildOrderedTree(document.tree)).toThrow(
      "duplicate tree node: document-0",
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
      parent_id: "bunsetsu-0-0",
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
      child_id: "document-0",
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

describe("buildGraphModel", () => {
  it("derives faithful labels from the committed document", () => {
    const root = buildGraphModel(documentCopy());
    expect([root.primaryLabel, root.secondaryLabel]).toEqual(["", ""]);
    // The sentence has no attached match, so it stays unlabeled rather than
    // duplicating its bunsetsu children.
    const sentence = root.children[0]!;
    expect([sentence.primaryLabel, sentence.secondaryLabel]).toEqual(["", ""]);
    expect(sentence.children[0]).toMatchObject({
      id: "bunsetsu-0-0",
      primaryLabel: "そして",
      secondaryLabel: "Used to connect two sentences; 'and then', 'and'.",
    });
    expect(sentence.children[1]).toMatchObject({
      id: "bunsetsu-0-1",
      primaryLabel: "なによりも",
      secondaryLabel: "Above all else, more than anything",
    });
    expect(
      sentence.children[1]?.children.map((node) => node.primaryLabel),
    ).toEqual(["なに", "より", "も"]);
  });

  it("uses a gloss before a non-redundant reading", () => {
    const withGloss = documentCopy();
    withGloss.tokens[1]!.surface = "何";
    withGloss.tokens[1]!.glosses = [
      { entry_seq: 1, gloss: "what", pos: ["pronoun"] },
    ];
    expect(
      buildGraphModel(withGloss).children[0]?.children[1]?.children[0]
        ?.secondaryLabel,
    ).toBe("what");

    withGloss.tokens[1]!.glosses = [];
    expect(
      buildGraphModel(withGloss).children[0]?.children[1]?.children[0]
        ?.secondaryLabel,
    ).toBe("なに");
  });

  it("does not repeat a reading identical to the surface", () => {
    // Isolate the reading-vs-surface rule from gloss precedence: そして's reading
    // equals its surface, so with no gloss the secondary label stays empty.
    const doc = documentCopy();
    doc.tokens[0]!.glosses = [];
    expect(
      buildGraphModel(doc).children[0]?.children[0]?.children[0]
        ?.secondaryLabel,
    ).toBe("");
  });

  it("requires a document root", () => {
    const document = documentCopy();
    document.tree.nodes[0]!.kind = "sentence";
    expect(() => buildGraphModel(document)).toThrow(
      "tree root must be a document",
    );
  });

  it("shows a bunsetsu surface without inventing a translation", () => {
    const document = documentCopy();
    document.tree.nodes[2]!.match_ids = [];
    expect(buildGraphModel(document).children[0]?.children[0]).toMatchObject({
      primaryLabel: "そして",
      secondaryLabel: "",
    });
  });

  it("rejects missing references and invalid spans", () => {
    const missingToken = documentCopy();
    missingToken.tree.nodes[3]!.token_id = "missing";
    expect(() => buildGraphModel(missingToken)).toThrow(
      "missing analyzed token: missing",
    );

    const missingMatch = documentCopy();
    missingMatch.tree.nodes[2]!.match_ids = ["missing"];
    expect(() => buildGraphModel(missingMatch)).toThrow(
      "missing primary match: missing",
    );

    const missingSecondary = documentCopy();
    missingSecondary.tree.nodes[4]!.secondary_match_ids = ["missing"];
    expect(() => buildGraphModel(missingSecondary)).toThrow(
      "missing secondary match: missing",
    );

    const invalidSpan = documentCopy();
    invalidSpan.tree.nodes[2]!.token_end = 99;
    expect(() => buildGraphModel(invalidSpan)).toThrow(
      "invalid token span: 0..99",
    );

    const escapingMatch = documentCopy();
    escapingMatch.primary_matches[0]!.token_end = 2;
    expect(() => buildGraphModel(escapingMatch)).toThrow(
      "match span escapes its tree node: match-0-0",
    );
  });
});
