import { timerFlush } from "d3";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import fixture from "../../tests/fixtures/analysis-soshite.json";
import { buildGraphModel } from "./graph-model";
import { renderGraph } from "./graph";
import type { AnalysisDocument } from "./types";

describe("renderGraph", () => {
  let host: HTMLDivElement;

  beforeEach(() => {
    vi.useFakeTimers();
    host = document.createElement("div");
    renderGraph(
      host,
      buildGraphModel(structuredClone(fixture) as AnalysisDocument),
    );
  });

  afterEach(() => vi.useRealTimers());

  it("renders the faithful Hanabira structure", () => {
    const svg = host.querySelector("svg");
    expect(svg?.getAttribute("viewBox")).toBe("0 0 1200 800");
    expect(svg?.getAttribute("preserveAspectRatio")).toBe("xMidYMid meet");
    expect(svg?.getAttribute("role")).toBe("tree");
    expect(svg?.getAttribute("aria-label")).toBe("Grammar analysis tree");
    expect(svg?.classList.contains("bg-[#f1f5f9]")).toBe(true);
    expect(host.querySelectorAll(".graph-node")).toHaveLength(7);
    expect(host.querySelectorAll(".graph-link")).toHaveLength(6);
    expect(
      host.querySelector("[data-layer=plot]")?.getAttribute("transform"),
    ).toBe("translate(200,20)");
    // The lone sentence scaffold is hoisted away entirely.
    expect(host.querySelector("#graph-node-sentence-0")).toBeNull();
    expect(host.textContent).toContain("なによりも");
    expect(host.textContent).toContain("(Above all else, more than anything)");

    const internalLabel = host.querySelector(
      "#graph-node-bunsetsu-0-1 .graph-primary-label",
    );
    expect(internalLabel?.getAttribute("x")).toBe("-10");
    expect(internalLabel?.getAttribute("dy")).toBe("-1.5em");
    expect(internalLabel?.getAttribute("text-anchor")).toBe("end");
    const leafLabel = host.querySelector(
      "#graph-node-token-1 .graph-primary-label",
    );
    expect(leafLabel?.getAttribute("x")).toBe("10");
    expect(leafLabel?.getAttribute("dy")).toBe(".35em");
    expect(leafLabel?.getAttribute("text-anchor")).toBe("start");
  });

  it("uses stable accessible node identities", () => {
    const node = host.querySelector("#graph-node-bunsetsu-0-1");
    expect(node?.getAttribute("role")).toBe("treeitem");
    expect(node?.getAttribute("tabindex")).toBe("0");
    expect(node?.getAttribute("aria-label")).toContain("なによりも");
    expect(node?.querySelector("circle")?.getAttribute("fill")).toBe("#4daf4a");
    expect(
      host
        .querySelector("#graph-node-document-0 circle")
        ?.getAttribute("fill"),
    ).toBe("#1f77b4");
  });

  it.each(["mouseenter", "focus"])(
    "applies orange emphasis on %s",
    (eventName) => {
      const node = host.querySelector<SVGGElement>("#graph-node-bunsetsu-0-1")!;
      node.dispatchEvent(new Event(eventName));
      vi.advanceTimersByTime(200);
      timerFlush();
      expect(node.querySelector("circle")?.getAttribute("r")).toBe("10");
      expect(node.querySelector("circle")?.getAttribute("fill")).toBe(
        "#ff7f0e",
      );
      expect(node.querySelector<SVGTextElement>("text")?.style.fontWeight).toBe(
        "bold",
      );
      expect(node.querySelector<SVGTextElement>("text")?.style.fill).toBe(
        "rgb(255, 127, 14)",
      );

      node.dispatchEvent(
        new Event(eventName === "mouseenter" ? "mouseleave" : "blur"),
      );
      expect(node.querySelector("circle")?.getAttribute("r")).toBe("6");
      expect(node.querySelector("circle")?.getAttribute("fill")).toBe(
        "#4daf4a",
      );
      expect(node.querySelector<SVGTextElement>("text")?.style.fill).toBe("");
      expect(node.querySelector<SVGTextElement>("text")?.style.fontWeight).toBe(
        "normal",
      );
    },
  );

  it("keeps emphasis while either hover or focus remains active", () => {
    const node = host.querySelector<SVGGElement>("#graph-node-bunsetsu-0-1")!;
    node.dispatchEvent(new Event("focus"));
    node.dispatchEvent(new Event("mouseenter"));
    node.dispatchEvent(new Event("mouseleave"));
    expect(node.querySelector("circle")?.getAttribute("r")).toBe("10");

    node.dispatchEvent(new Event("blur"));
    expect(node.querySelector("circle")?.getAttribute("r")).toBe("6");
  });
});
