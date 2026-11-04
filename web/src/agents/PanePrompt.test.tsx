import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as client from "../api/client";
import { ApiError } from "../api/errors";
import { PanePrompt } from "./PanePrompt";

afterEach(() => vi.restoreAllMocks());

const input = () => screen.getByRole("textbox", { name: "Input for u-demo" });
const sendButton = () => screen.getByRole("button", { name: /send/ });

describe("PanePrompt", () => {
  it("renders the prompt sign, input and a disabled send while empty", () => {
    const { container } = render(<PanePrompt name="u-demo" />);
    expect(container.querySelector("form.th-prompt")).not.toBeNull();
    expect(container.querySelector(".th-prompt-sign")).toHaveTextContent("›");
    expect(input()).toHaveAttribute("placeholder", "type into u-demo");
    expect(input()).toHaveClass("th-prompt-input");
    expect(sendButton()).toBeDisabled();
    expect(sendButton().querySelector(".th-kbd")).toHaveTextContent("⏎");
  });

  it("keeps send disabled for whitespace only and ignores enter", async () => {
    const send = vi.spyOn(client, "sendText").mockResolvedValue("ok");
    render(<PanePrompt name="u-demo" />);
    await userEvent.type(input(), "   {Enter}");
    expect(sendButton()).toBeDisabled();
    expect(send).not.toHaveBeenCalled();
  });

  it("sends the text with enter and clears the input", async () => {
    const send = vi.spyOn(client, "sendText").mockResolvedValue("ok");
    render(<PanePrompt name="u-demo" />);
    await userEvent.type(input(), "ls -la{Enter}");
    expect(send).toHaveBeenCalledWith("u-demo", "ls -la");
    await waitFor(() => expect(input()).toHaveValue(""));
    expect(input()).toHaveFocus();
  });

  it("shows sending and blocks a second send while it waits", async () => {
    let release: (value: string) => void = () => {};
    const send = vi
      .spyOn(client, "sendText")
      .mockImplementation(() => new Promise((resolve) => (release = resolve)));
    render(<PanePrompt name="u-demo" />);
    await userEvent.type(input(), "y");
    await userEvent.click(sendButton());
    expect(sendButton()).toBeDisabled();
    expect(sendButton()).toHaveTextContent("sending…");
    await userEvent.type(input(), "{Enter}");
    expect(send).toHaveBeenCalledTimes(1);
    release("ok");
    await waitFor(() => expect(sendButton()).toHaveTextContent("send"));
  });

  it("shows the error and keeps the text when sending fails", async () => {
    vi.spyOn(client, "sendText").mockRejectedValue(new ApiError("http", "agent `u-demo` not found", 404));
    render(<PanePrompt name="u-demo" />);
    await userEvent.type(input(), "echo hi{Enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent("error: agent `u-demo` not found");
    expect(input()).toHaveValue("echo hi");
  });

  it("clears an old error on the next successful send", async () => {
    vi.spyOn(client, "sendText").mockRejectedValueOnce(new Error("down")).mockResolvedValue("ok");
    render(<PanePrompt name="u-demo" />);
    await userEvent.type(input(), "a{Enter}");
    await screen.findByRole("alert");
    await userEvent.type(input(), "{Enter}");
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  });
});
