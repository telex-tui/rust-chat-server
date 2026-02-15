/// Server configuration — too many optional fields for a simple constructor.
/// Builder pattern: chain method calls, validate at build time.
pub struct ServerConfig {
    pub addr: String,
    pub port: u16,
    pub max_users: usize,
    pub max_rooms: usize,
    pub motd: Option<String>,
}

/// The builder accumulates optional values and produces a validated config.
pub struct ServerConfigBuilder {
    addr: String,
    port: u16,
    max_users: usize,
    max_rooms: usize,
    motd: Option<String>,
}

impl ServerConfig {
    pub fn builder() -> ServerConfigBuilder {
        ServerConfigBuilder {
            addr: "127.0.0.1".to_string(),
            port: 8080,
            max_users: 100,
            max_rooms: 50,
            motd: None,
        }
    }
}

impl ServerConfigBuilder {
    pub fn addr(mut self, addr: impl Into<String>) -> Self {
        self.addr = addr.into();
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn max_users(mut self, max: usize) -> Self {
        self.max_users = max;
        self
    }

    pub fn max_rooms(mut self, max: usize) -> Self {
        self.max_rooms = max;
        self
    }

    pub fn motd(mut self, motd: impl Into<String>) -> Self {
        self.motd = Some(motd.into());
        self
    }

    pub fn build(self) -> ServerConfig {
        ServerConfig {
            addr: self.addr,
            port: self.port,
            max_users: self.max_users,
            max_rooms: self.max_rooms,
            motd: self.motd,
        }
    }
}
