using System;
using System.Threading;
using log4net;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml.Controls;
using WinUIEx;

namespace Obscura_Client.NotifyIcon;

/// <summary>
/// Owns the notification icon and the MenuFlyout that pops on right-click.
/// Dispose() must run before the process exits — WinUIEx.TrayIcon will otherwise
/// hang shutdown and leave a ghost icon in the notification area.
/// </summary>
public sealed partial class NotifyIconManager
{
    static readonly ILog Log = LogManager.GetLogger(typeof(NotifyIconManager));

    static readonly TimeSpan ConnectingFrameInterval = TimeSpan.FromMilliseconds(500);

    readonly App _app;
    // UI updates must be executed on the thread that created the UI
    readonly DispatcherQueue _uiQueue;
    readonly NotifyIconAssets _assets;
    readonly TrayIcon _notifyIcon;
    readonly StatusSubscriber _statusSubscriber = new();
    readonly CityNameCache _cityNames = new();

    // Connecting animation timer
    readonly DispatcherQueueTimer _animTimer;
    int _animFrame;

    VpnStatusKind? _lastKind;

    public NotifyIconManager(App app, DispatcherQueue uiQueue)
    {
        _app = app;
        _uiQueue = uiQueue;

        _assets = new NotifyIconAssets();
        _notifyIcon = new TrayIcon(trayiconId: 1, _assets.Disconnected, "Obscura VPN") { IsVisible = true };
        _notifyIcon.Selected += (_, _) => _app.ShowMainWindow();
        _notifyIcon.ContextMenu += OnContextMenu;

        _animTimer = _uiQueue.CreateTimer();
        _animTimer.Interval = ConnectingFrameInterval;
        _animTimer.IsRepeating = true;
        _animTimer.Tick += OnAnimTick;

        _statusSubscriber.StatusChanged += OnStatusChanged;
        _statusSubscriber.Start();
        _cityNames.Start();
    }

    void OnStatusChanged(NeStatus status)
    {
        var kind = status.VpnStatus.Kind;
        _uiQueue.TryEnqueue(() => ApplyStatusKind(kind));
    }

    void ApplyStatusKind(VpnStatusKind kind)
    {
        if (_lastKind == kind) return;
        _lastKind = kind;

        switch (kind)
        {
            case VpnStatusKind.Connecting:
                StartConnectingAnimation();
                break;
            case VpnStatusKind.Connected:
                StopConnectingAnimation();
                _notifyIcon.SetIcon(_assets.Connected);
                break;
            case VpnStatusKind.Disconnected:
            default:
                StopConnectingAnimation();
                _notifyIcon.SetIcon(_assets.Disconnected);
                break;
        }
    }

    void StartConnectingAnimation()
    {
        if (_animTimer.IsRunning) return;
        _animFrame = 0;
        _notifyIcon.SetIcon(_assets.Connecting[_animFrame]);
        _animTimer.Start();
    }

    void OnAnimTick(DispatcherQueueTimer sender, object args)
    {
        _animFrame = (_animFrame + 1) % _assets.Connecting.Length;
        _notifyIcon.SetIcon(_assets.Connecting[_animFrame]);
    }

    void StopConnectingAnimation()
    {
        _animTimer.Stop();
    }

    void OnContextMenu(TrayIcon sender, TrayIconEventArgs args)
    {
        args.Flyout = BuildMenu();
    }

    MenuFlyout BuildMenu()
    {
        var status = _statusSubscriber.Current;
        var kind = status?.VpnStatus.Kind ?? VpnStatusKind.Disconnected;
        var menu = new MenuFlyout();

        if (kind == VpnStatusKind.Disconnected)
        {
            var connect = new MenuFlyoutItem { Text = "Connect" };
            connect.Click += (_, _) => StartTunnel(status?.LastExit ?? ExitSelector.ForAny());
            menu.Items.Add(connect);
        }
        else
        {
            var disconnect = new MenuFlyoutItem
            {
                Text = kind == VpnStatusKind.Connecting ? "Cancel Connecting" : "Disconnect",
            };
            disconnect.Click += (_, _) => StopTunnel();
            menu.Items.Add(disconnect);
        }

        menu.Items.Add(BuildConnectViaSubmenu(status));

        menu.Items.Add(new MenuFlyoutSeparator());
        var openWindow = new MenuFlyoutItem { Text = "Open Obscura Manager..." };
        openWindow.Click += (_, _) => _app.ShowMainWindow();
        menu.Items.Add(openWindow);

        menu.Items.Add(new MenuFlyoutSeparator());
        var version = new MenuFlyoutItem { Text = OsStatus.Instance.SrcVersion, IsEnabled = false };
        menu.Items.Add(version);

        var quit = new MenuFlyoutItem { Text = "Quit and Disconnect" };
        quit.Click += (_, _) => _app.RequestQuit();
        menu.Items.Add(quit);

        return menu;
    }

