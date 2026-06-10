import { useThrottledValue } from '@mantine/hooks';
import { notifications } from '@mantine/notifications';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { AccountId } from '../common/accountUtils';
import { AccountInfo, Exit } from '../common/api';
import { AppStatus, DNSContentBlock, FeatureFlagKey, NavigationView, NEVPNStatus, OsStatus, OsStatusWVpnStatus, PinnedLocation, SubscriptionProductModel } from '../common/appContext';
import { normalizeError } from '../common/utils';
import { ErrorI18n, fmtErrorI18n, TranslationKey } from '../translations/i18n';
import { HAS_NE_VPN_STATUS, Platform, PLATFORM } from './SystemProvider';
import './setupBridge';

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

export class CommandError extends ErrorI18n {
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

export async function status(lastStatusId: string | null = null, timeoutMs: number | null = null): Promise<AppStatus> {
    return await jsonFfiCmd(
        'getStatus',
        { knownVersion: lastStatusId },
        timeoutMs,
    ) as AppStatus;
}

export async function osStatus(lastOsStatusId: string | null = null): Promise<OsStatus> {
    return await invoke('getOsStatus', { knownVersion: lastOsStatusId }) as OsStatus;
}

export async function setNavigationView(view: NavigationView): Promise<void> {
    await invoke('setNavigationView', { view });
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

export async function debugBundle(userFeedback: string): Promise<String> {
    return (await invoke('debugBundle', { userFeedback })) as String;
}

export function revealItemInDir(path: String) {
    return invoke('revealItemInDir', { path });
}

export async function emailDebugBundle(path: String, subject: String, body: String): Promise<void> {
    await invoke('emailDebugBundle', { path, subject, body });
}

// trigger native share dialog
export async function shareDebugBundle(path: String): Promise<void> {
    await invoke('shareDebugBundle', { path });
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

export function setUseSystemDns(enable: boolean) {
  return jsonFfiCmd('setUseSystemDns', { enable });
}

export async function setLocalNetworkAccess(enable: boolean): Promise<void> {
  await jsonFfiCmd('setLocalNetworkAccess', { enable });
}

export async function setFeatureFlag(flag: FeatureFlagKey, active: boolean) {
  await jsonFfiCmd('setFeatureFlag', { flag, active });
}

export async function setDnsContentBlock(value: DNSContentBlock): Promise<void> {
  await jsonFfiCmd('setDnsContentBlock', { value });
}

export async function getSubscriptionProductDisplay(): Promise<SubscriptionProductModel> {
  return await invoke('getSubscriptionProduct') as SubscriptionProductModel;
}

export async function storeKitAssociateAccount(): Promise<void> {
  await invoke('associateAccount');
}

export async function storeKitPurchaseSubscription(): Promise<boolean> {
  return await invoke('purchaseSubscription', {}) as boolean;
}

export async function storeKitRestorePurchases(): Promise<void> {
  await invoke('restorePurchases', {});
}

export async function showOfferCodeRedemption(): Promise<void> {
  await invoke('showOfferCodeRedemption');
}

export async function resetOfferCodeRedemptionSuccess(): Promise<void> {
  if (PLATFORM === Platform.iOS) {
    await invoke('resetOfferCodeRedemptionSuccess');
  }
}

export async function playPurchaseSubscription(promoCode: string | null): Promise<boolean> {
  return await invoke('purchaseSubscription', { promoCode }) as boolean;
}

export class DisconnectTimeoutError extends ErrorI18n {
  i18nKey(): TranslationKey {
    return 'error-timeoutDisconnect';
  }
}

export async function waitUntilDisconnected(
  initialOsStatus: OsStatusWVpnStatus,
): Promise<void> {
  if (initialOsStatus.osVpnStatus === NEVPNStatus.Disconnected) return;
  const startTime = Date.now();
  if (HAS_NE_VPN_STATUS) {
    let latest = initialOsStatus;
    while (latest.osVpnStatus !== NEVPNStatus.Disconnected) {
      if (Date.now() - startTime >= 60_000) throw new DisconnectTimeoutError();
      latest = await osStatus(latest.version) as OsStatusWVpnStatus;
    }
  } else {
    let latest = await status(null);
    while (latest.vpnStatus.disconnected === undefined) {
      if (Date.now() - startTime >= 60_000) throw new DisconnectTimeoutError();
      latest = await status(latest.version);
    }
  }
}

export interface UseCommandOptions<CommandArgs extends any[]> {
  command: (...args: CommandArgs) => Promise<void>;
  /** Whether to show a notification on error. Default: false */
  showNotification?: boolean;
  /** Whether to re-throw the error after handling. Default: false */
  rethrow?: boolean;
}

/**
 * Hook for calling non-return value bridge commands with loading and error state management.
 *
 * @returns Object containing:
 *   - loading: boolean indicating if command is in progress
 *   - showLoadingUI: boolean indicating whether caller should show a throttled loading UI
 *   - error: string with error message if command failed
 */
export function useCommand<CommandArgs extends any[]>({ command, showNotification = false, rethrow = false }: UseCommandOptions<CommandArgs>) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();
  const { t } = useTranslation();
  const showLoadingUI = useThrottledValue(loading, loading ? 200 : 0);

  const execute = async (...args: CommandArgs) => {
    if (loading) return;
    setLoading(true);
    setError(undefined);
    try {
      await command(...args);
    } catch (err) {
      const error = normalizeError(err);
      const message = error instanceof CommandError
        ? fmtErrorI18n(t, error) : error.message;

      setError(message);

      if (showNotification) {
        notifications.show({
          color: 'red',
          title: t('Error'),
          message
        });
      }

      if (rethrow) {
        throw error;
      }
    } finally {
      setLoading(false);
    }
  };

  return { loading, showLoadingUI, error, execute };
}
