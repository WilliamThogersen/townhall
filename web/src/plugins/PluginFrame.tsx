import { httpBase } from "../api/base";

export function pluginUiUrl(plugin: string, entry: string, win: object = window): string {
  const file = entry.split("/").map(encodeURIComponent).join("/");
  return `${httpBase(win)}/plugins/${encodeURIComponent(plugin)}/ui/${file}`;
}

export interface PluginFrameProps {
  plugin: string;
  entry: string;
  title: string;
  height?: number;
}

export function PluginFrame({ plugin, entry, title, height }: PluginFrameProps) {
  return (
    <iframe
      src={pluginUiUrl(plugin, entry)}
      title={title}
      sandbox="allow-scripts"
      referrerPolicy="no-referrer"
      allow=""
      style={height === undefined ? undefined : { height: `${height}px` }}
    />
  );
}
