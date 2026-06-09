//! Cronet error codes and result types.

use std::fmt;

/// Cronet operation results returned by the C API.
///
/// Mirrors the `Cronet_RESULT` enum in `cronet.idl_c.h`. The values are grouped
/// into three families: illegal-argument (`-1xx`), illegal-state (`-2xx`), and
/// null-pointer (`-3xx`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum CronetResult {
    Success = 0,

    // ---- ILLEGAL_ARGUMENT family (-1xx) ----
    IllegalArgument = -100,
    IllegalArgumentStoragePathMustExist = -101,
    IllegalArgumentInvalidPin = -102,
    IllegalArgumentInvalidHostname = -103,
    IllegalArgumentInvalidHttpMethod = -104,
    IllegalArgumentInvalidHttpHeader = -105,

    // ---- ILLEGAL_STATE family (-2xx) ----
    IllegalState = -200,
    IllegalStateStoragePathInUse = -201,
    IllegalStateCannotShutdownEngineFromNetworkThread = -202,
    IllegalStateEngineAlreadyStarted = -203,
    IllegalStateRequestAlreadyStarted = -204,
    IllegalStateRequestNotInitialized = -205,
    IllegalStateRequestAlreadyInitialized = -206,
    IllegalStateRequestNotStarted = -207,
    IllegalStateUnexpectedRedirect = -208,
    IllegalStateUnexpectedRead = -209,
    IllegalStateReadFailed = -210,

    // ---- NULL_POINTER family (-3xx) ----
    NullPointer = -300,
    NullPointerHostname = -301,
    NullPointerSha256Pins = -302,
    NullPointerExpirationDate = -303,
    NullPointerEngine = -304,
    NullPointerUrl = -305,
    NullPointerCallback = -306,
    NullPointerExecutor = -307,
    NullPointerMethod = -308,
    NullPointerHeaderName = -309,
    NullPointerHeaderValue = -310,
    NullPointerParams = -311,
    NullPointerRequestFinishedInfoListenerExecutor = -312,

    /// Any value not defined by `Cronet_RESULT`.
    Unknown = -999,
}

impl From<i32> for CronetResult {
    fn from(v: i32) -> Self {
        match v {
            0 => Self::Success,

            -100 => Self::IllegalArgument,
            -101 => Self::IllegalArgumentStoragePathMustExist,
            -102 => Self::IllegalArgumentInvalidPin,
            -103 => Self::IllegalArgumentInvalidHostname,
            -104 => Self::IllegalArgumentInvalidHttpMethod,
            -105 => Self::IllegalArgumentInvalidHttpHeader,

            -200 => Self::IllegalState,
            -201 => Self::IllegalStateStoragePathInUse,
            -202 => Self::IllegalStateCannotShutdownEngineFromNetworkThread,
            -203 => Self::IllegalStateEngineAlreadyStarted,
            -204 => Self::IllegalStateRequestAlreadyStarted,
            -205 => Self::IllegalStateRequestNotInitialized,
            -206 => Self::IllegalStateRequestAlreadyInitialized,
            -207 => Self::IllegalStateRequestNotStarted,
            -208 => Self::IllegalStateUnexpectedRedirect,
            -209 => Self::IllegalStateUnexpectedRead,
            -210 => Self::IllegalStateReadFailed,

            -300 => Self::NullPointer,
            -301 => Self::NullPointerHostname,
            -302 => Self::NullPointerSha256Pins,
            -303 => Self::NullPointerExpirationDate,
            -304 => Self::NullPointerEngine,
            -305 => Self::NullPointerUrl,
            -306 => Self::NullPointerCallback,
            -307 => Self::NullPointerExecutor,
            -308 => Self::NullPointerMethod,
            -309 => Self::NullPointerHeaderName,
            -310 => Self::NullPointerHeaderValue,
            -311 => Self::NullPointerParams,
            -312 => Self::NullPointerRequestFinishedInfoListenerExecutor,

            _ => Self::Unknown,
        }
    }
}

impl fmt::Display for CronetResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Success => write!(f, "success"),

            Self::IllegalArgument => write!(f, "illegal argument"),
            Self::IllegalArgumentStoragePathMustExist => {
                write!(f, "illegal argument: storage path must exist")
            }
            Self::IllegalArgumentInvalidPin => write!(f, "illegal argument: invalid pin"),
            Self::IllegalArgumentInvalidHostname => write!(f, "illegal argument: invalid hostname"),
            Self::IllegalArgumentInvalidHttpMethod => {
                write!(f, "illegal argument: invalid http method")
            }
            Self::IllegalArgumentInvalidHttpHeader => {
                write!(f, "illegal argument: invalid http header")
            }

            Self::IllegalState => write!(f, "illegal state"),
            Self::IllegalStateStoragePathInUse => write!(f, "illegal state: storage path in use"),
            Self::IllegalStateCannotShutdownEngineFromNetworkThread => {
                write!(f, "illegal state: cannot shutdown engine from network thread")
            }
            Self::IllegalStateEngineAlreadyStarted => {
                write!(f, "illegal state: engine already started")
            }
            Self::IllegalStateRequestAlreadyStarted => {
                write!(f, "illegal state: request already started")
            }
            Self::IllegalStateRequestNotInitialized => {
                write!(f, "illegal state: request not initialized")
            }
            Self::IllegalStateRequestAlreadyInitialized => {
                write!(f, "illegal state: request already initialized")
            }
            Self::IllegalStateRequestNotStarted => {
                write!(f, "illegal state: request not started")
            }
            Self::IllegalStateUnexpectedRedirect => write!(f, "illegal state: unexpected redirect"),
            Self::IllegalStateUnexpectedRead => write!(f, "illegal state: unexpected read"),
            Self::IllegalStateReadFailed => write!(f, "illegal state: read failed"),

            Self::NullPointer => write!(f, "null pointer"),
            Self::NullPointerHostname => write!(f, "null pointer: hostname"),
            Self::NullPointerSha256Pins => write!(f, "null pointer: sha256 pins"),
            Self::NullPointerExpirationDate => write!(f, "null pointer: expiration date"),
            Self::NullPointerEngine => write!(f, "null pointer: engine"),
            Self::NullPointerUrl => write!(f, "null pointer: url"),
            Self::NullPointerCallback => write!(f, "null pointer: callback"),
            Self::NullPointerExecutor => write!(f, "null pointer: executor"),
            Self::NullPointerMethod => write!(f, "null pointer: method"),
            Self::NullPointerHeaderName => write!(f, "null pointer: header name"),
            Self::NullPointerHeaderValue => write!(f, "null pointer: header value"),
            Self::NullPointerParams => write!(f, "null pointer: params"),
            Self::NullPointerRequestFinishedInfoListenerExecutor => {
                write!(f, "null pointer: request finished info listener executor")
            }

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
