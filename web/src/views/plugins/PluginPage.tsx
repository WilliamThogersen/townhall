import { PluginSlot, type SlotServices } from "../../plugins/PluginSlot";

export interface PluginPageProps {
  readonly plugin: string;
  readonly page: string;
  readonly services?: Partial<SlotServices>;
}

export function PluginPage({ plugin, page, services }: PluginPageProps) {
  return (
    <div className="th-plugin-page">
      <PluginSlot name="page" context={{ page }} only={{ plugin, entry: page }} services={services} />
    </div>
  );
}
