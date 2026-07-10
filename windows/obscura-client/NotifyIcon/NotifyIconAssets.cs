using System;
using System.ComponentModel;
using System.IO;
using System.Runtime.InteropServices;
using Microsoft.UI;
using Windows.Win32;
using Windows.Win32.Foundation;
using Windows.Win32.UI.WindowsAndMessaging;

namespace Obscura_Client.NotifyIcon;

internal sealed partial class NotifyIconAssets
{
    internal sealed class IconSet
    {
        public required IconId Disconnected { get; init; }
        public required IconId Connected { get; init; }
        public required IconId[] Connecting { get; init; }
    }
    public IconSet OnDarkTaskbar { get; }
    public IconSet OnLightTaskbar { get; }

    public NotifyIconAssets()
    {
        OnDarkTaskbar = LoadSet("-light");
        OnLightTaskbar = LoadSet("-dark");
    }

    public IconSet For(TaskbarThemeKind theme) => theme == TaskbarThemeKind.Light ? OnLightTaskbar : OnDarkTaskbar;

    IconSet LoadSet(string suffix) => new()
    {
        Disconnected = Load($"Disconnected{suffix}.ico"),
        Connected = Load($"Connected{suffix}.ico"),
        Connecting =
        [
            Load($"Connecting-1{suffix}.ico"),
            Load($"Connecting-2{suffix}.ico"),
            Load($"Connecting-3{suffix}.ico"),
        ],
    };

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

        return Win32Interop.GetIconIdFromIcon((IntPtr)handle.Value);
    }
}
