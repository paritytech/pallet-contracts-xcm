#![cfg_attr(not(feature = "std"), no_std)]

use ink_env::chain_extension::FromStatusCode;
use ink_lang as ink;
use scale::Decode;
pub use xcm::{VersionedMultiAsset, VersionedMultiLocation, VersionedResponse, VersionedXcm};

#[derive(Decode)]
pub enum Error {
    NoResponse = 1,
}

impl FromStatusCode for Error {
    fn from_status_code(status_code: u32) -> Result<(), Self> {
        match status_code {
            0 => Ok(()),
            _ => Err(Self::NoResponse),
        }
    }
}

#[ink::chain_extension]
pub trait Extension {
    type ErrorCode = Error;

    #[ink(extension = 0, handle_status = false, returns_result = false)]
    fn prepare_execute(xcm: VersionedXcm<()>) -> u64;

    #[ink(extension = 1, handle_status = false, returns_result = false)]
    fn execute();

    #[ink(extension = 2, handle_status = false, returns_result = false)]
    fn prepare_send(dest: VersionedMultiLocation, xcm: VersionedXcm<()>) -> VersionedMultiAsset;

    #[ink(extension = 3, handle_status = false, returns_result = false)]
    fn send();

    #[ink(extension = 4, handle_status = false, returns_result = false)]
    fn new_query() -> u64;

    #[ink(extension = 5, handle_status = true, returns_result = false)]
    fn take_response(query_id: u64) -> Result<VersionedResponse, Error>;
}
