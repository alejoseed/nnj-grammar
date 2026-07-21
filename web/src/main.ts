import "./styles.css";
import { mountFixtureGraph } from "./app";
import { renderGraph } from "./graph";

const host = document.querySelector("#app");
if (!(host instanceof HTMLElement)) {
  throw new Error("missing graph host");
}

const fixtureUrl = new URL(
  "../../tests/fixtures/analysis-soshite.json",
  import.meta.url,
);
void mountFixtureGraph(host, fixtureUrl, renderGraph);
