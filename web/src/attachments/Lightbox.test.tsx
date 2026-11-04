import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as actions from "./imageActions";
import { Lightbox, type LightboxImage } from "./Lightbox";

const images: readonly LightboxImage[] = [
  { src: "/img/redirect-loop.png", name: "redirect-loop.png", meta: "182 KB · from you · 10:42" },
  { src: "/img/network.png", name: "network.png", meta: "96 KB · from you · 10:42", alt: "Network panel" },
  { src: "/img/login-fixed.png", name: "login-fixed.png" },
];

function Harness({ start = 0 }: { start?: number }) {
  const [open, setOpen] = useState<HTMLElement | null>(null);
  return (
    <>
      <button type="button" onClick={(event) => setOpen(event.currentTarget)}>
        open thumbnail
      </button>
      <button type="button">elsewhere</button>
      {open && <Lightbox images={images} startIndex={start} returnFocusTo={open} onClose={() => setOpen(null)} />}
    </>
  );
}

async function openHarness(start = 0) {
  const user = userEvent.setup();
  render(<Harness start={start} />);
  await user.click(screen.getByRole("button", { name: "open thumbnail" }));
  return user;
}

const dialog = () => screen.getByRole("dialog", { name: "Image viewer" });
const stageImage = () => dialog().querySelector(".th-lightbox-stage img") as HTMLImageElement;

afterEach(() => vi.restoreAllMocks());

