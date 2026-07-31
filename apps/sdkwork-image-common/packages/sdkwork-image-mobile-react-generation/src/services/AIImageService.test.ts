import assert from "node:assert/strict";
import test from "node:test";

import {
  AIImageCapabilityUnavailableError,
  AIImageService,
} from "./AIImageService";

test("AI image operations fail closed without an owner SDK", async () => {
  let progressCalled = false;

  await assert.rejects(
    AIImageService.generateImage(
      { aspectRatio: "1:1", prompt: "test", style: "none" },
      () => {
        progressCalled = true;
      },
    ),
    AIImageCapabilityUnavailableError,
  );
  await assert.rejects(
    AIImageService.optimizePrompt("test"),
    AIImageCapabilityUnavailableError,
  );
  await assert.rejects(
    AIImageService.getHistory(),
    AIImageCapabilityUnavailableError,
  );
  assert.throws(
    () => AIImageService.deleteFromHistory("task-id"),
    AIImageCapabilityUnavailableError,
  );
  assert.equal(progressCalled, false);
});
