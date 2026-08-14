using System.Text.Json;
using System.Text.Json.Serialization;

namespace Obscura_Client;

public static class JsonConfig
{
    public static readonly JsonSerializerOptions Options = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        Converters = { new JsonStringEnumConverter(JsonNamingPolicy.CamelCase) },
    };

    // Property names exactly as declared
    public static readonly JsonSerializerOptions BundleInfoOptions = new()
    {
        PropertyNamingPolicy = null,
        Converters = { new JsonStringEnumConverter() },
        WriteIndented = true,
    };
}
