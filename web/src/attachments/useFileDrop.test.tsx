import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { DropZone } from "./DropZone";
import { useFileDrop } from "./useFileDrop";

function Area({ onFiles, enabled }: { onFiles: (files: readonly File[]) => void; enabled?: boolean }) {
  const drop = useFileDrop(onFiles, enabled);
  return (
    <section aria-label="Conversation" {...drop.dropProps}>
      {drop.dragging && <DropZone />}
    </section>
  );
}

const shot = new File(["x"], "shot.png", { type: "image/png" });

function filesTransfer(files: readonly File[] = []) {
  return { dataTransfer: { types: ["Files"], files, dropEffect: "none" } };
}

const area = () => screen.getByRole("region", { name: "Conversation" });

describe("DropZone", () => {
  it("says what can be dropped", () => {
    const { container } = render(<DropZone />);
    const zone = container.querySelector(".th-drop");
    expect(zone).toHaveTextContent("drop to attach");
    expect(zone?.querySelector("span")).toHaveTextContent("images, text, code, pdf · up to 20 MB each");
  });
});

describe("useFileDrop", () => {
  it("shows the drop zone while files are dragged over the area", () => {
    render(<Area onFiles={vi.fn()} />);
    fireEvent.dragEnter(area(), filesTransfer());
    expect(screen.getByText("drop to attach")).toBeInTheDocument();
    fireEvent.dragLeave(area(), filesTransfer());
    expect(screen.queryByText("drop to attach")).not.toBeInTheDocument();
  });

  it("keeps the zone while the drag moves between children", () => {
    render(<Area onFiles={vi.fn()} />);
    fireEvent.dragEnter(area(), filesTransfer());
    fireEvent.dragEnter(area(), filesTransfer());
    fireEvent.dragLeave(area(), filesTransfer());
    expect(screen.getByText("drop to attach")).toBeInTheDocument();
  });

  it("allows the drop as a copy", () => {
    render(<Area onFiles={vi.fn()} />);
    const transfer = filesTransfer();
    expect(fireEvent.dragOver(area(), transfer)).toBe(false);
  });

  it("hands dropped files over and hides the zone", () => {
    const onFiles = vi.fn();
    render(<Area onFiles={onFiles} />);
    fireEvent.dragEnter(area(), filesTransfer());
    expect(fireEvent.drop(area(), filesTransfer([shot]))).toBe(false);
    expect(onFiles).toHaveBeenCalledWith([shot]);
    expect(screen.queryByText("drop to attach")).not.toBeInTheDocument();
  });

  it("ignores a drop without files", () => {
    const onFiles = vi.fn();
    render(<Area onFiles={onFiles} />);
    fireEvent.drop(area(), filesTransfer([]));
    expect(onFiles).not.toHaveBeenCalled();
  });

  it("ignores dragged text", () => {
    const onFiles = vi.fn();
    render(<Area onFiles={onFiles} />);
    const text = { dataTransfer: { types: ["text/plain"], files: [] } };
    fireEvent.dragEnter(area(), text);
    expect(fireEvent.dragOver(area(), text)).toBe(true);
    fireEvent.drop(area(), text);
    expect(screen.queryByText("drop to attach")).not.toBeInTheDocument();
    expect(onFiles).not.toHaveBeenCalled();
  });

  it("ignores everything while disabled and drops the zone when it gets disabled", () => {
    const onFiles = vi.fn();
    const { rerender } = render(<Area onFiles={onFiles} />);
    fireEvent.dragEnter(area(), filesTransfer());
    rerender(<Area onFiles={onFiles} enabled={false} />);
    expect(screen.queryByText("drop to attach")).not.toBeInTheDocument();
    fireEvent.dragEnter(area(), filesTransfer());
    fireEvent.drop(area(), filesTransfer([shot]));
    expect(screen.queryByText("drop to attach")).not.toBeInTheDocument();
    expect(onFiles).not.toHaveBeenCalled();
  });
});
