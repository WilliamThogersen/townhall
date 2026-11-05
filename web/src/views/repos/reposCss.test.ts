import { describe, expect, it } from "vitest";
import { declarationsFor } from "../../test/cssRules";
import reposCss from "./repos.css?raw";

describe("repos css", () => {
  it("lets the stacked form's error line take the whole form width", () => {
    const error = declarationsFor(reposCss, '.th-repo-add[data-layout="stack"] .th-repo-add-error');
    expect(error?.getPropertyValue("max-width")).toBe("none");
  });
});
