//! Integration tests for error types and conversions.
//!
//! These tests do not require libcronet.

use cronet_rs::error::{CronetError, CronetResult, ErrorCode, NetError};

#[test]
fn test_cronet_result_conversion() {
    assert_eq!(CronetResult::from(0), CronetResult::Success);
    assert_eq!(CronetResult::from(-100), CronetResult::IllegalArgument);
    assert_eq!(CronetResult::from(-101), CronetResult::IllegalState);
    assert_eq!(CronetResult::from(-102), CronetResult::IllegalMethodCalled);
    assert_eq!(CronetResult::from(-103), CronetResult::IllegalMethodNotCalled);
    assert_eq!(CronetResult::from(-104), CronetResult::NullPointer);
    assert_eq!(CronetResult::from(-105), CronetResult::BadInterface);
    assert_eq!(CronetResult::from(-999), CronetResult::Unknown);
}

#[test]
fn test_net_error_constants() {
    assert_eq!(NetError::IO_PENDING.0, -1);
    assert_eq!(NetError::FAILED.0, -2);
    assert_eq!(NetError::CONNECTION_FAILED.0, -36);
    assert_eq!(NetError::CONNECTION_REFUSED.0, -33);
    assert_eq!(NetError::CONNECTION_RESET.0, -34);
    assert_eq!(NetError::CONNECTION_TIMED_OUT.0, -32);
    assert_eq!(NetError::CONNECTION_CLOSED.0, -31);
    assert_eq!(NetError::TIMED_OUT.0, -7);
    assert_eq!(NetError::HOSTNAME_NOT_RESOLVED.0, -29);
    assert_eq!(NetError::INTERNET_DISCONNECTED.0, -30);
    assert_eq!(NetError::ADDRESS_UNREACHABLE.0, -37);
    assert_eq!(NetError::QUIC_PROTOCOL_FAILED.0, -38);
    assert_eq!(NetError::CERT_INVALID.0, -54);
    assert_eq!(NetError::CERT_AUTHORITY_INVALID.0, -49);
    assert_eq!(NetError::CERT_COMMON_NAME_INVALID.0, -47);
    assert_eq!(NetError::CERT_DATE_INVALID.0, -48);
}

#[test]
fn test_net_error_descriptions() {
    assert_eq!(NetError::CONNECTION_REFUSED.description(), "connection refused");
    assert_eq!(NetError::CERT_AUTHORITY_INVALID.description(), "certificate authority invalid");
    assert_eq!(NetError::HOSTNAME_NOT_RESOLVED.description(), "hostname not resolved");
}

#[test]
fn test_net_error_display() {
    let err = NetError::CONNECTION_TIMED_OUT;
    let display = format!("{}", err);
    assert!(display.contains("connection timed out"));
    assert!(display.contains("-32"));
}

#[test]
fn test_net_error_from_i32() {
    let err: NetError = (-34).into();
    assert_eq!(err, NetError::CONNECTION_RESET);
}

#[test]
fn test_cronet_error_from_net_error() {
    let net_err = NetError::CONNECTION_REFUSED;
    let cronet_err: CronetError = net_err.into();
    match cronet_err {
        CronetError::Net(n) => assert_eq!(n, NetError::CONNECTION_REFUSED),
        _ => panic!("expected Net variant"),
    }
}

#[test]
fn test_cronet_error_from_cronet_result() {
    let result = CronetResult::IllegalState;
    let err: CronetError = result.into();
    match err {
        CronetError::Result(r) => assert_eq!(r, CronetResult::IllegalState),
        _ => panic!("expected Result variant"),
    }
}

#[test]
fn test_cronet_error_display() {
    let err = CronetError::BadStatus("500".into());
    let display = format!("{}", err);
    assert!(display.contains("500"));
}

#[test]
fn test_all_cronet_error_variants() {
    // Verify Send + Sync boundaries
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CronetError>();
    assert_send_sync::<CronetResult>();
    assert_send_sync::<NetError>();
}

#[test]
fn test_error_code_discriminants() {
    assert_eq!(ErrorCode::ErrorCallback as i32, 0);
    assert_eq!(ErrorCode::ErrorHostnameNotResolved as i32, 1);
    assert_eq!(ErrorCode::ErrorInternetDisconnected as i32, 2);
    assert_eq!(ErrorCode::ErrorNetworkChanged as i32, 3);
    assert_eq!(ErrorCode::ErrorTimedOut as i32, 4);
    assert_eq!(ErrorCode::ErrorConnectionClosed as i32, 5);
    assert_eq!(ErrorCode::ErrorConnectionTimedOut as i32, 6);
    assert_eq!(ErrorCode::ErrorConnectionRefused as i32, 7);
    assert_eq!(ErrorCode::ErrorConnectionReset as i32, 8);
    assert_eq!(ErrorCode::ErrorAddressUnreachable as i32, 9);
    assert_eq!(ErrorCode::ErrorQuicProtocolFailed as i32, 10);
    assert_eq!(ErrorCode::ErrorOther as i32, 11);
}

#[test]
fn test_cronet_result_display() {
    assert_eq!(format!("{}", CronetResult::Success), "success");
    assert_eq!(format!("{}", CronetResult::NullPointer), "null pointer");
}

#[test]
fn test_net_error_debug() {
    let err = NetError::CONNECTION_RESET;
    let debug = format!("{:?}", err);
    assert!(debug.contains("CONNECTION_RESET") || debug.contains("NetError"));
}
