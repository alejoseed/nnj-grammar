import "./styles.css";
import { analyzeText } from "./app";
import { buildGraphModel } from "./graph-model";
import { renderGraph } from "./graph";

const root = document.querySelector("#app");
if (!(root instanceof HTMLElement)) {
  throw new Error("missing app host");
}

root.replaceChildren();
root.className = "flex h-screen w-screen flex-col";

// Toolbar: paste a sentence and analyze it.
const bar = document.createElement("form");
bar.className = "flex gap-2 border-b border-slate-300 bg-white p-3";

const input = document.createElement("input");
input.type = "text";
input.placeholder = "文を貼り付け… (e.g. 東京しか行かない)";
input.className =
  "flex-1 rounded border border-slate-300 px-3 py-2 text-lg";

const button = document.createElement("button");
button.type = "submit";
button.textContent = "Analyze";
button.className =
  "rounded bg-blue-600 px-4 py-2 font-medium text-white hover:bg-blue-700";

const status = document.createElement("span");
status.className = "self-center text-sm text-slate-500";

bar.append(input, button, status);

// Graph area fills the rest of the screen.
const graphHost = document.createElement("div");
graphHost.className = "min-h-0 flex-1";

root.append(bar, graphHost);

async function analyze(text: string): Promise<void> {
  const trimmed = text.trim();
  if (!trimmed) {
    return;
  }
  status.textContent = "Analyzing…";
  try {
    const doc = await analyzeText(trimmed);
    renderGraph(graphHost, buildGraphModel(doc));
    status.textContent = `${doc.tokens.length} tokens · ${doc.primary_matches.length} matches`;
  } catch {
    // Preserve the current graph on failure — just report it.
    status.textContent = "Analysis failed — is the backend running?";
  }
}

bar.addEventListener("submit", (event) => {
  event.preventDefault();
  void analyze(input.value);
});

// Seed with the classic example so the page isn't empty on load.
input.value = "東京しか行かない";
void analyze(input.value);
