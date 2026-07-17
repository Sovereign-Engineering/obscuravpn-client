using Microsoft.Windows.Storage;

namespace Obscura_Client;

static class ClientSettings
{
    const string CompletedFirstRunKey = "completedFirstRun";
    static ApplicationDataContainer Settings => ApplicationData.GetForUnpackaged("Sovereign Engineering", "Obscura VPN").LocalSettings;

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
