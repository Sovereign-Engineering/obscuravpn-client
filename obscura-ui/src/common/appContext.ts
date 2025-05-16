import { createContext } from 'react';
import { ExitSelector, TunnelArgs } from 'src/bridge/commands';
import { AccountId } from './accountUtils';
import { AccountInfo, Exit } from './api';

export enum NEVPNStatus {
    Invalid = 'invalid',
    Disconnected = 'disconnected',
    Connecting = 'connecting',
    Connected = 'connected',
    Reasserting = 'reasserting',
    Disconnecting = 'disconnecting'
}

export enum UpdaterStatusType {
    Uninitiated = 'uninitiated',
    Initiated = 'initiated',
    Available = 'available',
    NotFound = 'notFound',
    Error = 'error'
}

export interface AppcastSummary {
    date: string;
    description: string;
    version: string;
    minSystemVersionOk: boolean;
}

export interface UpdaterStatus {
    type: UpdaterStatusType;
    appcast?: AppcastSummary;
    error?: string;
    errorCode?: number;
}

export interface OsStatus {
    version: string,
    internetAvailable: boolean,
    osVpnStatus: NEVPNStatus,
    srcVersion: string
    strictLeakPrevention: boolean,
    updaterStatus: UpdaterStatus,
    debugBundleStatus: {
        inProgress?: boolean,
        latestPath?: string,
        inProgressCounter: number,
    }
}

export interface VpnStatus {
    connected?: {
      exit: Exit,
      clientPublicKey: string,
      exitPublicKey: string
      tunnelArgs: TunnelArgs,
    },
    connecting?: {
      connectError: string,
      reconnecting: boolean
      tunnelArgs: TunnelArgs,
    },
    disconnected?: {}
}

export function getTunnelArgs(status: VpnStatus): TunnelArgs | undefined {
  return status.connected?.tunnelArgs ?? status.connecting?.tunnelArgs;
}

export interface PinnedLocation {
    country_code: string,
    city_code: string,

    // Seconds since UNIX epoch.
    pinned_at: number,
}

export interface AccountStatus {
    account_info: AccountInfo,
    last_updated_sec: number
}

export interface AppStatus {
    version: string,
    vpnStatus: VpnStatus,
    accountId: AccountId,
    pinnedLocations: Array<PinnedLocation>,
    lastChosenExit: ExitSelector,
    inNewAccountFlow: boolean,
    apiUrl: string,
    account: AccountStatus | null,
    autoConnect: boolean,
}

interface IAppContext {
    vpnConnected: boolean,
    toggleVpnConnection: () => Promise<void>,
    vpnConnect: (exit: ExitSelector) => Promise<void>,
    vpnDisconnect: () => Promise<void>,
    pollAccount: () => Promise<void>,
    accountLoading: boolean,
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
