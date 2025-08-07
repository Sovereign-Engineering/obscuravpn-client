import { Translation } from "react-i18next";
import { PLATFORM } from "../bridge/SystemProvider";

export function VpnError({ errorEnum }: { errorEnum: string }) {
  return (
    <Translation>
      {(t,) => t(`vpnError-${errorEnum}`, { context: PLATFORM } as any)}
    </Translation>
  );
}
