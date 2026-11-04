const LOGO_COLUMNS = 18;
const LOGO_ROWS = 14;
const LOGO_PATH =
  "M8 0h2v1h-2zM6 1h6v1h-6zM4 2h10v1h-10zM2 3h14v1h-14zM1 4h16v1h-16zM2 6h2v5h-2zM6 6h2v5h-2zM10 6h2v5h-2zM14 6h2v5h-2zM1 12h16v1h-16zM0 13h18v1h-18z";

export interface LogoProps {
  readonly width?: number;
  readonly className?: string;
}

export function Logo({ width = LOGO_COLUMNS, className }: LogoProps) {
  const classes = className ? `th-logo ${className}` : "th-logo";
  return (
    <svg
      className={classes}
      viewBox={`0 0 ${LOGO_COLUMNS} ${LOGO_ROWS}`}
      width={width}
      height={(width * LOGO_ROWS) / LOGO_COLUMNS}
      fill="currentColor"
      shapeRendering="crispEdges"
      aria-hidden="true"
    >
      <path d={LOGO_PATH} />
    </svg>
  );
}
