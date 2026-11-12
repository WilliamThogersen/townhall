import "@testing-library/jest-dom/vitest";
import { cleanup, configure } from "@testing-library/react";
import { afterEach } from "vitest";
import { clearAllDrafts } from "../composer/drafts";
import { clearAllQueues } from "../composer/queue";

configure({ asyncUtilTimeout: 10_000 });

afterEach(() => {
  cleanup();
  clearAllDrafts();
  clearAllQueues();
});
