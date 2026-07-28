#![allow(clippy::as_conversions)]

use static_assertions::const_assert;
use std::ffi::{c_int, c_ulong};

pub const fn try_c_ulong_into_c_int(value: c_ulong) -> Option<c_int> {
    if value as c_int as c_ulong == value {
        Some(value as c_int)
    } else {
        None
    }
}

pub const fn u16_into_usize(value: u16) -> usize {
    const_assert!(size_of::<usize>() >= size_of::<u16>());
    value as usize
}

pub const fn u32_into_usize(value: u32) -> usize {
    const_assert!(size_of::<usize>() >= size_of::<u32>());
    value as usize
}

pub const fn try_usize_into_u32(value: usize) -> Option<u32> {
    if value as u32 as usize == value { Some(value as u32) } else { None }
}

pub const fn usize_into_u64(value: usize) -> u64 {
    const_assert!(size_of::<usize>() <= size_of::<u64>());
    value as u64
}
