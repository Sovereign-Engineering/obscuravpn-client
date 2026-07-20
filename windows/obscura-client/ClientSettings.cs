using Microsoft.UI.Xaml;
using Microsoft.Windows.Storage;
using System;

namespace Obscura_Client;

static class ClientSettings
{
    const string CompletedFirstRunKey = "completedFirstRun";
    const string ColorSchemeKey = "colorScheme";
    static ApplicationDataContainer Settings => ApplicationData.GetForUnpackaged("Sovereign Engineering", "Obscura VPN").LocalSettings;

    internal static ElementTheme ColorScheme
    {
        get => Settings.Values.TryGetValue(ColorSchemeKey, out var value) && value is int theme && Enum.IsDefined((ElementTheme)theme)
            ? (ElementTheme)theme
            : ElementTheme.Default;
        set => Settings.Values[ColorSchemeKey] = (int)value;
    }

    internal static bool IsFirstRun => !Settings.Values.ContainsKey(CompletedFirstRunKey);

    public static void SetFirstRunCompleted()
    {
        Settings.Values[CompletedFirstRunKey] = true;
    }

    public static void Clear()
    {
        Settings.Values.Clear();
    }
}
