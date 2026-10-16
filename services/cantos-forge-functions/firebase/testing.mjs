// Explicit isolated-test composition. Never import this file from a deployed entrypoint.
import { makeHttpHandler } from "./http.mjs";
import { makeRunner } from "./worker.mjs";

export const createMockRunnerForTests = makeRunner;
export function createMockAuthorizedHandlerForTests(runWorker) {
  return makeHttpHandler({ authorize: async () => ({ kind: "authorized" }), runWorker });
}
