//! Run with the published 143 and 154 libraries to exercise retry in one process.
#![cfg(not(cronet_static_link))]

#[test]
#[ignore = "requires CRONET_143_LIBRARY and CRONET_154_LIBRARY"]
fn reject_old_abi_then_load_154() {
    let old = std::env::var("CRONET_143_LIBRARY").expect("143 library path");
    let new = std::env::var("CRONET_154_LIBRARY").expect("154 library path");
    let error = unsafe { cronet_sys::load_library(&old) }.expect_err("old dialer ABI");
    assert!(
        error.to_string().contains("incompatible Cronet 143."),
        "{error}"
    );
    unsafe {
        cronet_sys::load_library(&new).expect("retry with the 154 library");
        let engine = cronet_sys::Cronet_Engine_Create();
        cronet_sys::Cronet_Engine_SetUdpDialer(engine, None, std::ptr::null_mut(), None);
        cronet_sys::Cronet_Engine_Destroy(engine);
    }
}
