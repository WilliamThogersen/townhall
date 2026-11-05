function styleRules(rules: CSSRuleList): readonly CSSStyleRule[] {
  return [...rules].flatMap((rule) => {
    if (rule instanceof CSSStyleRule) return [rule];
    return rule instanceof CSSGroupingRule ? styleRules(rule.cssRules) : [];
  });
}

export function declarationsFor(css: string, selector: string): CSSStyleDeclaration | null {
  const element = document.createElement("style");
  element.textContent = css;
  document.head.append(element);
  try {
    const rules = element.sheet ? styleRules(element.sheet.cssRules) : [];
    return rules.find((rule) => rule.selectorText === selector)?.style ?? null;
  } finally {
    element.remove();
  }
}
