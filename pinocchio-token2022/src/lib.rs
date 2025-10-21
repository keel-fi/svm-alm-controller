#![no_std]

pub mod extensions;

pub use pinocchio_token_2022::*;

/// Deserialize a type from a byte array.
///
/// # Safety
///
/// This function is unsafe because it transmutes the input data to the output type.
pub unsafe fn from_bytes<T: Clone + Copy>(data: &[u8]) -> T {
    assert_eq!(data.len(), core::mem::size_of::<T>());
    *(data.as_ptr() as *const T)
}

/// Deserialize a type from a byte array into a reference.
///
/// # Safety
///
/// This function is unsafe because it transmutes the input data to the output type.
pub unsafe fn from_bytes_ref<T: Clone + Copy>(data: &[u8]) -> &T {
    assert_eq!(data.len(), core::mem::size_of::<T>());
    &*(data.as_ptr() as *const T)
}
