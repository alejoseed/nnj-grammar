import { buildGraphModel, type GraphNode } from "./graph-model";
import { parseAnalysisDocument } from "./types";

export type GraphRenderer = (host: HTMLElement, model: GraphNode) => void;

export async function loadAnalysisDocument(
  url: URL,
  fetcher: typeof fetch = fetch,
) {
  const response = await fetcher(url);
  if (!response.ok) {
    throw new Error(`fixture request failed: ${response.status}`);
  }
  return parseAnalysisDocument(await response.json());
}

function showGraphError(host: HTMLElement): void {
  const error = document.createElement("p");
  error.setAttribute("role", "alert");
  error.className =
    "m-8 rounded border border-red-300 bg-red-50 p-4 text-sm text-red-900";
  error.textContent = "Unable to load grammar graph.";
  host.replaceChildren(error);
}

export async function mountFixtureGraph(
  host: HTMLElement,
  url: URL,
  render: GraphRenderer,
  fetcher: typeof fetch = fetch,
): Promise<void> {
  try {
    const document = await loadAnalysisDocument(url, fetcher);
    render(host, buildGraphModel(document));
  } catch {
    showGraphError(host);
  }
}
