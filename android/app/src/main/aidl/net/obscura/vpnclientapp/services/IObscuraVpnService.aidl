// IObscuraVpnService.aidl
package net.obscura.vpnclientapp.services;

interface IObscuraVpnService {
    void startTunnel(String exitSelector);
    void stopTunnel();

    // Submits the command to the ObscuraLibrary.jsonFfi(String, CompletableFuture<String>)
    // function, returning back a unique ID. To receive the result of the command, listen on the
    // CommandBridge.Receiver (BroadcastReceiver) for an Intent with the "id" extra, "result" extra
    // (indicating success) or "exception" extra (indicating a JsonFfiException).
    void jsonFfi(long id, String command);
}
