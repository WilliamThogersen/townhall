import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { Attachment } from "../api/types";
import { attachmentFor } from "./testXhr";
import { lightboxImages, Thumbs } from "./Thumbs";

const at = new Date(2026, 10, 4, 10, 42).toISOString();

const image = (id: string, name: string, size = 98304) =>
  attachmentFor(id, name, { size, createdAt: at }) as Attachment;

const file = (id: string, name: string, size: number) =>
  attachmentFor(id, name, { size, mime: "text/plain", kind: "file", createdAt: at }) as Attachment;

const all: readonly Attachment[] = [
  image("i1", "staging-1.png"),
  image("i2", "staging-2.png"),
  image("i3", "staging-3.png"),
  image("i4", "staging-4.png"),
  image("i5", "staging-5.png"),
  file("f1", "staging-nginx.log", 1153434),
  file("f2", "auth-flow.pdf", 327680),
];

describe("Thumbs", () => {
  it("renders nothing for a message without attachments it knows", () => {
    const { container } = render(<Thumbs conversationId="c1" attachmentIds={["gone"]} attachments={all} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("shows images as thumbnails and files as chips linking to the daemon", () => {
    const { container } = render(
      <Thumbs conversationId="c1" attachmentIds={["i1", "f1", "i2", "f2"]} attachments={all} />,
    );
    const images = container.querySelector(".th-msg-images") as HTMLElement;
    const thumbs = images.querySelectorAll("button.th-thumb");
    expect(thumbs).toHaveLength(2);
    expect(screen.getByRole("img", { name: "staging-1.png" })).toHaveAttribute(
      "src",
      "/api/conversations/c1/attachments/i1",
    );
    const files = container.querySelector(".th-msg-files") as HTMLElement;
    const log = screen.getByRole("link", { name: /staging-nginx\.log/ });
    expect(files).toContainElement(log);
    expect(log).toHaveClass("th-file");
    expect(log).toHaveAttribute("href", "/api/conversations/c1/attachments/f1");
    expect(log).toHaveAttribute("download", "staging-nginx.log");
    expect(log.querySelector(".th-file-ext")).toHaveTextContent("log");
    expect(log.querySelector(".th-file-size")).toHaveTextContent("1.1 MB");
  });

  it("leaves out the image row when there are only files", () => {
    const { container } = render(<Thumbs conversationId="c1" attachmentIds={["f2"]} attachments={all} />);
    expect(container.querySelector(".th-msg-images")).toBeNull();
    expect(container.querySelector(".th-msg-files")).not.toBeNull();
  });

  it("shows three images in full", () => {
    const { container } = render(
      <Thumbs conversationId="c1" attachmentIds={["i1", "i2", "i3"]} attachments={all} />,
    );
    expect(container.querySelectorAll(".th-thumb")).toHaveLength(3);
    expect(container.querySelector(".th-thumb-more")).toBeNull();
  });

  it("collapses more than three images into +N", () => {
    const { container } = render(
      <Thumbs conversationId="c1" attachmentIds={["i1", "i2", "i3", "i4", "i5"]} attachments={all} />,
    );
    expect(container.querySelectorAll(".th-thumb img")).toHaveLength(2);
    const more = screen.getByRole("button", { name: "Show 3 more images" });
    expect(more).toHaveClass("th-thumb th-thumb-more");
    expect(more).toHaveTextContent("+3");
  });

  it("shows every image when collapsing is off, as the session panel's files grid does", () => {
    const { container } = render(
      <Thumbs
        conversationId="c1"
        attachmentIds={["i1", "i2", "i3", "i4", "i5"]}
        attachments={all}
        collapse={false}
      />,
    );
    expect(container.querySelectorAll(".th-thumb img")).toHaveLength(5);
    expect(container.querySelector(".th-thumb-more")).toBeNull();
  });

  it("opens the lightbox on the clicked image with that message's images", async () => {
    const user = userEvent.setup();
    render(<Thumbs conversationId="c1" attachmentIds={["i1", "i2", "f1"]} attachments={all} />);
    const second = screen.getByRole("button", { name: "staging-2.png" });
    await user.click(second);
    const dialog = screen.getByRole("dialog", { name: "Image viewer" });
    expect(dialog).toHaveTextContent("2 / 2");
    expect(dialog.querySelector(".th-lightbox-meta")).toHaveTextContent("96 KB · from you · 10:42");
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(second).toHaveFocus();
  });

  it("opens the first hidden image from +N", async () => {
    const user = userEvent.setup();
    render(<Thumbs conversationId="c1" attachmentIds={["i1", "i2", "i3", "i4", "i5"]} attachments={all} />);
    await user.click(screen.getByRole("button", { name: "Show 3 more images" }));
    expect(screen.getByRole("dialog")).toHaveTextContent("3 / 5");
  });

  it("says who shared the images", async () => {
    const user = userEvent.setup();
    render(<Thumbs conversationId="c1" attachmentIds={["i1"]} attachments={all} from="fix-login" />);
    await user.click(screen.getByRole("button", { name: "staging-1.png" }));
    expect(screen.getByRole("dialog")).toHaveTextContent("from fix-login");
  });
});

describe("lightboxImages", () => {
  it("turns image attachments into lightbox images", () => {
    expect(lightboxImages("c1", [image("i1", "staging-1.png")], "you", {})).toEqual([
      {
        src: "/api/conversations/c1/attachments/i1",
        name: "staging-1.png",
        meta: "96 KB · from you · 10:42",
      },
    ]);
  });
});
