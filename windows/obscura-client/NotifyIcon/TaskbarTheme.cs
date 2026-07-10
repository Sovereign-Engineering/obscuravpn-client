using System;
using System.Runtime.Versioning;
using Microsoft.Win32;
using Windows.UI.ViewManagement;

namespace Obscura_Client.NotifyIcon;

internal enum TaskbarThemeKind
{
    Dark,
    Light,
}

internal class TaskbarTheme
{
    readonly UISettings _uiSettings = new();

    public TaskbarTheme()
    {
        _uiSettings.ColorValuesChanged += (_, _) => Changed?.Invoke();
    }

    public event Action? Changed;

    // No documented API for getting current taskbar system theme, read registry instead.
    [SupportedOSPlatform("windows")]
    public static TaskbarThemeKind Current
    {
        get
        {
            using var key = Registry.CurrentUser.OpenSubKey(
                @"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
            return key?.GetValue("SystemUsesLightTheme") is int value && value != 0
                ? TaskbarThemeKind.Light
                : TaskbarThemeKind.Dark;
        }
    }
}
