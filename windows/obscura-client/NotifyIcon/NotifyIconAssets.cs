using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.IO;
using System.Runtime.InteropServices;
using Microsoft.UI;
using Windows.Win32;
using Windows.Win32.Foundation;
using Windows.Win32.UI.WindowsAndMessaging;

namespace Obscura_Client.NotifyIcon;

/// <summary>
/// Loads tray .ico files into HICONs once and exposes them as <see cref="IconId"/>s
/// suitable for repeated <c>TrayIcon.SetIcon</c> calls. WinUIEx's path-based SetIcon
/// is unstable when called frequently; reusing pre-loaded IconIds avoids that path.
/// </summary>
internal sealed partial class NotifyIconAssets
{
    readonly List<HICON> _hicons = [];
    public IconId Disconnected { get; }
    public IconId Connected { get; }
    public IconId[] Connecting { get; }

    public NotifyIconAssets()
    {
        Disconnected = Load("Disconnected.ico");
        Connected = Load("Connected.ico");
        Connecting =
        [
            Load("Connecting-1.ico"),
            Load("Connecting-2.ico"),
            Load("Connecting-3.ico"),
        ];
    }

    unsafe IconId Load(string filename)
    {
        var path = Path.Combine(AppContext.BaseDirectory, "Assets", "Tray", filename);
        HANDLE handle;
        fixed (char* p = path)
        {
            handle = PInvoke.LoadImage(
                hInst: (HINSTANCE)IntPtr.Zero,
                name: p,
                type: GDI_IMAGE_TYPE.IMAGE_ICON,
                cx: 0,
                cy: 0,
                fuLoad: IMAGE_FLAGS.LR_LOADFROMFILE | IMAGE_FLAGS.LR_DEFAULTSIZE);
        }
        if (handle == IntPtr.Zero)
            throw new Win32Exception(Marshal.GetLastWin32Error(), $"LoadImage failed for {path}");

        var hicon = (HICON)handle.Value;
        _hicons.Add(hicon);
        return Win32Interop.GetIconIdFromIcon((IntPtr)hicon.Value);
    }
}
