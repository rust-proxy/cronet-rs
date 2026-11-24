//! Linux socket ownership at the Cronet C dialer boundary.

use std::{
    ffi::{CStr, c_char, c_void},
    fmt::Debug,
    io,
    net::{SocketAddr, TcpStream, UdpSocket},
    os::fd::IntoRawFd,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

/// Implementations return connected sockets and may be called on Cronet's network thread.
/// TCP connects must have a deadline. Cronet assumes ownership on success.
pub trait SocketDialer: Debug + Send + Sync {
    fn connect_tcp(&self, peer: SocketAddr) -> io::Result<TcpStream>;
    fn connect_udp(&self, peer: SocketAddr) -> io::Result<UdpSocket>;
}

#[derive(Debug)]
pub(crate) struct DialerContext(pub(crate) Arc<dyn SocketDialer>);

fn dial(
    context: *mut c_void,
    address: *const c_char,
    port: u16,
    connect: impl FnOnce(&dyn SocketDialer, SocketAddr) -> io::Result<i32>,
) -> i32 {
    // Both pointers come from Cronet. The engine owns the context, and the
    // address is a NUL-terminated IP literal valid for this callback.
    if context.is_null() || address.is_null() {
        return -4; // ERR_INVALID_ARGUMENT
    }
    catch_unwind(AssertUnwindSafe(|| {
        let dialer = unsafe { &*context.cast::<DialerContext>() };
        let ip = unsafe { CStr::from_ptr(address) }
            .to_str()
            .ok()
            .and_then(|value| value.parse().ok());
        let Some(ip) = ip else { return -4 };
        match connect(&*dialer.0, SocketAddr::new(ip, port)) {
            Ok(fd) => fd,
            Err(err) => {
                log::debug!("Cronet socket dial failed: {err}");
                match err.kind() {
                    io::ErrorKind::PermissionDenied => -10,
                    io::ErrorKind::ConnectionRefused => -102,
                    io::ErrorKind::TimedOut => -118,
                    io::ErrorKind::NetworkUnreachable | io::ErrorKind::HostUnreachable => -109,
                    io::ErrorKind::InvalidInput => -4,
                    _ => -104,
                }
            }
        }
    }))
    .unwrap_or(-2) // ERR_FAILED; never unwind across the C boundary.
}

pub(crate) extern "C" fn tcp(ctx: *mut c_void, ip: *const c_char, port: u16) -> i32 {
    dial(ctx, ip, port, |dialer, peer| {
        Ok(dialer.connect_tcp(peer)?.into_raw_fd())
    })
}

pub(crate) extern "C" fn udp(
    ctx: *mut c_void,
    ip: *const c_char,
    port: u16,
    local_ip: *mut c_char,
    local_port: *mut u16,
) -> i32 {
    if local_ip.is_null() || local_port.is_null() {
        return -4;
    }
    dial(ctx, ip, port, |dialer, peer| {
        let socket = dialer.connect_udp(peer)?;
        let local = socket.local_addr()?;
        let text = local.ip().to_string();
        // Cronet provides INET6_ADDRSTRLEN (46) bytes. An IPv6 literal
        // without a scope suffix fits, including the terminating NUL.
        unsafe {
            std::ptr::copy_nonoverlapping(text.as_ptr(), local_ip.cast(), text.len());
            *local_ip.add(text.len()) = 0;
            *local_port = local.port();
        }
        Ok(socket.into_raw_fd())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::FromRawFd;

    #[derive(Debug)]
    struct LoopbackDialer;

    impl SocketDialer for LoopbackDialer {
        fn connect_tcp(&self, _: SocketAddr) -> io::Result<TcpStream> {
            Err(io::ErrorKind::PermissionDenied.into())
        }

        fn connect_udp(&self, peer: SocketAddr) -> io::Result<UdpSocket> {
            let socket = UdpSocket::bind((peer.ip(), 0))?;
            socket.connect(peer)?;
            Ok(socket)
        }
    }

    #[test]
    fn udp_transfers_a_connected_fd_and_reports_its_local_endpoint() -> io::Result<()> {
        let peer = UdpSocket::bind("[::1]:0")?;
        let mut context = DialerContext(Arc::new(LoopbackDialer));
        let ctx = std::ptr::from_mut(&mut context).cast();
        let mut local_ip = [0; 46];
        let mut port = 0;
        let fd = udp(
            ctx,
            c"::1".as_ptr(),
            peer.local_addr()?.port(),
            local_ip.as_mut_ptr(),
            &mut port,
        );
        assert!(fd >= 0);
        // The callback transfers ownership of exactly one fd to the caller.
        let socket = unsafe { UdpSocket::from_raw_fd(fd) };
        assert_eq!(unsafe { CStr::from_ptr(local_ip.as_ptr()) }, c"::1");
        assert_eq!(socket.local_addr()?.port(), port);
        assert_eq!(socket.peer_addr()?, peer.local_addr()?);
        assert_eq!(tcp(ctx, c"127.0.0.1".as_ptr(), 443), -10);
        assert_eq!(tcp(ctx, c"invalid".as_ptr(), 443), -4);
        Ok(())
    }
}
