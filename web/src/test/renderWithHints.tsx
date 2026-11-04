import { render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { KeyHintsProvider, useCurrentKeyHints } from "../shell/keyHints";

function Hints() {
  return <output>{useCurrentKeyHints().map((h) => h.keys).join(" ")}</output>;
}

export function renderWithHints(view: ReactNode) {
  return render(
    <KeyHintsProvider>
      {view}
      <Hints />
    </KeyHintsProvider>,
  );
}

export function shownHints(): string | null {
  return screen.getByRole("status").textContent;
}