    MenuFlyoutSubItem BuildConnectViaSubmenu(NeStatus? status)
    {
        var sub = new MenuFlyoutSubItem { Text = "Connect via..." };
        var lastExit = status?.LastExit ?? ExitSelector.ForAny();

        var quickConnect = new RadioMenuFlyoutItem
        {
            Text = "Quick Connect",
            GroupName = "ConnectVia",
            IsChecked = lastExit.Kind == ExitSelectorKind.Any,
        };
        quickConnect.Click += (_, _) => StartTunnel(ExitSelector.ForAny());
        sub.Items.Add(quickConnect);

        sub.Items.Add(new MenuFlyoutSeparator());
        var pinnedHeader = new MenuFlyoutItem { Text = "Pinned Locations", IsEnabled = false };
        sub.Items.Add(pinnedHeader);

        var pinned = status?.PinnedLocations ?? [];
        bool lastExitIsPinned = false;
        bool showPinnedLocationsHint = true;

        foreach (var pin in pinned)
        {
            // Hide pins the exit-list hasn't surfaced
            if (!_cityNames.ContainsOrEmpty(pin.CountryCode, pin.CityCode)) continue;
            showPinnedLocationsHint = false;

            var displayName = _cityNames.DisplayName(pin.CountryCode, pin.CityCode);
            var item = new RadioMenuFlyoutItem
            {
                Text = $"{displayName}, {pin.CountryCode.ToUpperInvariant()}",
                GroupName = "ConnectVia",
            };

            if (lastExit.Kind == ExitSelectorKind.City
                && lastExit.CountryCode == pin.CountryCode
                && lastExit.CityCode == pin.CityCode)
            {
                item.IsChecked = true;
                lastExitIsPinned = true;
            }

            var capturedCountry = pin.CountryCode;
            var capturedCity = pin.CityCode;
            item.Click += (_, _) => StartTunnel(ExitSelector.ForCity(capturedCountry, capturedCity));
            sub.Items.Add(item);
        }

        if (showPinnedLocationsHint)
        {
            sub.Items.Add(new MenuFlyoutItem
            {
                Text = "Pinned locations will appear here",
                IsEnabled = false,
                FontStyle = Windows.UI.Text.FontStyle.Italic,
            });
        }

        // Show currently selected city if it's not in the pin list
        if (lastExit.Kind == ExitSelectorKind.City && !lastExitIsPinned
            && !string.IsNullOrEmpty(lastExit.CountryCode) && !string.IsNullOrEmpty(lastExit.CityCode))
        {
            sub.Items.Add(new MenuFlyoutSeparator());
            sub.Items.Add(new MenuFlyoutItem { Text = "Current Selection", IsEnabled = false });

            var displayName = _cityNames.DisplayName(lastExit.CountryCode!, lastExit.CityCode!);
            var current = new RadioMenuFlyoutItem
            {
                Text = $"{displayName}, {lastExit.CountryCode!.ToUpperInvariant()}",
                GroupName = "ConnectVia",
                IsChecked = true,
            };
            var capturedCountry = lastExit.CountryCode!;
            var capturedCity = lastExit.CityCode!;
            current.Click += (_, _) => StartTunnel(ExitSelector.ForCity(capturedCountry, capturedCity));
            sub.Items.Add(current);
        }

        sub.Items.Add(new MenuFlyoutSeparator());
        var more = new MenuFlyoutItem { Text = "More Locations..." };
        more.Click += (_, _) =>
        {
            _app.ShowMainWindow();
            _app.SelectNavigationView(NavigationView.Location);
        };
        sub.Items.Add(more);

        return sub;
    }

    static async void StartTunnel(ExitSelector exit)
    {
        var args = new SetTunnelArgs { Args = new TunnelArgs { Exit = exit }, Active = true };
        using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(5));
        try { await IPCCommand.RunWithArgAsync(args, cts.Token); }
        catch (Exception ex) { Log.Error($"StartTunnel failed: {ex}"); }
    }

    static async void StopTunnel()
    {
        try { await new StopTunnelCommand { TimeoutMs = 5000 }.RunAsync(); }
        catch (Exception ex) { Log.Error($"StopTunnel failed: {ex}"); }
    }
}
