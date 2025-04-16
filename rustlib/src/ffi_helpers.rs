use std::{ffi::c_void, marker::PhantomData};

#[repr(C)]
pub struct FfiBytes<'a> {
    buffer: *const c_void,
    len: usize,
    phantom: PhantomData<&'a [u8]>,
}

impl FfiBytes<'_> {
    pub unsafe fn to_vec(&self) -> Vec<u8> {
        if self.len == 0 {
            // catch zero sized early, to allow empty null pointer buffers
            return Vec::new();
        }
        core::slice::from_raw_parts(self.buffer as *const u8, self.len).to_vec()
    }
}

impl<'a, T: AsRef<[u8]>> From<&'a T> for FfiBytes<'a> {
    fn from(b: &'a T) -> Self {
        let b = b.as_ref();
        FfiBytes { buffer: b.as_ptr() as *const c_void, len: b.len(), phantom: PhantomData }
    }
}

pub trait FfiBytesExt {
    fn ffi(&self) -> FfiBytes;
}

impl<T: AsRef<[u8]>> FfiBytesExt for T {
    fn ffi(&self) -> FfiBytes {
        self.into()
    }
}

#[repr(C)]
pub struct FfiStr<'a> {
    bytes: FfiBytes<'a>,
}

impl FfiStr<'_> {
    pub unsafe fn to_string(&self) -> String {
        String::from_utf8(self.bytes.to_vec()).expect("ffi buffer does not contain valid utf8")
    }
}

impl<'a, T: AsRef<str>> From<&'a T> for FfiStr<'a> {
    fn from(s: &'a T) -> Self {
        let s = s.as_ref();
        FfiStr { bytes: FfiBytes { buffer: s.as_ptr() as *const c_void, len: s.len(), phantom: PhantomData } }
    }
}

pub trait FfiStrExt {
    fn ffi_str(&self) -> FfiStr;
}

impl<T: AsRef<str>> FfiStrExt for T {
    fn ffi_str(&self) -> FfiStr {
        self.into()
    }
}
