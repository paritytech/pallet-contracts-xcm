// Copyright Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]

use ink::env::Environment;
pub use xcm::{
    v3::prelude, VersionedMultiAsset, VersionedMultiLocation, VersionedResponse, VersionedXcm,
};

/// The error type returned by the XCM extension.
#[derive(Debug, Copy, Clone, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum XCMError {
    /// The XCM query requested via [`XCMExtension::take_response`] was not found
    QueryNotFound = 1,
}

impl ink::env::chain_extension::FromStatusCode for XCMError {
    fn from_status_code(status_code: u32) -> Result<(), Self> {
        match status_code {
            0 => Ok(()),
            1 => Err(Self::QueryNotFound),
            _ => panic!("Unknown status code"),
        }
    }
}

type DefaultBlockNumber = <ink::env::DefaultEnvironment as Environment>::BlockNumber;

#[ink::chain_extension]
pub trait XCMExtension {
    type ErrorCode = XCMError;

    /// Execute an XCM message locally, using the contract's address as the origin.
    #[ink(extension = 1)]
    fn execute(xcm: VersionedXcm<()>);

    /// Send an XCM message from the contract to the specified destination.
    #[ink(extension = 2)]
    fn send(dest: VersionedMultiLocation, message: VersionedXcm<()>);

    /// Create a new query, using the contract's address as the responder.
    ///
    /// Returns the query id.
    /// TODO: timeout should be generic over the BlockNumber but chain_extension does not support generics.
    #[ink(extension = 3)]
    fn new_query(timeout: DefaultBlockNumber, match_querier: VersionedMultiLocation) -> u64;

    /// Attempt to take a response for the specified query.
    ///
    /// Returns the response if it is has been received, None, if the query is pending, and an error otherwise.
    #[ink(extension = 4)]
    fn take_response(query_id: u64) -> Option<VersionedResponse>;
}
