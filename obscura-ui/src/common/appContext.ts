import { createContext } from 'react';
import { AccountId } from './accountUtils';
import { AccountInfo, Exit } from './api';

export enum NEVPNStatus {
    invalid = 'invalid',
    disconnected = 'disconnected',
    connecting = 'connecting',
    connected = 'connected',
    reasserting = 'reasserting',
    disconnecting = 'disconnecting'
}

interface OsStatus {
    version: string,
    internetAvailable: boolean,
    osVpnStatus: NEVPNStatus,
    srcVersion: string
}

interface ConnectedStatus {
    exit: Exit
}

interface VpnStatus {
    connected: ConnectedStatus,
    connecting: object, // TODO
    disconnected: object, // TODO
    reconnecting: {
        err: unknown,
    },
}

interface AppStatus {
    version: string,
    vpnStatus: VpnStatus,
    accountId: AccountId,
    pinnedExits: Array<string>,
    lastChosenExit: string,
    apiUrl: string
}

interface AppContext {
    vpnConnected: boolean,
    vpnConnect: (exit?: string) => Promise<void>,
    vpnDisconnect: () => Promise<void>,
    vpnDisconnectConnect: (exit: string) => Promise<void>,
    pollAccount: () => Promise<void>,
    appStatus: AppStatus | null,
    osStatus: OsStatus,
    accountInfo: AccountInfo | null,
    connectionInProgress: ConnectionInProgress
}

export const AppContext = createContext(null as any as AppContext);

export enum ConnectionInProgress {
    CONNECTING = 'Connecting',
    RECONNECTING = 'Reconnecting',
    DISCONNECTING = 'Disconnecting',
    // UI exclusives:
    CHANGING_LOCATIONS = 'Changing Locations',
    UNSET = 'UNSET'
}

export const ConnectingStrings = {
    connecting: 'Connecting',
    reconnecting: 'Reconnecting',
    disconnecting: 'Disconnecting',
    // UI exclusives:
    changingLocations: 'Changing Locations',
    UNSET: undefined
}

export function isConnecting(connectionInProgress: string) {
    switch (connectionInProgress) {
        case ConnectingStrings.connecting:
        case ConnectingStrings.reconnecting:
        case ConnectingStrings.changingLocations:
            return true;
    }
    return false;
}

interface ExitsContext {
  fetchExitList: () => Promise<void>,
  exitList: Exit[],
}

export const ExitsContext = createContext(null as any as ExitsContext);
