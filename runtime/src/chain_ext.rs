use crate::{Config, Error as PalletError};
use codec::{Decode, Encode};
use frame_support::{weights::Weight, DefaultNoBound};
use pallet_contracts::chain_extension::{
	ChainExtension, Environment, Ext, InitState, RegisteredChainExtension, Result, RetVal,
	SysConfig, UncheckedFrom,
};
use sp_runtime::traits::Bounded;
use xcm::prelude::*;
use xcm_executor::traits::WeightBounds;
use log;

type RuntimeCallOf<T> = <T as SysConfig>::RuntimeCall;

#[repr(u16)]
#[derive(num_enum::TryFromPrimitive)]
enum Command {
	PrepareExecute = 0,
	Execute = 1,
	ValidateSend = 2,
	Send = 3,
	NewQuery = 4,
	TakeResponse = 5,
}

#[repr(u32)]
#[derive(num_enum::IntoPrimitive)]
enum Error {
	Success = 0,
	NoResponse = 1,
}

#[derive(Decode)]
struct ValidateSendInput {
	dest: VersionedMultiLocation,
	xcm: VersionedXcm<()>,
}

pub struct PreparedExecution<Call> {
	xcm: Xcm<Call>,
	weight: Weight,
}

pub struct ValidatedSend {
	dest: MultiLocation,
	xcm: Xcm<()>,
}

#[derive(DefaultNoBound)]
pub struct Extension<T: Config> {
	prepared_execute: Option<PreparedExecution<RuntimeCallOf<T>>>,
	validated_send: Option<ValidatedSend>,
}

macro_rules! unwrap {
	($val:expr, $err:expr) => {
		match $val {
			Ok(inner) => inner,
			Err(_) => return Ok(RetVal::Converging($err.into())),
		}
	};
}

impl<T: Config> ChainExtension<T> for Extension<T>
where
	<T as SysConfig>::AccountId: AsRef<[u8; 32]>,
{
	fn call<E>(&mut self, mut env: Environment<E, InitState>) -> Result<RetVal>
	where
		E: Ext<T = T>,
		<E::T as SysConfig>::AccountId: UncheckedFrom<<E::T as SysConfig>::Hash> + AsRef<[u8]>,
	{
		match Command::try_from(env.func_id()).map_err(|_| PalletError::<T>::InvalidCommand)? {
			Command::PrepareExecute => {
				let mut env = env.buf_in_buf_out();
				let len = env.in_len();
				let input: VersionedXcm<RuntimeCallOf<T>> = env.read_as_unbounded(len)?;
				let mut xcm =
					input.try_into().map_err(|_| PalletError::<T>::XcmVersionNotSupported)?;
				let weight =
					Weight::from_ref_time(T::Weigher::weight(&mut xcm).map_err(|_| PalletError::<T>::CannotWeigh)?);
				self.prepared_execute = Some(PreparedExecution { xcm, weight });
				weight.using_encoded(|w| env.write(w, true, None))?;
			},
			Command::Execute => {
				let input =
					self.prepared_execute.take().ok_or(PalletError::<T>::PreparationMissing)?;
				env.charge_weight(input.weight)?;
				let origin = MultiLocation {
					parents: 0,
					interior: Junctions::X1(Junction::AccountId32 {
						network: NetworkId::Any,
						id: *env.ext().address().as_ref(),
					}),
				};
				let outcome = T::XcmExecutor::execute_xcm_in_credit(
					origin,
					input.xcm,
					input.weight.ref_time(),
					input.weight.ref_time(),
				);
				// revert for anything but a complete excution
				match outcome {
					Outcome::Complete(_) => (),
					_ => Err(PalletError::<T>::ExecutionFailed)?,
				}
			},
			Command::ValidateSend => {
				let mut env = env.buf_in_buf_out();
				let len = env.in_len();
				let input: ValidateSendInput = env.read_as_unbounded(len)?;
				self.validated_send = Some(ValidatedSend {
					dest: input
						.dest
						.try_into()
						.map_err(|_| PalletError::<T>::XcmVersionNotSupported)?,
					xcm: input
						.xcm
						.try_into()
						.map_err(|_| PalletError::<T>::XcmVersionNotSupported)?,
				});
				// just a dummy asset until XCMv3 rolls around with its validate function
				let asset = MultiAsset {
					id: AssetId::Concrete(MultiLocation { parents: 0, interior: Junctions::Here }),
					fun: Fungibility::Fungible(0),
				};
				VersionedMultiAsset::from(asset).using_encoded(|a| env.write(a, true, None))?;
			},
			Command::Send => {
				let input =
					self.validated_send.take().ok_or(PalletError::<T>::PreparationMissing)?;
				T::XcmRouter::send_xcm(input.dest, input.xcm)
					.map_err(|e| {
						log::debug!(
							target: "Contracts",
							"Send Failed: {:?}",
							e
						);
						PalletError::<T>::SendFailed
					})?;
			},
			Command::NewQuery => {
				let mut env = env.buf_in_buf_out();
				let location = MultiLocation {
					parents: 0,
					interior: Junctions::X1(Junction::AccountId32 {
						network: NetworkId::Any,
						id: *env.ext().address().as_ref(),
					}),
				};
				let query_id: u64 =
					pallet_xcm::Pallet::<T>::new_query(location, Bounded::max_value()).into();
				query_id.using_encoded(|q| env.write(q, true, None))?;
			},
			Command::TakeResponse => {
				let mut env = env.buf_in_buf_out();
				let query_id: u64 = env.read_as()?;
				let response = unwrap!(
					pallet_xcm::Pallet::<T>::take_response(query_id).map(|ret| ret.0).ok_or(()),
					Error::NoResponse
				);
				VersionedResponse::from(response).using_encoded(|r| env.write(r, true, None))?;
			},
		}

		Ok(RetVal::Converging(Error::Success.into()))
	}
}

impl<T: Config> RegisteredChainExtension<T> for Extension<T>
where
	<T as SysConfig>::AccountId: AsRef<[u8; 32]>,
{
	const ID: u16 = 1;
}
