pub fn release_version() -> &'static str {
    option_env!("OBSCURA_VERSION").unwrap_or("v0.0.0-dev")
}
