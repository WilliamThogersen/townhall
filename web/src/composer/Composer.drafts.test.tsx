import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { Composer } from "./Composer";
import { clearAllDrafts } from "./drafts";

const input = () => screen.getByRole("textbox");

function Switcher({ id }: { readonly id: string | null }) {
  return <Composer key={id ?? "home"} conversationId={id} onSubmit={vi.fn(async () => undefined)} />;
}

describe("Composer drafts", () => {
  beforeEach(clearAllDrafts);

  it("keeps unsent text per conversation across switching", async () => {
    const user = userEvent.setup();
    const { rerender } = render(<Switcher id="a" />);
    await user.type(input(), "half a thought");
    rerender(<Switcher id="b" />);
    expect(input()).toHaveValue("");
    await user.type(input(), "other");
    rerender(<Switcher id={null} />);
    expect(input()).toHaveValue("");
    rerender(<Switcher id="a" />);
    expect(input()).toHaveValue("half a thought");
    rerender(<Switcher id="b" />);
    expect(input()).toHaveValue("other");
  });

  it("drops the draft once the message was sent", async () => {
    const user = userEvent.setup();
    const { rerender } = render(<Switcher id="a" />);
    await user.type(input(), "ship it{Enter}");
    await act(async () => undefined);
    rerender(<Switcher id="b" />);
    rerender(<Switcher id="a" />);
    expect(input()).toHaveValue("");
  });
});
