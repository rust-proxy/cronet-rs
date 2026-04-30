//! Cronet error codes and result types.

use std::fmt;

/// Cronet operation results returned by the C API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum CronetResult {
    Success = 0,
    IllegalArgument = -100,
    IllegalState = -101,
    IllegalMethodCalled = -102,
    IllegalMethodNotCalled = -103,
    NullPointer = -104,
    BadInterface = -105,
    Unknown = -999,
}

impl From<i32> for CronetResult {
    fn from(v: i32) -> Self {
        match v {
            0 => Self::Success,
            -100 => Self::IllegalArgument,
            -101 => Self::IllegalState,
            -102 => Self::IllegalMethodCalled,
            -103 => Self::IllegalMethodNotCalled,
            -104 => Self::NullPointer,
            -105 => Self::BadInterface,
            _ => Self::Unknown,
        }
    }
}

impl fmt::Display for CronetResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Success => write!(f, "success"),
            Self::IllegalArgument => write!(f, "illegal argument"),
            Self::IllegalState => write!(f, "illegal state"),
            Self::IllegalMethodCalled => write!(f, "illegal method called"),
            Self::IllegalMethodNotCalled => write!(f, "illegal method not called"),
            Self::NullPointer => write!(f, "null pointer"),
            Self::BadInterface => write!(f, "bad interface"),
            Self::Unknown => write!(f, "unknown error"),
        }
    }
}

/// Network error codes from Cronet (net/base/net_error_list.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetError(pub i32);

#[allow(non_upper_case_globals)]
impl NetError {
    pub const IO_PENDING: Self = Self(-1);
    pub const FAILED: Self = Self(-2);
    pub const ABORTED: Self = Self(-3);
    pub const CONNECTION_FAILED: Self = Self(-36);
    pub const CONNECTION_REFUSED: Self = Self(-33);
    pub const CONNECTION_RESET: Self = Self(-34);
    pub const CONNECTION_TIMED_OUT: Self = Self(-32);
    pub const CONNECTION_CLOSED: Self = Self(-31);
    pub const CONNECTION_ABORTED: Self = Self(-35);
    pub const TIMED_OUT: Self = Self(-7);
    pub const HOSTNAME_NOT_RESOLVED: Self = Self(-29);
    pub const INTERNET_DISCONNECTED: Self = Self(-30);
    pub const ADDRESS_UNREACHABLE: Self = Self(-37);
    pub const QUIC_PROTOCOL_FAILED: Self = Self(-38);
    pub const CERT_INVALID: Self = Self(-54);
    pub const CERT_AUTHORITY_INVALID: Self = Self(-49);
    pub const CERT_COMMON_NAME_INVALID: Self = Self(-47);
    pub const CERT_DATE_INVALID: Self = Self(-48);

    pub fn description(&self) -> &'static str {
        match *self {
            Self::IO_PENDING => "IO pending",
            Self::FAILED => "failed",
            Self::ABORTED => "aborted",
            Self::CONNECTION_FAILED => "connection failed",
            Self::CONNECTION_REFUSED => "connection refused",
            Self::CONNECTION_RESET => "connection reset",
            Self::CONNECTION_TIMED_OUT => "connection timed out",
            Self::CONNECTION_CLOSED => "connection closed",
            Self::TIMED_OUT => "timed out",
            Self::HOSTNAME_NOT_RESOLVED => "hostname not resolved",
            Self::INTERNET_DISCONNECTED => "internet disconnected",
            Self::ADDRESS_UNREACHABLE => "address unreachable",
            Self::QUIC_PROTOCOL_FAILED => "QUIC protocol failed",
            Self::CERT_INVALID => "certificate invalid",
            Self::CERT_AUTHORITY_INVALID => "certificate authority invalid",
            Self::CERT_COMMON_NAME_INVALID => "certificate common name invalid",
            Self::CERT_DATE_INVALID => "certificate date invalid",
            _ => "unknown network error",
        }
    }
}

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NetError({}: {})", self.0, self.description())
    }
}

impl std::error::Error for NetError {}

impl From<i32> for NetError {
    fn from(v: i32) -> Self {
        Self(v)
    }
}

/// Error code for Cronet URL request errors (from cronet_c.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum ErrorCode {
    ErrorCallback = 0,
    ErrorHostnameNotResolved = 1,
    ErrorInternetDisconnected = 2,
    ErrorNetworkChanged = 3,
    ErrorTimedOut = 4,
    ErrorConnectionClosed = 5,
    ErrorConnectionTimedOut = 6,
    ErrorConnectionRefused = 7,
    ErrorConnectionReset = 8,
    ErrorAddressUnreachable = 9,
    ErrorQuicProtocolFailed = 10,
    ErrorOther = 11,
}

/// The main error type for cronet-rs operations.
#[derive(Debug, thiserror::Error)]
pub enum CronetError {
    /// Cronet API returned a non-success result code.
    #[error("cronet result error: {0}")]
    Result(CronetResult),

    /// Cronet network error.
    #[error("cronet net error: {0}")]
    Net(NetError),

    /// IO errors from I/O operations.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Library not loaded.
    #[error("libcronet not loaded: {0}")]
    LibraryNotLoaded(String),

    /// Unexpected response status (e.g. non-200).
    #[error("unexpected response status: {0}")]
    BadStatus(String),

    /// Invalid configuration.
    #[error("invalid config: {0}")]
    Config(String),

    /// Connection closed.
    #[error("connection closed")]
    ConnectionClosed,

    /// Stream canceled.
    #[error("canceled")]
    Canceled,
}

impl From<NetError> for CronetError {
    fn from(e: NetError) -> Self {
        Self::Net(e)
    }
}

impl From<CronetResult> for CronetError {
    fn from(r: CronetResult) -> Self {
        Self::Result(r)
    }
}
