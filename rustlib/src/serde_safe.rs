#[derive(serde::Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum TryParse<T> {
    Valid(T),
    Invalid(serde_json::Value),
}

pub fn deserialize<'de, T: Default + serde::Deserialize<'de>, D: serde::Deserializer<'de>>(deserializer: D) -> Result<T, D::Error> {
    Ok(match serde::Deserialize::deserialize(deserializer)? {
        TryParse::Valid(v) => v,
        TryParse::Invalid(json) => {
            tracing::error!(?json, "Deserialization invalid, using default");
            Default::default()
        }
    })
}
