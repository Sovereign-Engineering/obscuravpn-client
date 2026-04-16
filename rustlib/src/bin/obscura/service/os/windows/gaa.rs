use std::mem::MaybeUninit;

use windows::Win32::{
    Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_NO_DATA},
    NetworkManagement::IpHelper::{GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_INCLUDE_PREFIX, GetAdaptersAddresses, IP_ADAPTER_ADDRESSES_LH},
    Networking::WinSock::AF_INET,
};

pub struct GAABufferInit {
    _buffer: Box<[MaybeUninit<IP_ADAPTER_ADDRESSES_LH>]>,
    pub first: *const IP_ADAPTER_ADDRESSES_LH,
}

impl GAABufferInit {
    pub fn new() -> Result<Option<Self>, ()> {
        let family = AF_INET.0 as u32;
        // See all GAA_FLAG's here
        // https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/NetworkManagement/IpHelper/
        let flags = GAA_FLAG_INCLUDE_PREFIX | GAA_FLAG_INCLUDE_GATEWAYS;

        // It's recommended to pre-allocate a 15KB working buffer
        // https://learn.microsoft.com/windows/win32/api/iphlpapi/nf-iphlpapi-getadaptersaddresses#remarks
        let mut buf_len = 15_000u32;
        let struct_size = std::mem::size_of::<IP_ADAPTER_ADDRESSES_LH>();
        let mut buffer: Box<[MaybeUninit<IP_ADAPTER_ADDRESSES_LH>]>;

        let first = loop {
            let capacity = (buf_len as usize / struct_size) + 1;
            buffer = Box::new_uninit_slice(capacity);
            let start_of_buffer = buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;

            // SAFETY: `buffer` has size `capacity` which in bytes is greater than `buf_len`
            // `GetAdaptersAddresses` will only write within buf_len and otherwise reports `ERROR_BUFFER_OVERFLOW` (and sets `buf_len`)
            let ret = unsafe { GetAdaptersAddresses(family, flags, None, Some(start_of_buffer), &mut buf_len) };

            if ret == ERROR_BUFFER_OVERFLOW.0 {
                // `buf_len` has been updated to the required size; retry with a larger buffer.
                continue;
            }

            if ret == ERROR_NO_DATA.0 {
                return Ok(None);
            }

            if ret != 0 {
                tracing::error!(message_id = "p5n35MKN", ret, "GetAdaptersAddresses failed");
                return Err(());
            }

            break start_of_buffer;
        };
        Ok(Some(Self { _buffer: buffer, first }))
    }
}
