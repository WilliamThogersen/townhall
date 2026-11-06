import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { Composer } from "./Composer";
import { clearAllReplies, readReply, writeReply } from "./replies";

const asked = { id: 4, author: "townhall", excerpt: "which repo should it go in?" } as const;
const input = () => screen.getByRole("textbox");

describe("Composer quote bar", () => {
  beforeEach(clearAllReplies);

  it("shows nothing when no entry is being answered", () => {
    render(<Composer conversationId="c1" onSubmit={vi.fn()} />);
    expect(screen.queryByRole("group", { name: "Replying to" })).not.toBeInTheDocument();
  });

  it("shows the author and excerpt of the entry being answered, for this conversation only", () => {
    writeReply("c1", asked);
    writeReply("c2", { id: 9, author: "you", excerpt: "other" });
    render(<Composer conversationId="c1" onSubmit={vi.fn()} />);
    const bar = screen.getByRole("group", { name: "Replying to" });
    expect(bar).toHaveTextContent("townhall");
    expect(bar).toHaveTextContent("which repo should it go in?");
    expect(bar).not.toHaveTextContent("other");
  });

  it("appears when a reply is set while mounted", () => {
    render(<Composer conversationId="c1" onSubmit={vi.fn()} />);
    act(() => writeReply("c1", asked));
    expect(screen.getByRole("group", { name: "Replying to" })).toBeInTheDocument();
  });

  it("cancels with the X and keeps what was typed", async () => {
    const user = userEvent.setup();
    writeReply("c1", asked);
    render(<Composer conversationId="c1" onSubmit={vi.fn()} />);
    await user.type(input(), "draft");
    await user.click(screen.getByRole("button", { name: "Cancel reply" }));
    expect(readReply("c1")).toBeNull();
    expect(screen.queryByRole("group", { name: "Replying to" })).not.toBeInTheDocument();
    expect(input()).toHaveValue("draft");
  });

  it("cancels on Escape first, and only a second Escape leaves the input", async () => {
    const user = userEvent.setup();
    writeReply("c1", asked);
    render(<Composer conversationId="c1" onSubmit={vi.fn()} />);
    await user.click(input());
    await user.keyboard("{Escape}");
    expect(readReply("c1")).toBeNull();
    expect(input()).toHaveFocus();
    await user.keyboard("{Escape}");
    expect(input()).not.toHaveFocus();
  });

  it("is cleared once the message went out, not before", async () => {
    const user = userEvent.setup();
    let finish: () => void = () => undefined;
    const onSubmit = vi.fn(() => new Promise<void>((done) => (finish = done)));
    writeReply("c1", asked);
    render(<Composer conversationId="c1" onSubmit={onSubmit} />);
    await user.type(input(), "shop{Enter}");
    expect(onSubmit).toHaveBeenCalledOnce();
    expect(readReply("c1")).toEqual(asked);
    await act(async () => finish());
    expect(readReply("c1")).toBeNull();
  });

  it("stays when sending failed", async () => {
    const user = userEvent.setup();
    writeReply("c1", asked);
    render(<Composer conversationId="c1" onSubmit={vi.fn().mockRejectedValue(new Error("down"))} />);
    await user.type(input(), "shop{Enter}");
    expect(readReply("c1")).toEqual(asked);
  });
});
