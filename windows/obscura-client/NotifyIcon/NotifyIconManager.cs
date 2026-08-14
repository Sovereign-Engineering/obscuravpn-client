using System;
using System.Threading;
using log4net;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml.Controls;
using WinUIEx;

namespace Obscura_Client.NotifyIcon;

/// <summary>
/// Owns the notification icon and the MenuFlyout that pops on right-click.
/// Close must be called before the process exits to avoid a ghost icon in the notification status area.
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
    readonly CityNameCache _cityNames = new();
    readonly TaskbarTheme _taskbarTheme = new();
    NotifyIconAssets.IconSet _icons;
    TaskbarThemeKind _taskbarThemeKind;
    bool _closed;

    // Connecting animation timer
    readonly DispatcherQueueTimer _animTimer;
    int _animFrame;

    VpnStatusKind? _lastKind;

    public NotifyIconManager(App app, DispatcherQueue uiQueue)
    {
        _app = app;
        _uiQueue = uiQueue;

        _assets = new NotifyIconAssets();
        _taskbarTheme.Changed += OnTaskbarThemeChanged;
        _taskbarThemeKind = TaskbarTheme.Current;
        _icons = _assets.For(_taskbarThemeKind);

        _notifyIcon = new TrayIcon(trayiconId: 1, _icons.Disconnected, "Obscura VPN") { IsVisible = true };
        _notifyIcon.Selected += (_, _) => _app.ShowMainWindow();
        _notifyIcon.ContextMenu += OnContextMenu;
        Log.Info($"notify icon created (taskbar theme: {_taskbarThemeKind})");

        _animTimer = _uiQueue.CreateTimer();
        _animTimer.Interval = ConnectingFrameInterval;
        _animTimer.IsRepeating = true;
        _animTimer.Tick += OnAnimTick;

        StatusSubscriber.Instance.StatusChanged += OnStatusChanged;
        if (StatusSubscriber.Instance.Current is { } current) OnStatusChanged(current);
        _cityNames.Start();
    }

    public void EnsureVisible()
    {
        _uiQueue.TryEnqueue(() =>
        {
            if (_closed) return;
            Log.Info("ensuring notify icon is present");
            _notifyIcon.IsVisible = true;
            RefreshIcon();
        });
    }

    // Explicitly clean up TrayIcon to prevent ghosting icon
    public void Close()
    {
        if (_closed) return;
        _closed = true;
        Log.Info("closing notify icon");
        _taskbarTheme.Changed -= OnTaskbarThemeChanged;
        StatusSubscriber.Instance.StatusChanged -= OnStatusChanged;
        _animTimer.Stop();
        _notifyIcon.Dispose();
    }

    void OnStatusChanged(NeStatus status)
    {
        var kind = status.VpnStatus.Kind;
        _uiQueue.TryEnqueue(() => ApplyStatusKind(kind));
    }

    void OnTaskbarThemeChanged()
    {
        _uiQueue.TryEnqueue(() =>
        {
            var theme = TaskbarTheme.Current;
            if (theme == _taskbarThemeKind) return;
            Log.Info($"taskbar theme changed: {theme}");
            _taskbarThemeKind = theme;
            _icons = _assets.For(theme);
            RefreshIcon();
        });
    }

    void RefreshIcon()
    {
        _notifyIcon.SetIcon(_lastKind switch
        {
            VpnStatusKind.Connecting => _icons.Connecting[_animFrame],
            VpnStatusKind.Connected => _icons.Connected,
            _ => _icons.Disconnected,
        });
    }

    void ApplyStatusKind(VpnStatusKind kind)
    {
        if (_lastKind == kind) return;
        Log.Info($"notify icon status: {_lastKind?.ToString() ?? "none"} -> {kind}");
        _lastKind = kind;

        if (kind == VpnStatusKind.Connecting)
        {
            StartConnectingAnimation();
        }
        else
        {
            StopConnectingAnimation();
        }
        RefreshIcon();
    }

    void StartConnectingAnimation()
    {
        _animFrame = 0;
        _animTimer.Start();
    }

    void OnAnimTick(DispatcherQueueTimer sender, object args)
    {
        _animFrame = (_animFrame + 1) % _icons.Connecting.Length;
        RefreshIcon();
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
        var status = StatusSubscriber.Instance.Current;
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

        var checkForUpdates = new MenuFlyoutItem { Text = "Check for Updates..." };
        checkForUpdates.Click += (_, _) => _ = _app.Updater.CheckAndPromptAsync();
        menu.Items.Add(checkForUpdates);

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
