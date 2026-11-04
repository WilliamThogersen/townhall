import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type RefObject,
} from "react";

export type MenuItem = {
  id: string;
  label: string;
  detail?: string;
  role?: "menuitem" | "menuitemradio";
  checked?: boolean;
  onSelect: () => void;
};

type MenuPosition = { x: number; y: number };

type MenuProps = {
  items: readonly MenuItem[];
  label: string;
  onClose: () => void;
  position?: MenuPosition;
};

const EDGE_GAP = 8;

const fit = (start: number, extent: number, limit: number) => Math.max(0, Math.min(start, limit - extent - EDGE_GAP));

function nextIndex(key: string, active: number, count: number): number | null {
  switch (key) {
    case "ArrowDown":
      return (active + 1) % count;
    case "ArrowUp":
      return (active - 1 + count) % count;
    case "Home":
      return 0;
    case "End":
      return count - 1;
    default:
      return null;
  }
}

function useOutsideMouseDown(ref: RefObject<HTMLElement | null>, onOutside: () => void) {
  const latest = useRef(onOutside);
  useEffect(() => {
    latest.current = onOutside;
  });
  useEffect(() => {
    const onMouseDown = (event: MouseEvent) => {
      if (event.target instanceof Node && ref.current?.contains(event.target)) return;
      latest.current();
    };
    document.addEventListener("mousedown", onMouseDown, true);
    return () => document.removeEventListener("mousedown", onMouseDown, true);
  }, [ref]);
}

function useFixedStyle(ref: RefObject<HTMLElement | null>, position?: MenuPosition): CSSProperties | undefined {
  const [size, setSize] = useState({ width: 0, height: 0 });
  useLayoutEffect(() => {
    const rect = ref.current?.getBoundingClientRect();
    if (rect) setSize({ width: rect.width, height: rect.height });
  }, [ref]);
  if (!position) return undefined;
  return {
    position: "fixed",
    left: fit(position.x, size.width, window.innerWidth),
    top: fit(position.y, size.height, window.innerHeight),
    zIndex: "var(--th-z-overlay)",
  };
}

export function Menu({ items, label, onClose, position }: MenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [active, setActive] = useState(0);
  const style = useFixedStyle(menuRef, position);
  useOutsideMouseDown(menuRef, onClose);

  useEffect(() => {
    const menu = menuRef.current;
    const target = menu?.querySelectorAll<HTMLElement>(".th-option")[active] ?? menu;
    target?.focus();
  }, [active]);

  const select = (item: MenuItem) => {
    item.onSelect();
    onClose();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    event.stopPropagation();
    if (event.key === "Escape" || event.key === "Tab") {
      event.preventDefault();
      onClose();
      return;
    }
    const current = items[active];
    if (!current) return;
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      select(current);
      return;
    }
    const next = nextIndex(event.key, active, items.length);
    if (next === null) return;
    event.preventDefault();
    setActive(next);
  };

  return (
    <div ref={menuRef} className="th-menu" role="menu" aria-label={label} tabIndex={-1} style={style} onKeyDown={onKeyDown}>
      <ul className="th-menu-list" role="none">
        {items.map((item, index) => (
          <MenuOption
            key={item.id}
            item={item}
            active={index === active}
            onSelect={() => select(item)}
            onHover={() => setActive(index)}
          />
        ))}
      </ul>
    </div>
  );
}

type MenuOptionProps = {
  item: MenuItem;
  active: boolean;
  onSelect: () => void;
  onHover: () => void;
};

function MenuOption({ item, active, onSelect, onHover }: MenuOptionProps) {
  const radio = item.role === "menuitemradio";
  return (
    <li
      className="th-option"
      role={item.role ?? "menuitem"}
      aria-checked={radio ? Boolean(item.checked) : undefined}
      aria-selected={radio && item.checked ? true : undefined}
      data-active={active ? "true" : undefined}
      tabIndex={-1}
      onClick={onSelect}
      onMouseMove={active ? undefined : onHover}
    >
      <span>{item.label}</span>
      <span>{item.detail}</span>
    </li>
  );
}
