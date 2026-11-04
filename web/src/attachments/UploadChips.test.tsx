import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Attachment } from "../api/types";
import { attachmentFor } from "./testXhr";
import { UploadChips } from "./UploadChips";
import { newUpload, type PendingUpload } from "./uploads";

function fileOf(name: string, type: string, size: number): File {
  const file = new File(["x"], name, { type });
  Object.defineProperty(file, "size", { value: size });
  return file;
}

const png = fileOf("redirect-loop.png", "image/png", 186368);
const sql = fileOf("sessions-schema.sql", "", 4300);

function upload(file: File, overrides: Partial<PendingUpload> = {}): PendingUpload {
  return { ...newUpload(`k-${file.name}`, file, file.type ? `blob:${file.name}` : null, null), ...overrides };
}

const done = (file: File) =>
  upload(file, { status: "done", percent: 100, attachment: attachmentFor("a1", file.name) as Attachment });

describe("UploadChips", () => {
  it("renders nothing without uploads", () => {
    const { container } = render(<UploadChips items={[]} onRemove={vi.fn()} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("shows a finished image as a thumbnail that can be removed", async () => {
    const onRemove = vi.fn();
    render(<UploadChips items={[done(png)]} onRemove={onRemove} />);
    const group = screen.getByRole("group", { name: "Attachments" });
    expect(group).toHaveClass("th-composer-attachments");
    const image = screen.getByRole("img", { name: "redirect-loop.png" });
    expect(image).toHaveAttribute("src", "blob:redirect-loop.png");
    const thumb = image.closest(".th-thumb");
    expect(thumb).not.toHaveAttribute("data-state");
    const remove = screen.getByRole("button", { name: "Remove redirect-loop.png" });
    expect(remove).toHaveClass("th-thumb-remove");
    expect(remove).toHaveTextContent("×");
    await userEvent.click(remove);
    expect(onRemove).toHaveBeenCalledWith("k-redirect-loop.png");
  });

  it("shows an uploading image with its progress line and a cancel", async () => {
    const onRemove = vi.fn();
    render(<UploadChips items={[upload(png, { percent: 60 })]} onRemove={onRemove} />);
    const image = screen.getByRole("img", { name: "redirect-loop.png, uploading 60%" });
    const thumb = image.closest(".th-thumb") as HTMLElement;
    expect(thumb).toHaveAttribute("data-state", "uploading");
    expect(thumb.style.getPropertyValue("--progress")).toBe("60%");
    await userEvent.click(screen.getByRole("button", { name: "Cancel upload of redirect-loop.png" }));
    expect(onRemove).toHaveBeenCalledWith("k-redirect-loop.png");
  });

  it("shows a failed image with a red border that removes it on click", async () => {
    const onRemove = vi.fn();
    const failed = upload(png, { status: "failed", error: "redirect-loop.png is over 20 MB" });
    render(<UploadChips items={[failed]} onRemove={onRemove} />);
    const thumb = screen.getByRole("button", { name: "Remove redirect-loop.png, error: redirect-loop.png is over 20 MB" });
    expect(thumb).toHaveClass("th-thumb");
    expect(thumb).toHaveAttribute("data-state", "error");
    expect(thumb).toHaveAttribute("title", "error: redirect-loop.png is over 20 MB");
    await userEvent.click(thumb);
    expect(onRemove).toHaveBeenCalledWith("k-redirect-loop.png");
  });

  it("shows an image without a preview as an empty thumbnail", () => {
    render(<UploadChips items={[upload(png, { previewUrl: null })]} onRemove={vi.fn()} />);
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancel upload of redirect-loop.png" })).toBeInTheDocument();
  });

  it("shows a finished file as a chip with its extension, name and size", async () => {
    const onRemove = vi.fn();
    render(<UploadChips items={[done(sql)]} onRemove={onRemove} />);
    const chip = screen.getByText("sessions-schema.sql").closest(".th-file") as HTMLElement;
    expect(chip.querySelector(".th-file-ext")).toHaveTextContent("sql");
    expect(chip.querySelector(".th-file-size")).toHaveTextContent("4.2 KB");
    expect(chip).not.toHaveAttribute("data-state");
    await userEvent.click(screen.getByRole("button", { name: "Remove sessions-schema.sql" }));
    expect(onRemove).toHaveBeenCalledWith("k-sessions-schema.sql");
  });

  it("shows an uploading file's percent in place of its size", () => {
    render(<UploadChips items={[upload(sql, { percent: 25 })]} onRemove={vi.fn()} />);
    const chip = screen.getByText("sessions-schema.sql").closest(".th-file") as HTMLElement;
    expect(chip).toHaveAttribute("data-state", "uploading");
    expect(chip.querySelector(".th-file-size")).toHaveTextContent("25%");
    expect(screen.getByRole("button", { name: "Cancel upload of sessions-schema.sql" })).toBeInTheDocument();
  });

  it("marks a failed file chip and says why in its title", () => {
    const failed = upload(sql, { status: "failed", error: "cannot reach the daemon" });
    render(<UploadChips items={[failed]} onRemove={vi.fn()} />);
    const chip = screen.getByText("sessions-schema.sql").closest(".th-file") as HTMLElement;
    expect(chip).toHaveAttribute("data-state", "error");
    expect(chip).toHaveAttribute("title", "error: cannot reach the daemon");
  });
});
