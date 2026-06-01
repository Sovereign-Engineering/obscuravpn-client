using System.Text.Json;
using System.Text.Json.Serialization;

namespace Obscura_Client;

public sealed class NeStatus
{
    public required string Version { get; set; }
    public required VpnStatusEnvelope VpnStatus { get; set; }
    public required bool InNewAccountFlow { get; set; }
    public required PinnedLocation[] PinnedLocations { get; set; }
    public required ExitSelector LastExit { get; set; }
    public required JsonElement Account { get; set; }
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
