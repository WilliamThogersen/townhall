import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";
import { clearAllDrafts } from "../composer/drafts";
import { clearAllQueues } from "../composer/queue";

afterEach(() => {
  cleanup();
  clearAllDrafts();
  clearAllQueues();
});
