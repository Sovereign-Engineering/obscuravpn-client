using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Obscura_Client.NotifyIcon;

/// <summary>
/// Menu item with a caption line under its text, like the macOS account status item. Default style in Themes/Generic.xaml.
/// </summary>
public sealed partial class AccountMenuFlyoutItem : MenuFlyoutItem
{
    public static readonly DependencyProperty SubtitleProperty =
        DependencyProperty.Register(nameof(Subtitle), typeof(string), typeof(AccountMenuFlyoutItem), new PropertyMetadata(""));

    public static readonly DependencyProperty IsExpiredProperty =
        DependencyProperty.Register(nameof(IsExpired), typeof(bool), typeof(AccountMenuFlyoutItem), new PropertyMetadata(false, OnIsExpiredChanged));

    public AccountMenuFlyoutItem()
    {
        DefaultStyleKey = typeof(AccountMenuFlyoutItem);
    }

    public string Subtitle
    {
        get => (string)GetValue(SubtitleProperty);
        set => SetValue(SubtitleProperty, value);
    }

    /// <summary>Shows the subtitle in the critical color.</summary>
    public bool IsExpired
    {
        get => (bool)GetValue(IsExpiredProperty);
        set => SetValue(IsExpiredProperty, value);
    }

    protected override void OnApplyTemplate()
    {
        base.OnApplyTemplate();
        UpdateExpiryState();
    }

    static void OnIsExpiredChanged(DependencyObject d, DependencyPropertyChangedEventArgs e) => ((AccountMenuFlyoutItem)d).UpdateExpiryState();

    void UpdateExpiryState() => VisualStateManager.GoToState(this, IsExpired ? "Expired" : "Active", false);
}
