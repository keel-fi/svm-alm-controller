use core::ptr;

extern crate alloc;
use alloc::vec::Vec;
use pinocchio::{account_info::AccountInfo, pubkey::Pubkey};

#[repr(C)]
struct AccountFields {
    borrow_state: u8,
    is_signer: u8,
    is_writable: u8,
    executable: u8,
    resize_delta: i32,
    key: Pubkey,
    owner: Pubkey,
    lamports: u64,
    data_len: u64,
}

// Helper to create a mock AccountInfo
// NOTE: Creating AccountInfo for testing is complex due to pinocchio's internal structure.
// This is a simplified mock that may need adjustment based on pinocchio's actual AccountInfo implementation.
// For production use, consider using integration tests with lite svm instead.
pub fn create_mock_account_info(
    pubkey: Pubkey,
    owner: Pubkey,
    lamports: u64,
    data: Vec<u8>,
) -> (AccountInfo, Pubkey) {
    const NOT_BORROWED: u8 = 0b_1111_1111;

    let account_size = core::mem::size_of::<AccountFields>();
    let total_size = account_size + data.len();
    let storage = alloc::vec![0u8; total_size].into_boxed_slice();
    let storage = alloc::boxed::Box::leak(storage);
    let account_ptr = storage.as_mut_ptr() as *mut AccountFields;

    unsafe {
        (*account_ptr).borrow_state = NOT_BORROWED;
        (*account_ptr).is_signer = 0;
        (*account_ptr).is_writable = 0;
        (*account_ptr).executable = 0;
        (*account_ptr).resize_delta = 0;
        (*account_ptr).key = pubkey;
        (*account_ptr).owner = owner;
        (*account_ptr).lamports = lamports;
        (*account_ptr).data_len = data.len() as u64;

        let data_ptr = (account_ptr as *mut u8).add(account_size);
        ptr::copy_nonoverlapping(data.as_ptr(), data_ptr, data.len());
    }

    let account_info: AccountInfo = unsafe { core::mem::transmute(account_ptr) };
    let actual_key = *account_info.key();

    (account_info, actual_key)
}
