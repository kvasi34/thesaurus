use clap::Parser;

// Server startup configuration
#[derive(Debug, Parser)]
#[command(name = "thesaurus")]
pub(crate) struct Cli {
    /// Server IPv4 address
    #[arg(long, default_value = "127.0.0.1", env = "THESAURUS_BIND")]
    pub bind: String,

    /// Server port
    #[arg(long, default_value_t = 6379, env = "THESAURUS_PORT")]
    pub port: u16,

    /// Max TCP socket connections
    #[arg(long, default_value_t = 100, env = "THESAURUS_MAX_CONNECTIONS")]
    pub max_connections: usize,
}

/// Command parsed from raw TCP client input.
#[derive(Debug)]
pub(crate) enum Command {
    /// Ping the server to check if it is alive
    Ping,
    /// Get the value for a key
    Get { key: String },
    /// Set a key-value pair
    Set { key: String, value: String },
    /// Delete a key-value pair
    Delete { key: String },
}

impl Command {
    /// Parse a line into `Command` enum.
    pub fn from_input(line: &str) -> Result<Self, String> {
        let parts: Vec<&str> = line.trim().splitn(3, ' ').collect();
        match parts.as_slice() {
            ["PING"] => Ok(Command::Ping),
            ["GET", key] => Ok(Command::Get {
                key: key.to_string(),
            }),
            ["SET", key, value] => Ok(Command::Set {
                key: key.to_string(),
                value: value.to_string(),
            }),
            ["DEL", key] => Ok(Command::Delete {
                key: key.to_string(),
            }),
            _ => Err(format!("Unknown command: {}", line)),
        }
    }
}