describe("Lightbox", () => {
  it("opens as a labelled modal dialog on the clicked image with focus on close", async () => {
    await openHarness(1);
    const box = dialog();
    expect(box.tagName).toBe("DIALOG");
    expect(box).toHaveClass("th-lightbox");
    expect(box).toHaveAttribute("open");
    expect(box.querySelector(".th-lightbox-name")).toHaveTextContent("network.png");
    expect(box.querySelector(".th-lightbox-meta")).toHaveTextContent("96 KB · from you · 10:42");
    expect(screen.getByText("2 / 3")).toBeInTheDocument();
    expect(stageImage()).toHaveAttribute("src", "/img/network.png");
    expect(stageImage()).toHaveAttribute("alt", "Network panel");
    expect(screen.getByRole("button", { name: "close esc" })).toHaveFocus();
  });

  it("uses showModal where the browser has it", () => {
    const showModal = vi.fn(function (this: HTMLDialogElement) {
      this.setAttribute("open", "");
    });
    const close = vi.fn();
    Object.defineProperty(HTMLDialogElement.prototype, "showModal", { value: showModal, configurable: true });
    Object.defineProperty(HTMLDialogElement.prototype, "close", { value: close, configurable: true });
    const { unmount } = render(<Lightbox images={images} onClose={vi.fn()} />);
    expect(showModal).toHaveBeenCalledOnce();
    unmount();
    expect(close).toHaveBeenCalledOnce();
    Reflect.deleteProperty(HTMLDialogElement.prototype, "showModal");
    Reflect.deleteProperty(HTMLDialogElement.prototype, "close");
  });

  it("steps with the arrow keys and stops at the ends", async () => {
    const user = await openHarness();
    expect(screen.getByRole("button", { name: "Previous image" })).toBeDisabled();
    await user.keyboard("{ArrowLeft}");
    expect(screen.getByText("1 / 3")).toBeInTheDocument();
    await user.keyboard("{ArrowRight}{ArrowRight}");
    expect(screen.getByText("3 / 3")).toBeInTheDocument();
    expect(stageImage()).toHaveAttribute("alt", "login-fixed.png");
    expect(screen.getByRole("button", { name: "Next image" })).toBeDisabled();
    await user.keyboard("{ArrowRight}");
    expect(screen.getByText("3 / 3")).toBeInTheDocument();
  });

  it("steps with the side buttons and the strip", async () => {
    const user = await openHarness();
    await user.click(screen.getByRole("button", { name: "Next image" }));
    expect(screen.getByText("2 / 3")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Previous image" }));
    expect(screen.getByText("1 / 3")).toBeInTheDocument();
    const strip = screen.getByRole("button", { name: "Show login-fixed.png" });
    await user.click(strip);
    expect(strip).toHaveAttribute("aria-current", "true");
    expect(strip).toHaveClass("th-thumb");
    expect(screen.getByRole("button", { name: "Show network.png" })).not.toHaveAttribute("aria-current");
  });

  it("closes on esc and gives focus back to the thumbnail", async () => {
    const user = await openHarness();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "open thumbnail" })).toHaveFocus();
  });

  it("closes from the close button, the stage and a cancel request", async () => {
    const user = await openHarness();
    await user.click(screen.getByRole("button", { name: "close esc" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "open thumbnail" }));
    await user.click(stageImage());
    expect(dialog()).toBeInTheDocument();
    await user.click(dialog().querySelector(".th-lightbox-stage") as HTMLElement);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "open thumbnail" }));
    const cancel = new Event("cancel", { cancelable: true });
    act(() => {
      dialog().dispatchEvent(cancel);
    });
    expect(cancel.defaultPrevented).toBe(true);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("keeps tab focus inside the dialog", async () => {
    const user = await openHarness(2);
    const close = screen.getByRole("button", { name: "close esc" });
    const lastStrip = screen.getByRole("button", { name: "Show login-fixed.png" });
    const copy = screen.getByRole("button", { name: "copy" });
    lastStrip.focus();
    await user.tab();
    expect(copy).toHaveFocus();
    await user.tab({ shift: true });
    expect(lastStrip).toHaveFocus();
    close.focus();
    await user.tab();
    expect(screen.getByRole("button", { name: "Previous image" })).toHaveFocus();
  });

  it("adds the image's pixel size once it loads", async () => {
    await openHarness(1);
    Object.defineProperty(stageImage(), "naturalWidth", { value: 1440 });
    Object.defineProperty(stageImage(), "naturalHeight", { value: 900 });
    fireEvent.load(stageImage());
    expect(dialog().querySelector(".th-lightbox-meta")).toHaveTextContent("1440 × 900 · 96 KB · from you · 10:42");
  });

  it("copies the image with the button or c and says so", async () => {
    const copy = vi.spyOn(actions, "copyImage").mockResolvedValue(undefined);
    const user = await openHarness(1);
    await user.click(screen.getByRole("button", { name: "copy" }));
    expect(copy).toHaveBeenCalledWith("/img/network.png");
    expect(await screen.findByRole("button", { name: "copied" })).toBeInTheDocument();
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "copy" })).toBeInTheDocument();
    await user.keyboard("c");
    expect(copy).toHaveBeenLastCalledWith("/img/login-fixed.png");
  });

  it("says when a copy or download fails", async () => {
    vi.spyOn(actions, "copyImage").mockRejectedValue(new Error("this window cannot copy images"));
    const download = vi.spyOn(actions, "downloadFile").mockRejectedValue(new Error("HTTP 404"));
    const user = await openHarness(1);
    await user.click(screen.getByRole("button", { name: "copy" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "error: cannot copy network.png: this window cannot copy images",
    );
    await user.click(screen.getByRole("button", { name: "download" }));
    expect(download).toHaveBeenCalledWith("/img/network.png", "network.png");
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent("error: cannot download network.png: HTTP 404"),
    );
  });

  it("downloads quietly when it works", async () => {
    vi.spyOn(actions, "downloadFile").mockResolvedValue(undefined);
    const user = await openHarness();
    await user.click(screen.getByRole("button", { name: "download" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows the key hints in the foot", async () => {
    await openHarness();
    const foot = dialog().querySelector(".th-lightbox-foot") as HTMLElement;
    expect(foot).toHaveTextContent("← → step · c copy · esc close");
  });

  it("keeps its keys from the app's global shortcuts", async () => {
    const onWindowKey = vi.fn();
    window.addEventListener("keydown", onWindowKey);
    const user = await openHarness();
    await user.keyboard("n/");
    window.removeEventListener("keydown", onWindowKey);
    expect(onWindowKey).not.toHaveBeenCalled();
  });

  it("clamps a start index past the end", () => {
    render(<Lightbox images={images} startIndex={9} onClose={vi.fn()} />);
    expect(screen.getByText("3 / 3")).toBeInTheDocument();
  });
});
