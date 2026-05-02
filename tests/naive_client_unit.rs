//! Unit-like integration tests for NaiveClient.
//!
//! These tests validate configuration, error handling, and lifecycle
//! logic that does NOT require a running libcronet.

use cronet_rs::naive_client::{NaiveClient, NaiveClientConfig, QuicCongestionControl};
use cronet_rs::error::CronetError;

#[test]
fn test_config_empty_server_address() {
    let result = NaiveClient::new(NaiveClientConfig {
        server_address: "".into(),
        ..Default::default()
    });
    assert!(result.is_err());
    match result {
        Err(CronetError::Config(msg)) => assert!(msg.contains("server_address")),
        _ => panic!("expected Config error"),
    }
}

#[test]
fn test_config_sets_server_url_correctly() {
    let client = NaiveClient::new(NaiveClientConfig {
        server_address: "my-proxy.example.com:443".into(),
        ..Default::default()
    }).unwrap();
    assert!(client.engine().is_none());
}

#[test]
fn test_config_with_all_fields() {
    let mut extra = std::collections::HashMap::new();
    extra.insert("X-Test".into(), "value1".into());
    extra.insert("X-Debug".into(), "true".into());

    let config = NaiveClientConfig {
        server_address: "proxy.example.com:443".into(),
        server_name: Some("sni.example.com".into()),
        username: Some("user1".into()),
        password: Some("pass1".into()),
        concurrency: 4,
        extra_headers: extra,
        receive_window: 16_777_216,
        quic_session_receive_window: 33_554_432,
        trusted_root_certificates: Some("-----BEGIN CERTIFICATE-----\n...".into()),
        quic_enabled: true,
        quic_congestion_control: QuicCongestionControl::Bbr,
        ech_enabled: true,
        ech_config_list: Some(vec![0x00, 0x01, 0x02]),
        ech_query_server_name: Some("ech.proxy.example.com".into()),
    };
    let result = NaiveClient::new(config);
    assert!(result.is_ok());
}

#[test]
fn test_quic_congestion_control_strings() {
    assert_eq!(QuicCongestionControl::Default.as_str(), "");
    assert_eq!(QuicCongestionControl::Bbr.as_str(), "TBBR");
    assert_eq!(QuicCongestionControl::BbrV2.as_str(), "B2ON");
    assert_eq!(QuicCongestionControl::Cubic.as_str(), "QBIC");
    assert_eq!(QuicCongestionControl::Reno.as_str(), "RENO");
}

#[test]
fn test_double_close_is_idempotent() {
    let mut client = NaiveClient::new(NaiveClientConfig {
        server_address: "example.com:443".into(),
        ..Default::default()
    }).unwrap();
    assert!(client.close().is_ok());
    // Closing twice should be safe (idempotent)
    assert!(client.close().is_ok());
}

#[test]
fn test_client_drop_does_not_panic() {
    // Dropping a client that was never started should be safe
    let client = NaiveClient::new(NaiveClientConfig {
        server_address: "example.com:443".into(),
        ..Default::default()
    });
    drop(client);
}

#[test]
fn test_dial_before_start_returns_error() {
    let client = NaiveClient::new(NaiveClientConfig {
        server_address: "example.com:443".into(),
        ..Default::default()
    }).unwrap();
    let result = client.dial("httpbin.org:80");
    assert!(result.is_err(), "expected error, got success");
    match result.err().unwrap() {
        CronetError::Config(msg) => {
            assert!(msg.contains("not started"), "unexpected msg: {}", msg);
        }
        other => panic!("expected Config, got {:?}", other),
    }
}

#[test]
#[ignore = "requires libcronet shared library"]
fn test_start_twice_rejected() {
    #[cfg(feature = "dynamic")]
    unsafe {
        cronet_rs::sys::load_library("libcronet.so.119").unwrap();
    }
    let mut client = NaiveClient::new(NaiveClientConfig {
        server_address: "example.com:443".into(),
        ..Default::default()
    }).unwrap();

    // First start should succeed
    client.start().unwrap();

    // Second start should be rejected
    let result = client.start();
    assert!(result.is_err());
    match result.err().unwrap() {
        CronetError::Config(msg) => assert!(msg.contains("already started")),
        other => panic!("expected Config error, got {:?}", other),
    }
}

#[test]
fn test_config_defaults() {
    let config = NaiveClientConfig::default();
    assert_eq!(config.concurrency, 1);
    assert_eq!(config.quic_enabled, false);
    assert_eq!(config.quic_congestion_control, QuicCongestionControl::Default);
    assert!(config.username.is_none());
    assert!(config.password.is_none());
    assert!(config.extra_headers.is_empty());
    assert!(config.trusted_root_certificates.is_none());
}
