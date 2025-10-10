import { createContext, useContext } from 'react';
import { ExitSelector, ExitSelectorCity, TunnelArgs } from 'src/bridge/commands';
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
    },
    canSendMail: boolean,
    loginItemStatus?: {
        registered: boolean,
        error?: string
    }
}

export enum TransportKind {
    Quic = 'quic',
    TcpTls = 'tcpTls',
}

export interface VpnStatus {
    connected?: {
      exit: Exit,
      clientPublicKey: string,
      exitPublicKey: string,
      transport: TransportKind,
      tunnelArgs: TunnelArgs,
    },
    connecting?: {
      connectError: string,
      reconnecting: boolean
      tunnelArgs: TunnelArgs,
    },
    disconnected?: {}
}

export function getCityFromStatus(status: VpnStatus): ExitSelectorCity | undefined {
  const tunnelArgs = getTunnelArgs(status);
  return getCityFromArgs(tunnelArgs?.exit);
}

export function getCityFromArgs(exitSelector: ExitSelector | undefined): ExitSelectorCity | undefined {
  return exitSelector !== undefined && "city" in exitSelector ? exitSelector.city : undefined;
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

// See rustlib/src/config/feature_flags.rs
export enum KnownFeatureFlagKey {
  QuicFramePadding = "quicFramePadding",
  KillSwitch = "killSwitch",
}

export type FeatureFlagKey = KnownFeatureFlagKey | string;

export type FeatureFlagValue = boolean | null;

export function featureFlagEnabled(value: FeatureFlagValue | undefined): boolean {
  return value === true;
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
    forceTcpTlsRelayTransport: boolean,
    featureFlags: Record<FeatureFlagKey, FeatureFlagValue>,
    featureFlagKeys: FeatureFlagKey[],
}

interface IAppContext {
    vpnConnected: boolean,
    // the exitSelector used to initiate the connection
    initiatingExitSelector?: ExitSelector,
    vpnConnect: (exit: ExitSelector) => Promise<void>,
    vpnDisconnect: () => Promise<void>,
    pollAccount: () => Promise<void>,
    accountLoading: boolean,
    appStatus: AppStatus,
    osStatus: OsStatus,
    showOfflineUI: boolean,
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

/**
 * State derived isConnecting hook
 */
export function useIsConnecting() {
  const { connectionInProgress, osStatus, appStatus } = useContext(AppContext);
  return osStatus.osVpnStatus === NEVPNStatus.Connecting
    || osStatus.osVpnStatus === NEVPNStatus.Reasserting
    || connectionInProgress === ConnectionInProgress.ChangingLocations
    || appStatus.vpnStatus.connecting !== undefined;
}

export function useIsTransitioning() {
  const { connectionInProgress, osStatus, appStatus } = useContext(AppContext);
  return osStatus.osVpnStatus === NEVPNStatus.Connecting
    || osStatus.osVpnStatus === NEVPNStatus.Reasserting
    || osStatus.osVpnStatus === NEVPNStatus.Disconnecting
    || connectionInProgress === ConnectionInProgress.ChangingLocations
    || appStatus.vpnStatus.connecting !== undefined;
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

export function connectionIsIdle(connectionInProgress: ConnectionInProgress, vpnStatus: VpnStatus, osVpnStatus: NEVPNStatus) {
  return connectionInProgress === ConnectionInProgress.UNSET
    && vpnStatus.disconnected !== undefined
    && (
      osVpnStatus === NEVPNStatus.Disconnected ||
      osVpnStatus === NEVPNStatus.Invalid
    );
}
