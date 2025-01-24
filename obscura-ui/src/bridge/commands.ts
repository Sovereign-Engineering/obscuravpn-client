import { AccountId } from '../common/accountUtils';
import { AccountInfo, Exit } from '../common/api';
import { AppStatus, NEVPNStatus, OsStatus, PinnedLocation } from '../common/appContext';
import { normalizeError } from '../common/utils';

async function WKWebViewInvoke(command: string, args: Object) {
    const commandJson = JSON.stringify({ [command]: args });
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
        console.error(`Command ${command}(${JSON.stringify(args)}) resulted in error: ${e}`);
        // rethrow error
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

export function readContents(path: string) {
    return invoke('read_contents', { path });
}

export function showItemInFolder(path: string) {
    return invoke('show_item_in_folder', { path });
}

export function checkForUpdate() {
    return invoke('check_for_update');
}

export function installUpdate(newVersion: string) {
    return invoke('install_update', { newVersion });
}

// VPN Client Specific Commands

export async function jsonFfiCmd(cmd: string, arg = {}, timeoutMs: number | null = 10_000): Promise<unknown> {
    let jsonCmd = JSON.stringify(({ [cmd]: arg }));
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

export function connect(exit: string | null = null) {
    console.log(`got exit: ${JSON.stringify(exit)}`);
    let jsonTunnelArgs = JSON.stringify(({ exit }));

    return invoke('startTunnel', { tunnelArgs: jsonTunnelArgs });
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
            console.error(`failed to get status in disconnectThenConnect ${e}`)
        }
    }
    // NEVPNStatus
    knownStatusId = null;
    while (true) {
      try {
          const s = await osStatus(knownStatusId);
          if (s.osVpnStatus === NEVPNStatus.disconnected) break;
          knownStatusId = s.version;
      } catch (e) {
          console.error(`failed to get osStatus in disconnectThenConnect ${e}`)
      }
  }
}

export function debuggingArchive() {
    return invoke('debuggingArchive');
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

export function registerLoginItem() {
    return invoke('registerLoginItem');
}


export function developerResetUserDefaults() {
    return invoke('resetUserDefaults');
}

export interface TrafficStats {
    timestampMs: number,
    connId: string,
    txBytes: number,
    rxBytes: number,
    latestLatencyMs: number,
}

export async function getTrafficStats(): Promise<TrafficStats> {
    return await jsonFfiCmd('getTrafficStats') as TrafficStats;
}


export interface ApiListExitResponse {
    exits: Exit[]
}

export async function getExitServers(): Promise<Exit[]> {
    const { exits } = await jsonFfiCmd('apiListExit') as ApiListExitResponse;
    return exits;
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
