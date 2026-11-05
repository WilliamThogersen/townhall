import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";
import { clearAllDrafts } from "../composer/drafts";

afterEach(() => {
  cleanup();
  clearAllDrafts();
});
