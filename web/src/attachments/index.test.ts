import { describe, expect, it } from "vitest";
import * as attachments from "./index";
import * as composer from "../composer/index";

describe("public surface", () => {
  it("exports what the home and conversation screens mount", () => {
    expect(Object.keys(attachments).sort()).toEqual([
      "DropZone",
      "Lightbox",
      "Thumbs",
      "UploadChips",
      "formatSize",
      "lightboxImages",
      "stripAttachmentMarkers",
      "useFileDrop",
      "useUploads",
    ]);
    expect(Object.keys(composer)).toEqual(["Composer"]);
  });
});
