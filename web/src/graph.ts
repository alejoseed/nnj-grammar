import * as d3 from "d3";
import type { GraphNode } from "./graph-model";

const MARGIN = { top: 20, right: 120, bottom: 20, left: 200 };
const INNER_WIDTH = 1200 - MARGIN.left - MARGIN.right;
const INNER_HEIGHT = 800 - MARGIN.top - MARGIN.bottom;

const ROOT_COLOR = "#1f77b4";
const DESCENDANT_COLOR = "#4daf4a";
const EMPHASIS_COLOR = "#ff7f0e";

type PointNode = d3.HierarchyPointNode<GraphNode>;
type NodeSelection = d3.Selection<SVGGElement, PointNode, d3.BaseType, unknown>;

function screenX(point: { x: number; y: number }): number {
  return point.y * 0.8;
}

function screenY(point: { x: number; y: number }): number {
  return point.x + 40;
}

function restingCircleColor(point: PointNode): string {
  return point.depth === 0 ? ROOT_COLOR : DESCENDANT_COLOR;
}

function ariaLabel(node: GraphNode): string {
  if (!node.primaryLabel) {
    return node.kind;
  }
  return node.secondaryLabel
    ? `${node.primaryLabel} (${node.secondaryLabel})`
    : node.primaryLabel;
}

function applyEmphasis(group: NodeSelection, active: boolean): void {
  const point = group.datum();
  const circleColor = active ? EMPHASIS_COLOR : restingCircleColor(point);

  group
    .select<SVGCircleElement>("circle")
    .attr("r", active ? 10 : 6)
    .attr("fill", circleColor);

  group.selectAll<SVGTextElement, unknown>("text").each(function () {
    this.style.fill = active ? EMPHASIS_COLOR : "";
    this.style.fontWeight = active ? "bold" : "normal";
  });
}

export function renderGraph(host: HTMLElement, model: GraphNode): void {
  host.replaceChildren();

  const layout = d3.tree<GraphNode>().size([INNER_HEIGHT, INNER_WIDTH / 1.5]);
  const root = layout(d3.hierarchy(model, (node) => node.children));

  const svg = d3
    .select(host)
    .append("svg")
    .attr("viewBox", "0 0 1200 800")
    .attr("preserveAspectRatio", "xMidYMid meet")
    .attr("role", "tree")
    .attr("aria-label", "Grammar analysis tree")
    .attr("class", "block h-full w-full bg-[#f1f5f9] font-sans text-[12px]");

  const viewport = svg.append("g").attr("data-layer", "viewport");
  const plot = viewport
    .append("g")
    .attr("data-layer", "plot")
    .attr("transform", "translate(200,20)");

  const linkPath = d3
    .linkHorizontal<d3.HierarchyPointLink<GraphNode>, PointNode>()
    .x((point) => screenX(point))
    .y((point) => screenY(point));

  plot
    .selectAll("path.graph-link")
    .data(root.links())
    .join("path")
    .attr("class", "graph-link stroke-[#94a3b8] fill-none")
    .attr("stroke-width", 1)
    .attr("stroke-opacity", 0.6)
    .attr("d", linkPath);

  const node: NodeSelection = plot
    .selectAll<SVGGElement, PointNode>("g.graph-node")
    .data(root.descendants())
    .join("g")
    .attr("class", "graph-node")
    .attr("id", (point) => `graph-node-${point.data.id}`)
    .attr("transform", (point) => `translate(${screenX(point)},${screenY(point)})`)
    .attr("role", "treeitem")
    .attr("tabindex", 0)
    .attr("aria-label", (point) => ariaLabel(point.data));

  node
    .append("circle")
    .attr("class", "stroke-[#555] transition-all duration-200")
    .attr("r", 6)
    .attr("fill", (point) => restingCircleColor(point))
    .attr("stroke-width", 1);

  node
    .filter((point) => point.data.primaryLabel !== "")
    .append("text")
    .attr(
      "class",
      "graph-primary-label fill-[#333] text-[12px] transition-all duration-200",
    )
    .attr("x", (point) => (point.children ? -10 : 10))
    .attr("dy", (point) => (point.children ? "-1.5em" : ".35em"))
    .attr("text-anchor", (point) => (point.children ? "end" : "start"))
    .text((point) => point.data.primaryLabel);

  node
    .filter((point) => point.data.secondaryLabel !== "")
    .append("text")
    .attr(
      "class",
      "graph-secondary-label fill-[#555] text-[10px] italic transition-all duration-200",
    )
    .attr("x", (point) => (point.children ? -10 : 10))
    .attr("dy", (point) => (point.children ? "-.5em" : "1.5em"))
    .attr("text-anchor", (point) => (point.children ? "end" : "start"))
    .text((point) => `(${point.data.secondaryLabel})`);

  const emphasisState = new WeakMap<
    SVGGElement,
    { hovered: boolean; focused: boolean }
  >();
  const updateEmphasis = (
    element: SVGGElement,
    kind: "hovered" | "focused",
    active: boolean,
  ): void => {
    const state = emphasisState.get(element) ?? {
      hovered: false,
      focused: false,
    };
    state[kind] = active;
    emphasisState.set(element, state);
    applyEmphasis(
      d3.select<SVGGElement, PointNode>(element),
      state.hovered || state.focused,
    );
  };

  node
    .on("mouseenter", function () {
      updateEmphasis(this, "hovered", true);
    })
    .on("mouseleave", function () {
      updateEmphasis(this, "hovered", false);
    })
    .on("focus", function () {
      updateEmphasis(this, "focused", true);
    })
    .on("blur", function () {
      updateEmphasis(this, "focused", false);
    });

  const zoom = d3
    .zoom<SVGSVGElement, unknown>()
    .scaleExtent([0.5, 2])
    .on("zoom", (event) => {
      viewport.attr("transform", event.transform.toString());
    });

  svg.call(zoom);
}
