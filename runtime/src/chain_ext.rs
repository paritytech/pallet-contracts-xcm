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

use crate::{Config, Error as PalletError};
use codec::{Decode, Encode};
use core::convert::identity;
use frame_support::DefaultNoBound;
use pallet_contracts::chain_extension::{
    ChainExtension, Environment, Ext, InitState, Result, RetVal, SysConfig,
};
use pallet_xcm::{Error as XCMError, WeightInfo};
use xcm::{prelude::*, v3::QueryId};
use xcm_executor::traits::{QueryHandler, QueryResponseStatus};

type CallOf<T> = <T as SysConfig>::RuntimeCall;

/// The commands that the chain extension accepts.
/// See the ink! counterpart for more details.
#[repr(u16)]
#[derive(num_enum::TryFromPrimitive)]
enum Command {
    Execute = 1,
    Send = 2,
    NewQuery = 3,
    TakeResponse = 4,
}

/// The errors that the chain extension can return.
#[repr(u32)]
#[derive(num_enum::IntoPrimitive)]
enum Error {
    /// The Call finished successfully.
    Success = 0,
    /// The XCM query requested via [`Command::TakeResponse`] was not found
    QueryNotFound = 1,
}

/// The input for [`Command::Send`].
#[derive(Decode)]
struct SendInput {
    dest: VersionedMultiLocation,
    msg: VersionedXcm<()>,
}

/// The input for [`Command::NewQuery`].
#[derive(Decode)]
struct NewQueryInput {
    timeout: u32, // TODO use BlockNumberFor<T> when ink_extension supports generics
    match_querier: VersionedMultiLocation,
}

/// The XCM chain extension, that should be use in pallet-contracts configuration, to interact with XCM.
#[derive(DefaultNoBound)]
pub struct XCMExtension<T: Config>(sp_std::marker::PhantomData<T>);

const LOG_TARGET: &'static str = "pallet_contracts_xcm";

impl<T: Config> ChainExtension<T> for XCMExtension<T>
where
    <T as SysConfig>::AccountId: AsRef<[u8; 32]>,
{
    fn call<E>(&mut self, env: Environment<E, InitState>) -> Result<RetVal>
    where
        E: Ext<T = T>,
    {
        log::debug!(target: LOG_TARGET, "Start call");
        match Command::try_from(env.func_id()).map_err(|_| PalletError::<T>::InvalidCommand)? {
            // Execute an XCM message locally, using the contract's address as the origin.
            Command::Execute => {
                log::debug!(target: LOG_TARGET, "Execute XCM message");
                let mut env = env.buf_in_buf_out();

                let message: VersionedXcm<CallOf<T>> = env.read_as_unbounded(env.in_len())?;
                let message = Box::new(message);
                let origin = frame_system::Origin::<T>::Signed(env.ext().address().clone());
                let max_weight = env.ext().gas_meter().gas_left();

                let charged = env.charge_weight(<T as pallet_xcm::Config>::WeightInfo::execute())?;
                let res = pallet_xcm::Pallet::<T>::execute(origin.into(), message, max_weight);
                if let Some(weight) = res.map_or_else(|res| res.post_info, identity).actual_weight {
                    env.adjust_weight(charged, weight);
                }

                res.map_err(|err| err.error)?;
            }

            // Send an XCM message from the contract to the specified destination.
            Command::Send => {
                log::debug!(target: LOG_TARGET, "Send XCM message");
                let mut env = env.buf_in_buf_out();
                let origin = frame_system::Origin::<T>::Signed(env.ext().address().clone());
                let SendInput { dest, msg } = env.read_as_unbounded(env.in_len())?;
                env.charge_weight(<T as pallet_xcm::Config>::WeightInfo::send())?;
                pallet_xcm::Pallet::<T>::send(origin.into(), Box::new(dest), Box::new(msg))?;
            }

            // Create a new query, using the contract's address as the responder.
            Command::NewQuery => {
                log::debug!(target: LOG_TARGET, "Create new XCM query");
                let mut env = env.buf_in_buf_out();
                let len = env.in_len();
                let NewQueryInput {
                    timeout,
                    match_querier,
                }: NewQueryInput = env.read_as_unbounded(len)?;

                let responder = MultiLocation {
                    parents: 0,
                    interior: Junctions::X1(Junction::AccountId32 {
                        network: None,
                        id: *env.ext().address().as_ref(),
                    }),
                };

                // TODO Charge weight

                let query_id = <pallet_xcm::Pallet<T> as QueryHandler>::new_query(
                    responder,
                    timeout.into(),
                    MultiLocation::try_from(match_querier)
                        .map_err(|_| XCMError::<T>::BadVersion)?,
                );

                log::debug!(target: LOG_TARGET, "new query id {query_id}");
                query_id.using_encoded(|q| env.write(q, true, None))?;
            }

            // Attempt to take a response for the specified query.
            Command::TakeResponse => {
                log::debug!(target: LOG_TARGET, "Take XCM query response");
                let mut env = env.buf_in_buf_out();
                let query_id: QueryId = env.read_as()?;
                let response = <pallet_xcm::Pallet<T> as QueryHandler>::take_response(query_id);

                let response = match response {
                    QueryResponseStatus::Ready { response, .. } => {
                        Some(VersionedResponse::from(response))
                    }
                    QueryResponseStatus::Pending { .. } => None,
                    QueryResponseStatus::UnexpectedVersion => Err(XCMError::<T>::BadVersion)?,
                    QueryResponseStatus::NotFound => {
                        return Ok(RetVal::Converging(Error::QueryNotFound.into()));
                    }
                };
                response.using_encoded(|q| env.write(q, true, None))?;
            }
        }

        log::debug!(target: LOG_TARGET, "Call done");
        Ok(RetVal::Converging(Error::Success.into()))
    }
}
