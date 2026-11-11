import { startDemo } from "./demo";

export default async function globalSetup(): Promise<() => Promise<void>> {
  return startDemo();
}
