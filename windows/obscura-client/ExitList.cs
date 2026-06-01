using System.Text.Json.Serialization;

namespace Obscura_Client;

public sealed class ExitListEnvelope
{
    [JsonPropertyName("version")]
    public required string Version { get; set; }
    [JsonPropertyName("last_updated")]
    public required double LastUpdated { get; set; }
    [JsonPropertyName("value")]
    public required ExitListBody Value { get; set; }
}

public sealed class ExitListBody
{
    public required OneExit[] Exits { get; set; }
}

public sealed class OneExit
{
    [JsonPropertyName("city_code")]
    public required string CityCode { get; set; }
    [JsonPropertyName("country_code")]
    public required string CountryCode { get; set; }
    [JsonPropertyName("city_name")]
    public required string CityName { get; set; }
}
