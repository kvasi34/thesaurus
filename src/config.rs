use log::error;

/// Server behaviour configuration, loaded from an INI file.
///
/// Controls resource limits and background task tuning. Connection and
/// network settings (bind address, port) are handled separately by [`command::Cli`].
#[derive(Debug)]
pub(crate) struct ThesaurusConfig {
    pub max_connections: usize,
    pub hz: u64,
}

impl Default for ThesaurusConfig {
    fn default() -> Self {
        ThesaurusConfig {
            max_connections: 100,
            hz: 100,
        }
    }
}

/// Loads configuration from an INI file at `path`, with environment variable overrides.
///
/// Environment variables prefixed with `THESAURUS_` take precedence over file values.
/// Returns an error if the path does not point to an `.ini` file or if parsing fails.
pub(crate) fn load_config(path: &str) -> Result<ThesaurusConfig, String> {
    if !path.ends_with(".ini") {
        error!("Failed to read INI file at `{}`", path);
        return Err(format!("Expected an .ini file, got: {}", path));
    }
    let config = config::Config::builder()
        .set_default("max_connections", 100)
        .and_then(|b| b.set_default("hz", 100))
        .map_err(|e| e.to_string())?
        .add_source(config::File::with_name(path))
        .add_source(config::Environment::with_prefix("THESAURUS"))
        .build()
        .map_err(|e| e.to_string())?;

    Ok(ThesaurusConfig {
        max_connections: config
            .get::<usize>("max_connections")
            .map_err(|e| e.to_string())?,
        hz: config.get::<u64>("hz").map_err(|e| e.to_string())?,
    })
}
