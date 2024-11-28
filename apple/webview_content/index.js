async function runCommand(command) {
    let command_json = JSON.stringify(command);
    let result_json = await window.webkit.messageHandlers.commandBridge.postMessage(command_json);
    return JSON.parse(result_json);
}

async function setAccountId() {
    var id = document.getElementById("accountId").value;
    if (id == "") id = null;
    await runCommand({ setAccountId: { accountId: id }});
}

async function startTunnel() {
    await runCommand({ vpn: { enable: true }})
}
async function stopTunnel() {
    await runCommand({ vpn: { enable: false }})
}

async function updateStatusLoop() {
    var version = null
    while (true) {
        let jsonCmd = JSON.stringify({ getStatus: { knownVersion: version}});
        let status = await await runCommand({ jsonFfiCmd: {cmd: jsonCmd}});
        let pre = document.createElement("pre");
        pre.textContent = JSON.stringify(status, null, 2);
        let log = document.getElementById("status");
        log.prepend(pre)
        version = status.version
    }
}
updateStatusLoop()

async function runJsonFfiCmd(cmd) {
    let jsonCmd = JSON.stringify(cmd);
    let retPre = document.getElementById("jsonCmdRet");
    try {
        retPre.textContent = "";
        let ret = await runCommand({ jsonFfiCmd: {cmd: jsonCmd}})
        retPre.style.color = "green"
        retPre.textContent = JSON.stringify(ret, null, 2);
    } catch (err) {
        retPre.textContent = err;
        retPre.style.color = "red"
    }
}

async function getTrafficStats() {
    runJsonFfiCmd({ "getTrafficStats": {} })
}

async function setExit() {
    var exit = document.getElementById("exit").value;
    exit = exit === '' ? null : exit;
    runJsonFfiCmd({ "setExit": { exit: exit } })
}

async function apiListExit() {
    runJsonFfiCmd({ "apiListExit": {} })
}
