using log4net;
using Microsoft.Windows.AppLifecycle;
using System.Collections.Generic;
using System.Text.Json;
using Windows.Win32;
using Windows.Win32.Foundation;
using Windows.Win32.System.DataExchange;
using Windows.Win32.UI.WindowsAndMessaging;

namespace Obscura_Client;

/// <summary>
/// WM_COPYDATA activation envelope and hand-off protocol. The WinRT activation Data objects
/// cannot be reconstructed in another process, so the secondary instance ships a JSON snapshot
/// of their contents (ActivatePrimary) and the primary re-dispatches from it
/// (MainWindow's WndProc via TryReadPayload).
/// </summary>
internal sealed class FallbackActivation
{
    public ExtendedActivationKind Kind { get; set; }
    // Kind == Protocol
    public string? ProtocolUri { get; set; }
    // Kind == AppNotification / ToastNotification
    public Dictionary<string, string>? NotificationArguments { get; set; }

    static readonly ILog Log = LogManager.GetLogger(typeof(FallbackActivation));

    // "OBS"; distinguishes our activation hand-off from other WM_COPYDATA traffic
    const nuint OBS_ACTIVATION_TAG = 0x4F4253;

    /// <summary>
    /// Activates primary instance using WM_COPYDATA message.
    /// RedirectActivationToAsync marshals IAppActivationArguments via WinRT metadata resolution,
    /// which fails (0x80040155) on Windows 10 for self-contained deployments with sparse package
    /// identity: https://github.com/microsoft/WindowsAppSDK/issues/3439#issuecomment-4970200486.
    /// Primary instance's MainWindow replies 1 when it accepts.
    /// </summary>
    internal static void ActivatePrimary(AppActivationArguments activationArgs, AppInstance keyInstance)
    {
        var activation = new FallbackActivation { Kind = activationArgs.Kind };
        if (activationArgs.Data is Windows.ApplicationModel.Activation.IProtocolActivatedEventArgs protocolArgs)
        {
            activation.ProtocolUri = protocolArgs.Uri.ToString();
        }
        else if (NotificationActions.GetAction(activationArgs.Data) is { } action)
        {
            activation.NotificationArguments = new Dictionary<string, string>
            {
                [NotificationActions.ArgumentKey] = action,
            };
        }
        var payload = JsonSerializer.Serialize(activation);

        var candidates = new List<HWND>();
        PInvoke.EnumWindows((hwnd, _) =>
        {
            if (GetWindowPid(hwnd) == keyInstance.ProcessId)
            {
                candidates.Add(hwnd);
            }
            return true;
        }, 0);

        PInvoke.AllowSetForegroundWindow(keyInstance.ProcessId);
        foreach (var hwnd in candidates)
        {
            if (SendActivationPayload(hwnd, payload))
            {
                Log.Info("Activated primary instance via WM_COPYDATA fallback");
                return;
            }
        }
        Log.Error($"WM_COPYDATA fallback not accepted by any of {candidates.Count} windows of pid {keyInstance.ProcessId}");
    }

    /// <summary>
    /// Isolates call to unsafe method GetWindowThreadProcessId
    /// </summary>
    private static unsafe uint GetWindowPid(HWND hwnd)
    {
        uint pid;
        // SAFETY: &pid is not null; points to the stack-allocated variable pid
        var _ = PInvoke.GetWindowThreadProcessId(hwnd, &pid);
        return pid;
    }

    /// <summary>
    /// Isolates WM_COPYDATA marshaling: building a COPYDATASTRUCT requires
    /// pinning the payload string and passing raw addresses.
    /// </summary>
    private static unsafe bool SendActivationPayload(HWND hwnd, string payload)
    {
        fixed (char* payloadPtr = payload)
        {
            var copyData = new COPYDATASTRUCT
            {
                dwData = OBS_ACTIVATION_TAG,
                cbData = (uint)(payload.Length * sizeof(char)),
                lpData = payloadPtr,
            };
            nuint result = 0;
            // SAFETY: SendMessageTimeout is synchronous, so the fixed pin and the stack-allocated
            // copyData/result outlive the call; the OS copies the buffer into the receiving
            // process, so nothing is referenced after return.
            PInvoke.SendMessageTimeout(hwnd, PInvoke.WM_COPYDATA, 0, (nint)(&copyData),
                SEND_MESSAGE_TIMEOUT_FLAGS.SMTO_ABORTIFHUNG, 5000, &result);
            return result == 1;
        }
    }

    /// <summary>
    /// Exists to isolate reading the sender's COPYDATASTRUCT out of a raw WM_COPYDATA lParam;
    /// returns false if the message is not our activation hand-off.
    /// SAFETY: for WM_COPYDATA, the OS maps the sender's COPYDATASTRUCT and its lpData buffer
    /// (cbData bytes) into this process and keeps them valid while the message is being
    /// handled; the read stays within cbData and the string constructor copies, so no
    /// pointer escapes the handler.
    /// </summary>
    internal static unsafe bool TryReadPayload(nint lParam, out string payload)
    {
        payload = "";
        var copyData = (COPYDATASTRUCT*)lParam;
        if (copyData == null || copyData->dwData != OBS_ACTIVATION_TAG)
        {
            return false;
        }
        if (copyData->cbData != 0)
        {
            payload = new string((char*)copyData->lpData, 0, (int)(copyData->cbData / sizeof(char)));
        }
        return true;
    }
}
