import { useInfo } from "../../repos/useInfo";
import { Logo } from "../../ui/Logo";
import { greetingText } from "./greetingText";

const GREETING_LOGO_WIDTH = 36;

export function Greeting({ now }: { readonly now: Date }) {
  const { info } = useInfo();
  return (
    <h1 className="th-greeting">
      <Logo width={GREETING_LOGO_WIDTH} />
      {greetingText(now, info?.userName)}
    </h1>
  );
}
