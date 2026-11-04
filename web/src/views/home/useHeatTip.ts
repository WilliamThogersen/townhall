import { useCallback, useState, type KeyboardEvent, type MouseEvent, type RefObject } from "react";

export interface HeatTip {
  readonly index: number;
  readonly left: number;
  readonly top: number;
}

const DAYS_PER_WEEK = 7;

const STEPS: Readonly<Record<string, number>> = {
  ArrowLeft: -DAYS_PER_WEEK,
  ArrowRight: DAYS_PER_WEEK,
  ArrowUp: -1,
  ArrowDown: 1,
};

export function nextIndex(key: string, current: number, count: number): number | null {
  if (key === "Home") return 0;
  if (key === "End") return count - 1;
  const step = STEPS[key];
  if (step === undefined) return null;
  return Math.min(count - 1, Math.max(0, current + step));
}

function placeTip(map: HTMLElement, cell: Element, index: number): HeatTip {
  const box = map.getBoundingClientRect();
  const rect = cell.getBoundingClientRect();
  return { index, left: rect.left - box.left + rect.width / 2, top: rect.top - box.top };
}

const cellSelector = (index: number) => `.th-heat-grid [data-index="${index}"]`;

export function useHeatTip(mapRef: RefObject<HTMLElement | null>, count: number) {
  const [tip, setTip] = useState<HeatTip | null>(null);

  const showAt = useCallback(
    (index: number) => {
      const map = mapRef.current;
      const cell = map?.querySelector(cellSelector(index));
      if (map && cell) setTip(placeTip(map, cell, index));
    },
    [mapRef],
  );

  const onMouseOver = (e: MouseEvent<HTMLElement>) => {
    const cell = e.target instanceof Element ? e.target.closest(".th-heat-grid [data-index]") : null;
    if (cell instanceof HTMLElement) showAt(Number(cell.dataset.index));
  };

  const onFocus = () => {
    if (tip === null) showAt(count - 1);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLElement>) => {
    const next = nextIndex(e.key, tip?.index ?? count - 1, count);
    if (next === null) return;
    e.preventDefault();
    showAt(next);
  };

  const hide = () => setTip(null);

  return { tip, onMouseOver, onMouseLeave: hide, onFocus, onBlur: hide, onKeyDown };
}
