import { TFunction } from 'i18next';
import { AccountId } from '../common/accountUtils';
import { AccountInfo, Exit } from '../common/api';
import { AppStatus, NEVPNStatus, OsStatus, PinnedLocation } from '../common/appContext';
import { fmt } from '../common/fmt';
import { normalizeError } from '../common/utils';
import { fmtErrorI18n } from '../translations/i18n';
import { notifications } from '@mantine/notifications';

async function WKWebViewInvoke(command: string, args: Object) {
    const commandJson = JSON.stringify({ [command]: args });
    if (command !== 'jsonFfiCmd') {
      console.log("invoked non-FFI command", command);
    }
    let resultJson;
    try {
        resultJson = await window.webkit.messageHandlers.commandBridge.postMessage(commandJson);
    } catch (e) {
        throw new CommandError(normalizeError(e).message);
    }
    return JSON.parse(resultJson);
}

async function invoke(command: string, args: Object = {}) {
    // all commands are logged for wkwebview according to ContentView.swift
    try {
        return await WKWebViewInvoke(command, args);
    } catch (e) {
        console.error("Command failed", command, args, e);
        throw e;
    }
}

export class CommandError extends Error {
    code: string

    constructor(code: string) {
        // HACK: We should put some "human readable" message into the message field but lots of code currently just hopes to find specific error codes in the message field. So until we hunt down all of those just put the code in the message as well. Don't write new code that treats `message` as machine readable.
        super(code);
        this.code = code;
    }

    i18nKey() {
        return `ipcError-${this.code}`;
    }
}

// VPN Client Specific Commands

export async function jsonFfiCmd(cmd: string, arg = {}, timeoutMs: number | null = 10_000): Promise<unknown> {
    let jsonCmd = JSON.stringify(({ [cmd]: arg }));
    console.log("invoked FFI command", cmd);
    return await invoke('jsonFfiCmd', {
        cmd: jsonCmd,
        timeoutMs,
    })
}

export async function status(lastStatusId: string | null = null): Promise<AppStatus> {
    return await jsonFfiCmd(
        'getStatus',
        { knownVersion: lastStatusId },
        null,
    ) as AppStatus;
}

export async function osStatus(lastOsStatusId: string | null = null): Promise<OsStatus> {
    return await invoke('getOsStatus', { knownVersion: lastOsStatusId }) as OsStatus;
}

export function login(accountId: AccountId, validate = false) {
    return jsonFfiCmd('login', { accountId, validate });
}

export function logout() {
    return jsonFfiCmd('logout');
}

export async function setApiUrl(url: string | null): Promise<void> {
    await jsonFfiCmd("setApiUrl", { url });
}

export async function setApiHostAlternate(host: string | null): Promise<void> {
    await jsonFfiCmd('setApiHostAlternate', { host });
}

export async function setSniRelay(host: string | null): Promise<void> {
    await jsonFfiCmd('setSniRelay', { host });
}

export async function setStrictLeakPrevention(enable: boolean): Promise<void> {
    await invoke('setStrictLeakPrevention', { enable });
}

// See ../../../rustlib/src/manager.rs
export interface TunnelArgs {
    exit: ExitSelector,
}

export interface ExitSelectorId {
  id: string;
}

export interface ExitSelectorCity {
  country_code: string,
  city_code: string,
}

export interface ExitSelectorCountry {
  country_code: string,
}

// See ../../../rustlib/src/manager.rs
export type ExitSelector =
  | { any: {} }
  | { exit: ExitSelectorId }
  | { city: ExitSelectorCity }
  | { country: ExitSelectorCountry }
;

export function connect(exit: ExitSelector) {
    let args: TunnelArgs = {
      exit,
    };
    return invoke('startTunnel', {
      tunnelArgs: JSON.stringify(args),
    });
}

export function disconnect() {
    return invoke('stopTunnel');
}

export async function disconnectBlocking() {
    await disconnect();
    let knownStatusId = null;
    // NEStatus
    while (true) {
        try {
            const s = await status(knownStatusId);
            if (s.vpnStatus.disconnected !== undefined) break;
            knownStatusId = s.version;
        } catch (e) {
            console.error(fmt`failed to get status in disconnectThenConnect ${e}`);
        }
    }
    // NEVPNStatus
    knownStatusId = null;
    while (true) {
      try {
          const s = await osStatus(knownStatusId);
          if (s.osVpnStatus === NEVPNStatus.Disconnected) break;
          knownStatusId = s.version;
      } catch (e) {
          console.error(fmt`failed to get osStatus in disconnectThenConnect ${e}`);
      }
  }
}

export async function debuggingArchive(): Promise<String> {
    return await invoke('debuggingArchive');
}

export function revealItemInDir(path: String) {
    return invoke('revealItemInDir', { path });
}

export interface Notice {
  type: 'Error' | 'Warn' | 'Important',
  content: string
}


export async function notices(): Promise<Notice[]>  {
    return [];
    // TODO
    return await invoke('notices');
}

export function registerAsLoginItem() {
    return invoke('registerAsLoginItem');
}

export function unregisterAsLoginItem() {
  return invoke('unregisterAsLoginItem');
}

export function developerResetUserDefaults() {
    return invoke('resetUserDefaults');
}

export function checkForUpdates() {
  return invoke('checkForUpdates');
}

export function installUpdate() {
  return invoke('installUpdate');
}

export interface TrafficStats {
    connectedMs: number,
    connId: string,
    txBytes: number,
    rxBytes: number,
    latestLatencyMs: number,
}

export async function getTrafficStats(): Promise<TrafficStats> {
    return await jsonFfiCmd('getTrafficStats') as TrafficStats;
}


export interface CachedValue<T> {
  version: string,
  last_updated: number,
  value: T,
}

export interface ExitList {
    exits: Exit[]
}

export async function getExitList(version?: string): Promise<CachedValue<ExitList>> {
  return await jsonFfiCmd('getExitList', {
    knownVersion: version,
  }) as CachedValue<ExitList>;
}

export async function refreshExitList(freshnessS: number): Promise<void> {
  await jsonFfiCmd('refreshExitList', {
    freshness: freshnessS * 1000,
  });
}

export async function getAccount(): Promise<AccountInfo> {
    /* see obscuravpn-api/src/types.rs:AccountInfo */
    return await jsonFfiCmd('apiGetAccountInfo') as AccountInfo;
}

export function setInNewAccountFlow(value: boolean) {
    return jsonFfiCmd('setInNewAccountFlow', { value });
}

export function setPinnedExits(newPinnedExits: PinnedLocation[]) {
    return jsonFfiCmd('setPinnedExits', { exits: newPinnedExits });
}

export function rotateWgKey() {
    return jsonFfiCmd('rotateWgKey');
}

export function setAutoConnect(enable: boolean) {
  return jsonFfiCmd('setAutoConnect', { enable });
}

export async function captureLogs(): Promise<void> {
  await invoke('captureLogs');
}

// trigger native share dialog
export async function shareLogs(): Promise<void> {
  await invoke('shareLogs');
}

export function useHandleCommand(t: TFunction) {
  return async (command: () => Promise<void> | void) => {
    try {
      await command();
    } catch (e) {
      const error = normalizeError(e);
      const message = error instanceof CommandError
        ? fmtErrorI18n(t, error) : error.message;
      notifications.show({
        color: 'red',
        title: t('Error'),
        message
      });
    }
  };
}
