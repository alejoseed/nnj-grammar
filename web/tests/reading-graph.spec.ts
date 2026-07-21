import { expect, test } from "@playwright/test";

test("renders and navigates the Hanabira-faithful graph", async ({ page }) => {
  await page.goto("/");
  const svg = page.locator("svg[role=tree]");
  await expect(svg).toBeVisible();
  await expect(svg).toHaveAttribute("viewBox", "0 0 1200 800");
  await expect(page.locator(".graph-node")).toHaveCount(7);
  await expect(page.locator(".graph-link")).toHaveCount(6);

  const grammarNode = page.locator("#graph-node-match-1-3");
  const primaryLabel = grammarNode.locator(".graph-primary-label");
  await grammarNode.hover();
  await expect(grammarNode.locator("circle")).toHaveAttribute("fill", "#ff7f0e");
  await expect(grammarNode.locator("circle")).toHaveAttribute("r", "10");
  await expect(primaryLabel).toHaveCSS("fill", "rgb(255, 127, 14)");

  await page.mouse.move(0, 0);
  await expect(grammarNode.locator("circle")).toHaveAttribute("fill", "#4daf4a");
  await grammarNode.focus();
  await expect(grammarNode.locator("circle")).toHaveAttribute("fill", "#ff7f0e");
  await expect(primaryLabel).toHaveCSS("fill", "rgb(255, 127, 14)");

  const viewport = page.locator("[data-layer=viewport]");
  const plot = page.locator("[data-layer=plot]");
  await expect(plot).toHaveAttribute("transform", "translate(200,20)");
  const initialTransform = await viewport.getAttribute("transform");

  await svg.hover({ position: { x: 600, y: 400 } });
  await page.mouse.wheel(0, -500);
  await expect.poll(() => viewport.getAttribute("transform")).not.toBe(
    initialTransform,
  );
  await expect(plot).toHaveAttribute("transform", "translate(200,20)");

  const afterZoom = await viewport.getAttribute("transform");
  const box = await svg.boundingBox();
  if (!box) {
    throw new Error("graph SVG has no bounding box");
  }
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(
    box.x + box.width / 2 + 80,
    box.y + box.height / 2 + 40,
  );
  await page.mouse.up();
  await expect.poll(() => viewport.getAttribute("transform")).not.toBe(
    afterZoom,
  );
  await expect(plot).toHaveAttribute("transform", "translate(200,20)");

  await page.reload();
  await expect(page.locator("svg[role=tree]")).toBeVisible();
  await page.screenshot({
    path: "test-results/hanabira-fixture.png",
    fullPage: true,
  });
});
