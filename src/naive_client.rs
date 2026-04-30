//! NaiveClient - high-level Cronet-based HTTP/2 and QUIC client.
//!
//! Maps to cronet-go's naive_client.go. Manages the Cronet engine lifecycle,
//! DNS resolution, connection pooling, and provides a dialer interface.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::bidirectional::{BidirectionalConn, BidirectionalStreamEngine};
use crate::engine::Engine;
use crate::engine_params::EngineParamsBuilder;
use crate::error::CronetError;
use crate::naive_conn::NaiveConn;

/// Congestion control algorithm for QUIC connections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuicCongestionControl {
    Default,
    Bbr,
    BbrV2,
    Cubic,
    Reno,
}

impl QuicCongestionControl {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "",
            Self::Bbr => "TBBR",
            Self::BbrV2 => "B2ON",
            Self::Cubic => "QBIC",
            Self::Reno => "RENO",
        }
    }
}

/// Configuration options for the NaiveClient.
#[derive(Clone, Debug)]
pub struct NaiveClientConfig {
    /// Server address (host:port).
    pub server_address: String,
    /// Server name (SNI), defaults to server_address's host.
    pub server_name: Option<String>,
    /// Username for basic auth.
    pub username: Option<String>,
    /// Password for basic auth.
    pub password: Option<String>,
    /// Number of concurrent connections (1 = no concurrency).
    pub concurrency: u32,
    /// Custom extra headers.
    pub extra_headers: HashMap<String, String>,
    /// Receive window size in bytes (0 = default).
    pub receive_window: u64,
    /// QUIC session receive window size.
    pub quic_session_receive_window: u64,
    /// PEM certificates to trust.
    pub trusted_root_certificates: Option<String>,
    /// Whether QUIC is enabled.
    pub quic_enabled: bool,
    /// QUIC congestion control.
    pub quic_congestion_control: QuicCongestionControl,
    /// Whether ECH is enabled.
    pub ech_enabled: bool,
    /// Pre-configured ECH config list.
    pub ech_config_list: Option<Vec<u8>>,
    /// Server name to query for ECH configs.
    pub ech_query_server_name: Option<String>,
}

impl Default for NaiveClientConfig {
    fn default() -> Self {
        Self {
            server_address: String::new(),
            server_name: None,
            username: None,
            password: None,
            concurrency: 1,
            extra_headers: HashMap::new(),
            receive_window: 0,
            quic_session_receive_window: 0,
            trusted_root_certificates: None,
            quic_enabled: false,
            quic_congestion_control: QuicCongestionControl::Default,
            ech_enabled: false,
            ech_config_list: None,
            ech_query_server_name: None,
        }
    }
}

/// State machine for the NaiveClient lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClientState {
    Created,
    Starting,
    Running,
    Closing,
    Closed,
}

/// The NaiveClient - a high-level Cronet-based proxy client.
pub struct NaiveClient {
    config: NaiveClientConfig,
    state: Arc<Mutex<ClientState>>,
    engine: Option<Engine>,
    stream_engine: Option<BidirectionalStreamEngine>,
    counter: AtomicU64,
    server_url: String,
    authorization: String,
}

impl NaiveClient {
    /// Create a new NaiveClient with the given configuration.
    pub fn new(config: NaiveClientConfig) -> std::result::Result<Self, CronetError> {
        if config.server_address.is_empty() {
            return Err(CronetError::Config("server_address is required".into()));
        }

        let server_name = config
            .server_name
            .clone()
            .unwrap_or_else(|| config.server_address.split(':').next().unwrap_or("").to_string());

        let server_url = format!("https://{}", server_name);

        let authorization = if let (Some(user), Some(pass)) = (&config.username, &config.password) {
            format!("Basic {}", base64_encode(&format!("{}:{}", user, pass)))
        } else {
            String::new()
        };

        Ok(Self {
            config,
            state: Arc::new(Mutex::new(ClientState::Created)),
            engine: None,
            stream_engine: None,
            counter: AtomicU64::new(0),
            server_url,
            authorization,
        })
    }

