async function WKWebViewInvoke(command, args) {
    const commandJson = JSON.stringify({ [command]: args });
    let resultJson;
    try {
        resultJson = await window.webkit.messageHandlers.commandBridge.postMessage(commandJson);
    } catch (e) {
        throw new CommandError(e.message);
    }
    return JSON.parse(resultJson);
}

async function invoke(command, args) {
    // all commands are logged for wkwebview according to ContentView.swift
    if (args === undefined) args = {};
    try {
        return await WKWebViewInvoke(command, args);
    } catch (e) {
        console.error(`Command ${command}(${JSON.stringify(args)}) resulted in error: ${e}`);
        // rethrow error
        throw e;
    }
}

export class CommandError extends Error {
    constructor(code) {
        // HACK: We should put some "human readable" message into the message field but lots of code currently just hopes to find specific error codes in the message field. So until we hunt down all of those just put the code in the message as well. Don't write new code that treats `message` as machine readable.
        super(code);
        this.code = code;
    }

    i18nKey() {
        return `ipcError-${this.code}`;
    }
}

export function readContents(path) {
    return invoke('read_contents', { path }, false);
}

export function showItemInFolder(path) {
    return invoke('show_item_in_folder', { path });
}

export function checkForUpdate() {
    return invoke('check_for_update');
}

export function installUpdate(newVersion) {
    return invoke('install_update', { newVersion });
}

// VPN Client Specific Commands

export async function jsonFfiCmd(cmd, arg = {}, timeoutMs=10_000) {
    let jsonCmd = JSON.stringify(({ [cmd]: arg }));
    return await invoke('jsonFfiCmd', {
        cmd: jsonCmd,
        timeoutMs,
    })
}

export async function status(lastStatusId = null) {
    return await jsonFfiCmd(
        'getStatus',
        { knownVersion: lastStatusId },
        null,
    );
}

export async function osStatus(lastOsStatusId = null) {
    return await invoke('getOsStatus', { knownVersion: lastOsStatusId });
}

export function login(accountId, validate = false) {
    return jsonFfiCmd('login', { accountId, validate });
}

export function logout() {
    return jsonFfiCmd('logout');
}

export function connect(exit = null) {
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
    while (true) {
        try {
            const s = await status(knownStatusId);
            if (s.vpnStatus.disconnected !== undefined) break;
            knownStatusId = s.version;
        } catch (e) {
            console.error(`failed to get status in disconnectThenConnect ${e}`)
        }
    }
}

export function debuggingArchive() {
    return invoke('debuggingArchive');
}

export async function notices() {
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

export function getTrafficStats() {
    return jsonFfiCmd('getTrafficStats');
}

export function getExitServers() {
    return jsonFfiCmd('apiListExit');
}

export function getAccount() {
    /* see obscuravpn-api/src/types.rs:AccountInfo */
    return jsonFfiCmd('apiGetAccountInfo');
}

export function setInNewAccountFlow(value) {
    return jsonFfiCmd('setInNewAccountFlow', { value });
}

export function setPinnedExits(newPinnedExits) {
    return jsonFfiCmd('setPinnedExits', { exits: newPinnedExits });
}
