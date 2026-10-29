/**
 * Browser-only regression, run through the documented CUA tab API against the real Web WASM.
 * Start on a loaded Home catalogue containing the given work. This intentionally exercises
 * the dock button, whose navigation disposes its ancestor during the same click dispatch.
 * No browser driver, injected DOM event, mock navigation or test-only application path.
 */
export async function searchDockResultNavigation(tab, { title, resultLabel, workId }) {
  const startedAt = new Date().toISOString();
  await tab.getAXState({ emit: false });
  await tab.playwright.locator("#theatre-discovery-search").click();
  await tab.getAXState({ emit: false });
  await tab.playwright.getByRole("button", { name: resultLabel, exact: true }).click();
  await tab.getAXState({ emit: false });
  await tab.playwright.getByRole("heading", { name: title, exact: true }).waitFor({
    state: "visible", timeoutMs: 20000,
  });
  await tab.getAXState({ emit: false });
  const url = await tab.url();
  const errors = (await tab.dev.logs({ levels: ["error"], limit: 200 }))
    .filter((entry) => entry.timestamp >= startedAt);
  const report = { startedAt, title, workId, url, errors };
  if (!url.endsWith(`#work/${workId}`) || errors.length !== 0) {
    const failure = new Error("Dock navigation must reach its work without a runtime error");
    failure.report = report;
    throw failure;
  }
  return { ...report, status: "PASS" };
}
