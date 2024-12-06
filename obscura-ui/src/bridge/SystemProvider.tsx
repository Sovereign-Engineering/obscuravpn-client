import { createContext, ErrorInfo, PropsWithChildren, useContext } from 'react';

const BUNDLE_ID = 'Obscura-VPN'

interface SystemProvideContext {
  BUNDLE_ID: string,
  loading: boolean,
  osType: string,
  osPlatform: string,
  fileSep: string,
  usingCustomTitleBar: boolean,
  logDir: undefined,
  defaultLogFile: undefined,
}

const SystemContext = createContext<SystemProvideContext>({} as SystemProvideContext);

export const useSystemContext = () => useContext(SystemContext);

export function SystemProvider({ children }: PropsWithChildren) {
  const contextValues = {
    BUNDLE_ID,
    loading: false,
    fileSep: '/',
    osType: 'darwin',
    osPlatform: 'Darwin',
    usingCustomTitleBar: false,
    logDir: undefined,
    defaultLogFile: undefined,
  }

  return <SystemContext.Provider value={contextValues}>
    {children}
  </SystemContext.Provider>;
}

export async function logReactError(error: Error, info: ErrorInfo) {
  console.error(`Render error "${error.message}"; ComponentStack = ${info.componentStack}`);
}
