import { useThrottledValue } from '@mantine/hooks';
import { notifications } from '@mantine/notifications';
import { TFunction } from 'i18next';
import { useState } from 'react';
import { AccountId } from '../common/accountUtils';
import { AccountInfo, Exit } from '../common/api';
import { AppStatus, FeatureFlagKey, NEVPNStatus, OsStatus, PinnedLocation } from '../common/appContext';
import { errMsg, normalizeError } from '../common/utils';
import { fmtErrorI18n } from '../translations/i18n';

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

async function invoke(command: string, args: Object = {}): Promise<unknown> {
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

export async function setColorScheme(value: 'dark' | 'light' | 'auto'): Promise<void> {
    await invoke('setColorScheme', { value });
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

export async function connect(exit: ExitSelector): Promise<void> {
    let args: TunnelArgs = {
      exit,
    };
    await invoke('startTunnel', {
      tunnelArgs: JSON.stringify(args),
    });
}

export async function disconnect(): Promise<void> {
    await invoke('stopTunnel');
}

export async function debuggingArchive(): Promise<String> {
    return (await invoke('debuggingArchive')) as String;
}

export function revealItemInDir(path: String) {
    return invoke('revealItemInDir', { path });
}

export async function emailDebugArchive(path: String, subject: String, body: String): Promise<void> {
    await invoke('emailDebugArchive', { path, subject, body });
}

// trigger native share dialog
export async function shareDebugArchive(path: String): Promise<void> {
    await invoke('shareDebugArchive', { path });
}

export interface Notice {
  type: 'Error' | 'Warn' | 'Important',
  content: string
}


export async function registerAsLoginItem(): Promise<void> {
  await invoke('registerAsLoginItem');
}

export async function unregisterAsLoginItem(): Promise<void> {
  await invoke('unregisterAsLoginItem');
}

export async function developerResetUserDefaults(): Promise<void> {
  await invoke('resetUserDefaults');
}

export async function checkForUpdates(): Promise<void> {
  await invoke('checkForUpdates');
}

export async function installUpdate(): Promise<void> {
  await invoke('installUpdate');
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
  return await jsonFfiCmd(
    'getExitList',
    { knownVersion: version },
    null
  ) as CachedValue<ExitList>;
}

export async function refreshExitList(freshnessS: number): Promise<void> {
  await jsonFfiCmd('refreshExitList', {
    freshness: freshnessS * 1000,
  });
}

export async function deleteAccount(): Promise<void> {
    await jsonFfiCmd('apiDeleteAccount');
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

export async function setFeatureFlag(flag: FeatureFlagKey, active: boolean) {
  await jsonFfiCmd('setFeatureFlag', { flag, active });
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

/**
 * Hook for calling bridge commands with no return values with loading and error state management.
 *
 * @returns Object containing:
 *   - loading: boolean indicating if command is in progress
 *   - showLoadingUI: boolean indicating whether caller should show loading UI
 *   - error: string with error message if command failed
 *   - execute: function that wraps the async command with loading/error handling
 *
 * @example
 * const { loading, error, execute } = useAsyncCommand();
 *
 * const onChange = (checked: boolean) =>
 *   execute(() => commands.setFeatureFlag(FeatureFlagKey.KillSwitch, checked));
 */
export function useAsyncCommand() {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();
  const showLoadingUI = useThrottledValue(loading, loading ? 200 : 0);

  const execute = async (command: () => Promise<unknown>) => {
    if (loading) return;
    setLoading(true);
    setError(undefined);
    try {
      await command();
    } catch (err) {
      setError(errMsg(err));
    } finally {
      setLoading(false)
    }
  };

  return { loading, showLoadingUI, error, execute };
}
