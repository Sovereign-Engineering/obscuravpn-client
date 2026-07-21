using Microsoft.Windows.AppLifecycle;
using Microsoft.Windows.AppNotifications;
using Windows.ApplicationModel.Activation;

namespace Obscura_Client;

internal static class NotificationActions
{
    internal const string ArgumentKey = "action";
    internal const string ShowUpdate = "showUpdate";

    // Windows 10 reports notification activations as ToastNotification rather than AppNotification.
    internal static bool IsNotificationKind(ExtendedActivationKind kind) =>
        kind is ExtendedActivationKind.AppNotification or ExtendedActivationKind.ToastNotification;

    internal static string? GetAction(object? activationData) => activationData switch
    {
        AppNotificationActivatedEventArgs args =>
            args.Arguments.TryGetValue(ArgumentKey, out var action) ? action : null,
        IToastNotificationActivatedEventArgs args => ParseAction(args.Argument),
        _ => null,
    };

    // Toast activation carries the raw "key=value;key=value" string that
    // AppNotificationBuilder serialized instead of a parsed dictionary.
    static string? ParseAction(string? argument)
    {
        if (string.IsNullOrEmpty(argument))
        {
            return null;
        }
        foreach (var pair in argument.Split(';'))
        {
            var separator = pair.IndexOf('=');
            var key = separator < 0 ? pair : pair[..separator];
            if (key == ArgumentKey)
            {
                return separator < 0 ? "" : pair[(separator + 1)..];
            }
        }
        return null;
    }
}
