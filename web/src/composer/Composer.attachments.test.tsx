import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MAX_ATTACHMENT_BYTES } from "../api/attachments";
import { stubObjectUrls, type FakeObjectUrls } from "../attachments/testObjectUrls";
import { attachmentFor, installFakeXhr, lastXhr, xhrs } from "../attachments/testXhr";
import { Composer, type ComposerHandle } from "./Composer";

let urls: FakeObjectUrls;

beforeEach(() => {
  installFakeXhr();
  urls = stubObjectUrls();
});

afterEach(() => {
  urls.restore();
  vi.unstubAllGlobals();
});

function png(name = "shot.png", size = 10): File {
  const file = new File(["x"], name, { type: "image/png" });
  Object.defineProperty(file, "size", { value: size });
  return file;
}

const input = () => screen.getByRole("textbox");
const send = () => screen.getByRole("button", { name: "send" });
const fileInput = (container: HTMLElement) => container.querySelector('input[type="file"]') as HTMLInputElement;

describe("Composer thread", () => {
  it("matches the conversation composer markup", () => {
    const { container } = render(<Composer conversationId="c1" onSubmit={vi.fn()} />);
    expect(container.querySelector("form")).toHaveAttribute("class", "th-composer");
    expect(input()).toHaveAttribute("rows", "2");
    expect(input()).toHaveAttribute("placeholder", "reply, ask for more work, or drop files here");
    expect(input()).toHaveAccessibleName("Message");
    const attach = screen.getByRole("button", { name: "Attach files or images" });
    expect(attach).toHaveTextContent("+ attach");
    expect(attach).toHaveClass("th-btn th-btn--ghost th-btn--sm");
    expect(container.querySelector(".th-help")).toHaveTextContent("⏎ send · ⇧⏎ new line · paste or drop images");
    expect(send()).toHaveAttribute("class", "th-btn th-btn--sm");
    expect(container.querySelector(".th-composer-attachments")).toBeNull();
    expect(fileInput(container)).toHaveAttribute("multiple");
    expect(fileInput(container).accept).toContain("image/png");
  });

  it("leaves attachments out without a conversation", () => {
    const { container } = render(<Composer onSubmit={vi.fn()} />);
    expect(screen.queryByRole("button", { name: "Attach files or images" })).not.toBeInTheDocument();
    expect(fileInput(container)).toBeNull();
    expect(input()).toHaveAttribute("placeholder", "reply, or ask for more work");
    expect(container.querySelector(".th-help")).toHaveTextContent("⏎ send · ⇧⏎ new line");
    expect(container.querySelector(".th-help")).not.toHaveTextContent("paste");
  });

  it("opens the file picker from + attach", async () => {
    const user = userEvent.setup();
    const { container } = render(<Composer conversationId="c1" onSubmit={vi.fn()} />);
    const click = vi.spyOn(fileInput(container), "click");
    await user.click(screen.getByRole("button", { name: "Attach files or images" }));
    expect(click).toHaveBeenCalledOnce();
  });

  it("uploads picked files, holds send until they finish and sends their ids", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const { container } = render(<Composer conversationId="c1" onSubmit={onSubmit} />);
    await user.upload(fileInput(container), [png()]);
    const box = container.querySelector(".th-composer-box") as HTMLElement;
    expect(box.firstElementChild).toHaveClass("th-composer-attachments");
    expect(lastXhr().url).toBe("/api/conversations/c1/attachments");
    expect(fileInput(container).value).toBe("");
    await user.type(input(), "here is the screenshot");
    expect(send()).toBeDisabled();
    await user.keyboard("{Enter}");
    expect(onSubmit).not.toHaveBeenCalled();
    await act(async () => lastXhr().succeed(attachmentFor("a1", "shot.png")));
    expect(send()).toBeEnabled();
    await user.click(send());
    expect(onSubmit).toHaveBeenCalledWith("here is the screenshot", ["a1"], []);
    expect(container.querySelector(".th-composer-attachments")).toBeNull();
  });

  it("sends finished attachments without any text", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const { container } = render(<Composer conversationId="c1" onSubmit={onSubmit} />);
    await user.upload(fileInput(container), [png()]);
    await act(async () => lastXhr().succeed(attachmentFor("a1", "shot.png")));
    await user.click(input());
    await user.keyboard("{Enter}");
    expect(onSubmit).toHaveBeenCalledWith("", ["a1"], []);
  });

  it("keeps the attachments when sending fails", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockRejectedValue(new Error("a turn is running"));
    const { container } = render(<Composer conversationId="c1" onSubmit={onSubmit} />);
    await user.upload(fileInput(container), [png()]);
    await act(async () => lastXhr().succeed(attachmentFor("a1", "shot.png")));
    await user.type(input(), "look{Enter}");
    expect(container.querySelector(".th-composer-attachments")).not.toBeNull();
    expect(container.querySelector(".th-help")).toHaveTextContent("error: a turn is running");
  });

  it("uploads an image pasted into the input", () => {
    const { container } = render(<Composer conversationId="c1" onSubmit={vi.fn()} />);
    const pasted = fireEvent.paste(input(), {
      clipboardData: { files: [png("image.png")], items: [], types: ["Files"] },
    });
    expect(pasted).toBe(false);
    expect(xhrs()).toHaveLength(1);
    expect(container.querySelector(".th-composer-attachments img")).toHaveAttribute("src", "blob:image.png");
  });

  it("lets pasted text through untouched", () => {
    render(<Composer conversationId="c1" onSubmit={vi.fn()} />);
    const pasted = fireEvent.paste(input(), { clipboardData: { files: [], items: [], types: ["text/plain"] } });
    expect(pasted).toBe(true);
    expect(xhrs()).toHaveLength(0);
  });

  it("ignores pasted files without a conversation", () => {
    render(<Composer onSubmit={vi.fn()} />);
    fireEvent.paste(input(), { clipboardData: { files: [png()], items: [], types: ["Files"] } });
    expect(xhrs()).toHaveLength(0);
  });

  it("says in red why a file was refused and keeps send for the rest", async () => {
    const user = userEvent.setup();
    const { container } = render(<Composer conversationId="c1" onSubmit={vi.fn()} />);
    await user.upload(fileInput(container), [png("screenshot-3.png", MAX_ATTACHMENT_BYTES + 1)]);
    const help = container.querySelector(".th-help");
    expect(help).toHaveTextContent("error: screenshot-3.png is over 20 MB");
    expect(help).toHaveAttribute("data-tone", "error");
    await user.type(input(), "the rest");
    expect(send()).toBeEnabled();
    expect(screen.queryByRole("button", { name: "retry" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /^Remove screenshot-3\.png/ }));
    expect(container.querySelector(".th-composer-attachments")).toBeNull();
  });

  it("retries uploads that stopped on the way", async () => {
    const user = userEvent.setup();
    const { container } = render(<Composer conversationId="c1" onSubmit={vi.fn()} />);
    await user.upload(fileInput(container), [png()]);
    await act(async () => lastXhr().fail());
    await user.click(screen.getByRole("button", { name: "retry" }));
    expect(xhrs()).toHaveLength(2);
    expect(screen.queryByRole("button", { name: "retry" })).not.toBeInTheDocument();
  });

  it("takes dropped files through its ref", () => {
    const ref = createRef<ComposerHandle>();
    render(<Composer conversationId="c1" onSubmit={vi.fn()} ref={ref} />);
    act(() => ref.current?.addFiles([png()]));
    expect(xhrs()).toHaveLength(1);
  });
});
