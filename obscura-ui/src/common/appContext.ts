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

export interface OsStatus {
    version: string,
    internetAvailable: boolean,
    osVpnStatus: NEVPNStatus,
    srcVersion: string
}

export interface VpnStatus {
    connected?: {
      exit: Exit
    },
    connecting: {},
    disconnected: {},
    reconnecting?: {
        err: string,
    },
}

export interface PinnedLocation {
    country_code: string,
    city_code: string,

    // Seconds since UNIX epoch.
    pinned_at: number,
}

export interface AccountStatus {
    account_info: AccountInfo,
    days_till_expiry: number,
    last_updated_sec: number
}

export interface AppStatus {
    version: string,
    vpnStatus: VpnStatus,
    accountId: AccountId,
    pinnedLocations: Array<PinnedLocation>,
    lastChosenExit: string,
    inNewAccountFlow: boolean,
    apiUrl: string,
    account: AccountStatus | null
}

interface IAppContext {
    vpnConnected: boolean,
    toggleVpnConnection: () => Promise<void>,
    vpnConnect: (exit?: string) => Promise<void>,
    vpnDisconnect: () => Promise<void>,
    vpnDisconnectConnect: (exit: string) => Promise<void>,
    pollAccount: () => Promise<void>,
    appStatus: AppStatus,
    osStatus: OsStatus,
    accountInfo: AccountInfo | null,
    connectionInProgress: ConnectionInProgress
}

export const AppContext = createContext(null as any as IAppContext);

export enum ConnectionInProgress {
    Connecting = 'Connecting',
    Reconnecting = 'Reconnecting',
    Disconnecting = 'Disconnecting',
    // UI exclusives:
    ChangingLocations = 'Changing Locations',
    UNSET = 'UNSET'
}

export function isConnecting(connectionInProgress: ConnectionInProgress) {
    switch (connectionInProgress) {
        case ConnectionInProgress.Connecting:
        case ConnectionInProgress.Reconnecting:
        case ConnectionInProgress.ChangingLocations:
            return true;
    }
    return false;
}

interface IExitsContext {
  fetchExitList: () => Promise<void>,
  exitList: Exit[] | null,
}

export const ExitsContext = createContext(null as any as IExitsContext);
