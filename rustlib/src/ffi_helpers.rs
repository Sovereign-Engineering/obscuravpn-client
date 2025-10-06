use std::{ffi::c_void, marker::PhantomData};

#[repr(C)]
pub struct FfiBytes<'a> {
    buffer: *const c_void,
    len: usize,
    phantom: PhantomData<&'a [u8]>,
}

impl<'a> FfiBytes<'a> {
    pub fn as_slice(&self) -> &'a [u8] {
        if self.len == 0 {
            // catch zero sized early, to allow empty null pointer buffers
            &[]
        } else {
            // SAFETY: This type must be constructed such that this is safe
            unsafe { std::slice::from_raw_parts(self.buffer as *const u8, self.len) }
        }
    }
}

impl FfiBytes<'_> {
    pub fn to_vec(&self) -> Vec<u8> {
        self.as_slice().to_vec()
    }
}

impl<'a, T: AsRef<[u8]>> From<&'a T> for FfiBytes<'a> {
    fn from(b: &'a T) -> Self {
        let b = b.as_ref();
        FfiBytes { buffer: b.as_ptr() as *const c_void, len: b.len(), phantom: PhantomData }
    }
}

pub trait FfiBytesExt {
    fn ffi(&self) -> FfiBytes<'_>;
}

impl<T: AsRef<[u8]>> FfiBytesExt for T {
    fn ffi(&self) -> FfiBytes<'_> {
        self.into()
    }
}

#[repr(C)]
pub struct FfiStr<'a> {
    bytes: FfiBytes<'a>,
}

impl<'a> FfiStr<'a> {
    pub fn as_str(&self) -> &'a str {
        std::str::from_utf8(self.bytes.as_slice()).expect("ffi buffer does not contain valid utf8")
    }
}

impl std::fmt::Display for FfiStr<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_str().fmt(f)
    }
}

impl<'a, T: AsRef<str>> From<&'a T> for FfiStr<'a> {
    fn from(s: &'a T) -> Self {
        let s = s.as_ref();
        FfiStr { bytes: FfiBytes { buffer: s.as_ptr() as *const c_void, len: s.len(), phantom: PhantomData } }
    }
}

pub trait FfiStrExt {
    fn ffi_str(&self) -> FfiStr<'_>;
}

impl<T: AsRef<str>> FfiStrExt for T {
    fn ffi_str(&self) -> FfiStr<'_> {
        self.into()
    }
}
