using System;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Obscura_Client;

public sealed class NeStatus
{
    public required string Version { get; set; }
    public required VpnStatusEnvelope VpnStatus { get; set; }
    public string? AccountId { get; set; }
    public required bool InNewAccountFlow { get; set; }
    public required PinnedLocation[] PinnedLocations { get; set; }
    public required ExitSelector LastExit { get; set; }
    public required AccountStatus? Account { get; set; }
    public required bool AutoConnect { get; set; }
}

public sealed class VpnStatusEnvelope
{
    public JsonElement? Disconnected { get; set; }
    public JsonElement? Connecting { get; set; }
    public JsonElement? Connected { get; set; }

    [JsonIgnore]
    public VpnStatusKind Kind =>
        Connected.HasValue ? VpnStatusKind.Connected :
        Connecting.HasValue ? VpnStatusKind.Connecting :
        VpnStatusKind.Disconnected;

    [JsonIgnore]
    public string? ConnectError =>
        Connecting is { } connecting && connecting.TryGetProperty("connectError", out var error) ? error.GetString() : null;
}

// Expiry logic mirrors apple/shared/Account.swift
public sealed class AccountStatus
{
    [JsonPropertyName("account_info")]
    public required AccountInfo AccountInfo { get; set; }
    [JsonPropertyName("last_updated_sec")]
    public required long LastUpdatedSec { get; set; }

    [JsonIgnore]
    public bool IsActive => AccountInfo.Active;

    // When the subscription renews or the account expires
    [JsonIgnore]
    public long? PeriodEnd => AccountInfo.CurrentExpiry ?? AccountInfo.AutoRenews;

    // null while the subscription auto-renews
    public int? DaysUntilExpiry()
    {
        if (!IsActive) return 0;
        if (AccountInfo.AutoRenews != null) return null;
        var remaining = DateTimeOffset.FromUnixTimeSeconds(AccountInfo.CurrentExpiry ?? 0) - DateTimeOffset.UtcNow;
        return Math.Max(remaining.Days, 0);
    }

    public bool ExpiringSoon() => DaysUntilExpiry() <= 10;

    // Same schedule as the macOS status menu
    public bool NeedsRefresh()
    {
        var now = DateTimeOffset.UtcNow;
        var age = now - DateTimeOffset.FromUnixTimeSeconds(LastUpdatedSec);
        var periodEnded = DateTimeOffset.FromUnixTimeSeconds(PeriodEnd ?? 0) < now;
        return (periodEnded && age > TimeSpan.FromMinutes(5)) || (ExpiringSoon() && age > TimeSpan.FromHours(12));
    }
}

public sealed class AccountInfo
{
    public required bool Active { get; set; }
    [JsonPropertyName("auto_renews")]
    public long? AutoRenews { get; set; }
    [JsonPropertyName("current_expiry")]
    public long? CurrentExpiry { get; set; }
}

public enum VpnStatusKind
{
    Disconnected,
    Connecting,
    Connected,
}

public sealed class PinnedLocation
{
    [JsonPropertyName("country_code")]
    public required string CountryCode { get; set; }
    [JsonPropertyName("city_code")]
    public required string CityCode { get; set; }
}

public sealed class ExitSelector
{
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public AnyData? Any { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public ExitData? Exit { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public CountryData? Country { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public CityData? City { get; set; }

    [JsonIgnore]
    public ExitSelectorKind Kind =>
        Exit != null ? ExitSelectorKind.Exit :
        City != null ? ExitSelectorKind.City :
        Country != null ? ExitSelectorKind.Country :
        ExitSelectorKind.Any;
    [JsonIgnore]
    public string? ExitId => Exit?.Id;
    [JsonIgnore]
    public string? CountryCode => City?.CountryCode ?? Country?.CountryCode;
    [JsonIgnore]
    public string? CityCode => City?.CityCode;

    public override string ToString() => Kind switch
    {
        ExitSelectorKind.Exit => $"exit {ExitId}",
        ExitSelectorKind.City => $"city {CountryCode}/{CityCode}",
        ExitSelectorKind.Country => $"country {CountryCode}",
        _ => "any",
    };

    public static ExitSelector ForAny() => new() { Any = new AnyData() };
    public static ExitSelector ForCity(string countryCode, string cityCode) =>
        new() { City = new CityData { CountryCode = countryCode, CityCode = cityCode } };
}

public sealed class AnyData { }

public sealed class ExitData
{
    public required string Id { get; set; }
}

public sealed class CountryData
{
    [JsonPropertyName("country_code")]
    public required string CountryCode { get; set; }
}

public sealed class CityData
{
    [JsonPropertyName("country_code")]
    public required string CountryCode { get; set; }
    [JsonPropertyName("city_code")]
    public required string CityCode { get; set; }
}

public enum ExitSelectorKind
{
    Any,
    Exit,
    Country,
    City,
}