    /// Start the Cronet engine and prepare for connections.
    pub fn start(&mut self) -> std::result::Result<(), CronetError> {
        let mut state = self.state.lock().unwrap();
        match *state {
            ClientState::Closed => return Err(CronetError::Config("client is closed".into())),
            ClientState::Running => return Err(CronetError::Config("already started".into())),
            ClientState::Starting => return Err(CronetError::Config("start in progress".into())),
            _ => {}
        }
        *state = ClientState::Starting;
        drop(state);

        let mut engine = Engine::new();

        // Set trusted root certificates if provided
        if let Some(ref pem) = self.config.trusted_root_certificates {
            if !engine.set_trusted_root_certificates(pem) {
                return Err(CronetError::Config("failed to set trusted CA certificates".into()));
            }
        }

        // Build engine params
        let mut params_builder = EngineParamsBuilder::new();

        if self.config.quic_enabled {
            params_builder = params_builder.quic(true);
        } else {
            params_builder = params_builder.http2(true);
        }

        // Set experimental options for QUIC/H2 tuning
        let mut exp_opts = Vec::new();
        if self.config.quic_enabled {
            let stream_win = if self.config.receive_window > 0 {
                self.config.receive_window
            } else {
                8 * 1024 * 1024
            };
            let session_win = if self.config.quic_session_receive_window > 0 {
                self.config.quic_session_receive_window
            } else {
                20 * 1024 * 1024
            };
            exp_opts.push(format!(
                r#""QUIC":{{"connection_options":"{}","receive_stream_window_size":{},"session_max_reads":{}}}"#,
                self.config.quic_congestion_control.as_str(),
                stream_win,
                session_win,
            ));
        } else {
            let recv_win = if self.config.receive_window > 0 {
                self.config.receive_window
            } else {
                128 * 1024 * 1024
            };
            let header_table_size = recv_win / 2;
            exp_opts.push(format!(
                r#""SPDY":{{"recv_window_size":{},"header_table_size":{}}}"#,
                recv_win, header_table_size,
            ));
        }
        let json = format!("{{{}}}", exp_opts.join(","));
        params_builder = params_builder.experimental_options(json);

        let mut params = params_builder.build()?;

        engine.start_with_params(&params)?;
        params.destroy();

        self.stream_engine = Some(engine.stream_engine());
        self.engine = Some(engine);

        let mut state = self.state.lock().unwrap();
        *state = ClientState::Running;
        Ok(())
    }

    /// Dial a connection to the given destination.
    pub fn dial(&self, destination: &str) -> std::result::Result<NaiveConn, CronetError> {
        {
            let state = self.state.lock().unwrap();
            match *state {
                ClientState::Running => {}
                ClientState::Created | ClientState::Starting => {
                    return Err(CronetError::Config("client not started".into()));
                }
                _ => return Err(CronetError::ConnectionClosed),
            }
        }

        let stream_engine = self.stream_engine.as_ref().ok_or_else(|| {
            CronetError::Config("stream engine not available".into())
        })?;

        let mut conn = BidirectionalConn::new(stream_engine);

        // Build headers
        let mut headers = HashMap::new();
        headers.insert("-connect-authority".to_string(), destination.to_string());
        headers.insert(
            "Padding".to_string(),
            String::from_utf8_lossy(&NaiveConn::generate_padding_header()).to_string(),
        );
        if !self.authorization.is_empty() {
            headers.insert("proxy-authorization".to_string(), self.authorization.clone());
        }
        if self.config.quic_enabled {
            headers.insert("-force-quic".to_string(), "true".to_string());
        }
        for (key, value) in &self.config.extra_headers {
            headers.insert(key.clone(), value.clone());
        }

        // Multi-connection support
        if self.config.concurrency > 1 {
            let idx = self.counter.fetch_add(1, Ordering::Relaxed) % self.config.concurrency as u64;
            headers.insert(
                "-network-isolation-key".to_string(),
                format!("https://pool-{}:443", idx),
            );
        }

        conn.start("CONNECT", &self.server_url, &headers, 0, false)?;

        let naive_conn = NaiveConn::new(conn);
        Ok(naive_conn)
    }

    /// Dial and handshake in one step.
    pub fn dial_and_handshake(&self, destination: &str) -> std::result::Result<NaiveConn, CronetError> {
        let mut conn = self.dial(destination)?;
        conn.handshake()?;
        Ok(conn)
    }

    /// Close the client and shutdown the engine.
    pub fn close(&mut self) -> std::result::Result<(), CronetError> {
        let mut state = self.state.lock().unwrap();
        match *state {
            ClientState::Closed => return Ok(()),
            ClientState::Closing => return Ok(()),
            _ => *state = ClientState::Closing,
        }
        drop(state);

        if let Some(ref mut engine) = self.engine {
            engine.close_all_connections();
            engine.shutdown()?;
        }
        self.engine = None;
        self.stream_engine = None;

        let mut state = self.state.lock().unwrap();
        *state = ClientState::Closed;
        Ok(())
    }

    /// Get the underlying Cronet Engine (if running).
    pub fn engine(&self) -> Option<&Engine> {
        self.engine.as_ref()
    }
}

impl Drop for NaiveClient {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// Simple base64 encoding without external dependencies.
fn base64_encode(input: &str) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut result = String::new();

    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triplet = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARS[((triplet >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triplet >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            result.push(CHARS[((triplet >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(CHARS[(triplet & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64() {
        assert_eq!(base64_encode("user:pass"), "dXNlcjpwYXNz");
        assert_eq!(base64_encode("hello"), "aGVsbG8=");
    }

    #[test]
    fn test_create_client() {
        let config = NaiveClientConfig {
            server_address: "example.com:443".into(),
            ..Default::default()
        };
        let client = NaiveClient::new(config).unwrap();
        assert_eq!(client.authorization, "");
    }

    #[test]
    fn test_create_client_with_auth() {
        let config = NaiveClientConfig {
            server_address: "example.com:443".into(),
            username: Some("user".into()),
            password: Some("pass".into()),
            ..Default::default()
        };
        let client = NaiveClient::new(config).unwrap();
        assert_eq!(client.authorization, "Basic dXNlcjpwYXNz");
    }
}
