use crate::{Config, Error as PalletError};
use codec::{Decode, Encode};
use frame_support::weights::Weight;
use pallet_contracts::chain_extension::{
	ChainExtension, Environment, Ext, InitState, RegisteredChainExtension, Result, RetVal,
	SysConfig, UncheckedFrom,
};
use sp_runtime::traits::Bounded;
use xcm::prelude::*;
use xcm_executor::traits::WeightBounds;

type CallOf<T> = <T as SysConfig>::Call;

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
struct PrepareExecuteInput<Call> {
	dest: VersionedMultiLocation,
	xcm: VersionedXcm<Call>,
}

#[derive(Decode)]
struct ValidateSendInput {
	dest: VersionedMultiLocation,
	xcm: VersionedXcm<()>,
}

pub struct PreparedExecution<Call> {
	dest: MultiLocation,
	xcm: Xcm<Call>,
	weight: Weight,
}

pub struct ValidatedSend {
	dest: MultiLocation,
	xcm: Xcm<()>,
}

#[derive(Default)]
struct XcmExtension<T: Config> {
	prepared_execute: Option<PreparedExecution<CallOf<T>>>,
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

impl<T: Config> ChainExtension<T> for XcmExtension<T>
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
				let input: PrepareExecuteInput<CallOf<T>> = env.read_as_unbounded(len)?;
				let mut xcm =
					input.xcm.try_into().map_err(|_| PalletError::<T>::XcmVersionNotSupported)?;
				let weight =
					T::Weigher::weight(&mut xcm).map_err(|_| PalletError::<T>::CannotWeigh)?;
				self.prepared_execute = Some(PreparedExecution {
					dest: input
						.dest
						.try_into()
						.map_err(|_| PalletError::<T>::XcmVersionNotSupported)?,
					xcm,
					weight,
				});
				weight.using_encoded(|w| env.write(w, true, None))?;
			},
			Command::Execute => {
				let input =
					self.prepared_execute.take().ok_or(PalletError::<T>::PreparationMissing)?;
				env.charge_weight(input.weight)?;
				T::XcmExecutor::execute_xcm_in_credit(
					input.dest,
					input.xcm,
					input.weight,
					input.weight,
				);
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
					.map_err(|_| PalletError::<T>::SendFailed)?;
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
				let query_id = pallet_xcm::Pallet::<T>::new_query(location, Bounded::max_value());
				query_id.using_encoded(|q| env.write(q, true, None))?;
			},
			Command::TakeResponse => {
				let mut env = env.prim_in_buf_out();
				let query_id = (env.val0() as u64) | ((env.val1() as u64) << 32);
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

impl<T: Config> RegisteredChainExtension<T> for XcmExtension<T>
where
	<T as SysConfig>::AccountId: AsRef<[u8; 32]>,
{
	const ID: u16 = 1;
}