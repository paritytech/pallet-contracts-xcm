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

//! A Test ink! contract that uses the XCM chain extension, used to test the XCM chain extension.

#![cfg_attr(not(feature = "std"), no_std, no_main)]

use ink::env::Environment;
use extension_xcm::XCMExtension;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum CustomEnvironment {}

impl Environment for CustomEnvironment {
    const MAX_EVENT_TOPICS: usize = <ink::env::DefaultEnvironment as Environment>::MAX_EVENT_TOPICS;

    type AccountId = <ink::env::DefaultEnvironment as Environment>::AccountId;
    type Balance = <ink::env::DefaultEnvironment as Environment>::Balance;
    type Hash = <ink::env::DefaultEnvironment as Environment>::Hash;
    type BlockNumber = <ink::env::DefaultEnvironment as Environment>::BlockNumber;
    type Timestamp = <ink::env::DefaultEnvironment as Environment>::Timestamp;

    type ChainExtension = XCMExtension;
}

#[ink::contract(env = crate::CustomEnvironment)]
mod xcm_contract {
    use extension_xcm::{
        prelude::*, VersionedMultiLocation, VersionedResponse, VersionedXcm, XCMError,
    };
    use scale_info::prelude::vec::Vec;

    #[ink(storage)]
    pub struct XcmContract {}

    impl XcmContract {
        #[ink(constructor)]
        pub fn new() -> Self {
            ink::env::debug_println!("instantiating contract");
            Self {}
        }

        #[ink(message)]
        pub fn xcm_transfer(&mut self, msg: Vec<Instruction<()>>) -> Result<(), XCMError> {
            ink::env::debug_println!("testing xcm_transfer");
            self.env()
                .extension()
                .execute(VersionedXcm::V3(msg.into()))?;

            Ok(())
        }

        #[ink(message)]
        pub fn xcm_send(
            &mut self,
            dest: MultiLocation,
            msg: Vec<Instruction<()>>,
        ) -> Result<(), XCMError> {
            ink::env::debug_println!("testing xcm_send");
            self.env().extension().send(
                VersionedMultiLocation::from(dest),
                VersionedXcm::V3(msg.into()),
            )?;
            Ok(())
        }

        #[ink(message)]
        pub fn xcm_new_query(
            &mut self,
            timeout: u32,
            match_querier: MultiLocation,
        ) -> Result<u64, XCMError> {
            ink::env::debug_println!("testing new_query with {:?}", (timeout, match_querier));
            let query_id = self
                .env()
                .extension()
                .new_query(timeout, VersionedMultiLocation::from(match_querier))?;
            Ok(query_id)
        }

        #[ink(message)]
        pub fn xcm_take_response(
            &mut self,
            query_id: u64,
        ) -> Result<Option<VersionedResponse>, XCMError> {
            ink::env::debug_println!("testing xcm_take_response with {:?}", query_id);
            self.env().extension().take_response(query_id)
        }
    }
}
