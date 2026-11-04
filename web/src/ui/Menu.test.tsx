import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Menu, type MenuItem } from "./Menu";

function makeItems() {
  const end = vi.fn();
  const auto = vi.fn();
  const shop = vi.fn();
  const items: MenuItem[] = [
    { id: "end", label: "end conversation", onSelect: end },
    { id: "auto", label: "repo: auto", detail: "townhall", role: "menuitemradio", checked: true, onSelect: auto },
    { id: "shop", label: "repo: shop", detail: "develop", role: "menuitemradio", checked: false, onSelect: shop },
  ];
  return { items, end, auto, shop };
}

function renderMenu(position?: { x: number; y: number }) {
  const made = makeItems();
  const onClose = vi.fn();
  render(
    <div>
      <button type="button">outside</button>
      <Menu items={made.items} label="Conversation" onClose={onClose} position={position} />
    </div>,
  );
  return { ...made, onClose };
}

function item(name: string) {
  return screen.getByText(name).closest(".th-option") as HTMLElement;
}

describe("Menu", () => {
  it("renders the menu structure from the preview", () => {
    renderMenu();
    const menu = screen.getByRole("menu", { name: "Conversation" });
    expect(menu).toHaveClass("th-menu");
    const list = menu.firstElementChild;
    expect(list?.tagName).toBe("UL");
    expect(list).toHaveClass("th-menu-list");
    expect(list).toHaveAttribute("role", "none");
    expect(list?.querySelectorAll(":scope > li.th-option")).toHaveLength(3);
  });

  it("gives items their roles, label and detail spans", () => {
    renderMenu();
    expect(screen.getAllByRole("menuitem")).toHaveLength(1);
    expect(screen.getAllByRole("menuitemradio")).toHaveLength(2);
    const spans = item("repo: auto").querySelectorAll(":scope > span");
    expect(spans).toHaveLength(2);
    expect(spans[0]).toHaveTextContent("repo: auto");
    expect(spans[1]).toHaveTextContent("townhall");
    expect(item("end conversation").querySelectorAll(":scope > span")).toHaveLength(2);
  });

  it("sets aria-checked on radio items only", () => {
    renderMenu();
    expect(screen.getByRole("menuitemradio", { name: /repo: auto/ })).toHaveAttribute("aria-checked", "true");
    expect(screen.getByRole("menuitemradio", { name: /repo: shop/ })).toHaveAttribute("aria-checked", "false");
    expect(screen.getByRole("menuitem")).not.toHaveAttribute("aria-checked");
  });

  it("marks the checked radio item so the reference css draws its check", () => {
    renderMenu();
    expect(item("repo: auto")).toHaveAttribute("aria-selected", "true");
    expect(item("repo: shop")).not.toHaveAttribute("aria-selected");
    expect(item("end conversation")).not.toHaveAttribute("aria-selected");
  });

  it("focuses the first item on mount and marks it active", () => {
    renderMenu();
    expect(item("end conversation")).toHaveFocus();
    expect(item("end conversation")).toHaveAttribute("data-active", "true");
    expect(item("repo: auto")).not.toHaveAttribute("data-active");
  });

  it("moves with the arrow keys and wraps", async () => {
    renderMenu();
    await userEvent.keyboard("{ArrowDown}");
    expect(item("repo: auto")).toHaveFocus();
    expect(item("repo: auto")).toHaveAttribute("data-active", "true");
    expect(item("end conversation")).not.toHaveAttribute("data-active");
    await userEvent.keyboard("{ArrowDown}{ArrowDown}");
    expect(item("end conversation")).toHaveFocus();
    await userEvent.keyboard("{ArrowUp}");
    expect(item("repo: shop")).toHaveFocus();
  });

  it("jumps to the ends with home and end", async () => {
    renderMenu();
    await userEvent.keyboard("{End}");
    expect(item("repo: shop")).toHaveFocus();
    await userEvent.keyboard("{Home}");
    expect(item("end conversation")).toHaveFocus();
  });

  it("selects the active item with enter, then closes", async () => {
    const { shop, end, onClose } = renderMenu();
    await userEvent.keyboard("{ArrowUp}{Enter}");
    expect(shop).toHaveBeenCalledTimes(1);
    expect(end).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("selects the active item with space", async () => {
    const { end, onClose } = renderMenu();
    await userEvent.keyboard(" ");
    expect(end).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("selects a clicked item, then closes", async () => {
    const { auto, onClose } = renderMenu();
    await userEvent.click(screen.getByText("repo: auto"));
    expect(auto).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("follows the pointer", () => {
    renderMenu();
    fireEvent.mouseMove(item("repo: shop"));
    expect(item("repo: shop")).toHaveAttribute("data-active", "true");
    expect(item("repo: shop")).toHaveFocus();
  });

  it("closes on escape without selecting", async () => {
    const { end, onClose } = renderMenu();
    await userEvent.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(end).not.toHaveBeenCalled();
  });

  it("closes on tab", async () => {
    const { onClose } = renderMenu();
    await userEvent.keyboard("{Tab}");
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("closes on a click outside", async () => {
    const { onClose } = renderMenu();
    await userEvent.click(screen.getByRole("button", { name: "outside" }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("stays open on a click inside that hits no item", () => {
    const { onClose } = renderMenu();
    fireEvent.mouseDown(screen.getByRole("menu"));
    expect(onClose).not.toHaveBeenCalled();
  });

  it("ignores other keys", async () => {
    const { onClose } = renderMenu();
    await userEvent.keyboard("x");
    expect(onClose).not.toHaveBeenCalled();
    expect(item("end conversation")).toHaveFocus();
  });

  it("stops listening for outside clicks after unmount", () => {
    const { items } = makeItems();
    const onClose = vi.fn();
    const { unmount } = render(<Menu items={items} label="Conversation" onClose={onClose} />);
    unmount();
    fireEvent.mouseDown(document.body);
    expect(onClose).not.toHaveBeenCalled();
  });

  it("is fixed at the given position", () => {
    renderMenu({ x: 40, y: 60 });
    const menu = screen.getByRole("menu");
    expect(menu.style.position).toBe("fixed");
    expect(menu.style.left).toBe("40px");
    expect(menu.style.top).toBe("60px");
  });

  it("stays inside the window near its edges", () => {
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
      width: 320,
      height: 140,
    } as DOMRect);
    renderMenu({ x: window.innerWidth - 10, y: window.innerHeight - 10 });
    const menu = screen.getByRole("menu");
    expect(menu.style.left).toBe(`${window.innerWidth - 320 - 8}px`);
    expect(menu.style.top).toBe(`${window.innerHeight - 140 - 8}px`);
    vi.restoreAllMocks();
  });

  it("stays in the flow without a position", () => {
    renderMenu();
    expect(screen.getByRole("menu").style.position).toBe("");
  });

  it("renders an empty menu without failing", async () => {
    const onClose = vi.fn();
    render(<Menu items={[]} label="Empty" onClose={onClose} />);
    expect(screen.getByRole("menu", { name: "Empty" })).toBeInTheDocument();
    await userEvent.keyboard("{ArrowDown}{Enter}");
    expect(onClose).not.toHaveBeenCalled();
  });
});
