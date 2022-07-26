#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::weights::Weight;
use pallet_contracts::chain_extension::{
    ChainExtension, Config as PalletConfig, Environment, Ext, InitState, RegisteredChainExtension,
    Result, RetVal, SysConfig, UncheckedFrom,
};
use xcm::{
    latest::{
        AssetId, ExecuteXcm, Fungibility, Junctions, MultiAsset, MultiLocation, SendXcm, Xcm,
    },
    VersionedMultiAsset, VersionedMultiLocation, VersionedXcm,
};
use xcm_executor::traits::WeightBounds;

type CallOf<C> = <C as SysConfig>::Call;

trait Config: PalletConfig {
    type XcmRouter: SendXcm;
    type XcmExecutor: ExecuteXcm<CallOf<Self>>;
    type XcmWeigher: WeightBounds<CallOf<Self>>;
}

#[repr(u16)]
#[derive(num_enum::TryFromPrimitive)]
enum Command {
    PrepareExecute = 0,
    Execute = 1,
    ValidateSend = 2,
    Send = 3,
}

#[repr(u32)]
#[derive(num_enum::IntoPrimitive)]
enum Error {
    Success = 0,
    InvalidCommand = 1,
    XcmVersionNotSupported = 2,
    CannotWeigh = 3,
    NoPrep = 4,
    SendFailed = 5,
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

struct PreparedExecution<Call> {
    dest: MultiLocation,
    xcm: Xcm<Call>,
    weight: Weight,
}

struct ValidatedSend {
    dest: MultiLocation,
    xcm: Xcm<()>,
}

#[derive(Default)]
struct XcmExtension<C: Config> {
    prepared_execute: Option<PreparedExecution<CallOf<C>>>,
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

impl<C: Config> ChainExtension<C> for XcmExtension<C> {
    fn call<E>(&mut self, mut env: Environment<E, InitState>) -> Result<RetVal>
    where
        E: Ext<T = C>,
        <E::T as SysConfig>::AccountId: UncheckedFrom<<E::T as SysConfig>::Hash> + AsRef<[u8]>,
    {
        match unwrap!(Command::try_from(env.func_id()), Error::InvalidCommand) {
            Command::PrepareExecute => {
                let mut env = env.buf_in_buf_out();
                let len = env.in_len();
                let input: PrepareExecuteInput<CallOf<C>> = env.read_as_unbounded(len)?;
                let mut xcm = unwrap!(input.xcm.try_into(), Error::XcmVersionNotSupported);
                let weight = unwrap!(C::XcmWeigher::weight(&mut xcm), Error::CannotWeigh);
                self.prepared_execute = Some(PreparedExecution {
                    dest: unwrap!(input.dest.try_into(), Error::XcmVersionNotSupported),
                    xcm,
                    weight,
                });
                env.write(&weight.encode(), true, None)?;
            }
            Command::Execute => {
                let input = unwrap!(self.prepared_execute.take().ok_or(()), Error::NoPrep);
                env.charge_weight(input.weight)?;
                C::XcmExecutor::execute_xcm_in_credit(
                    input.dest,
                    input.xcm,
                    input.weight,
                    input.weight,
                );
            }
            Command::ValidateSend => {
                let mut env = env.buf_in_buf_out();
                let len = env.in_len();
                let input: ValidateSendInput = env.read_as_unbounded(len)?;
                self.validated_send = Some(ValidatedSend {
                    dest: unwrap!(input.dest.try_into(), Error::XcmVersionNotSupported),
                    xcm: unwrap!(input.xcm.try_into(), Error::XcmVersionNotSupported),
                });
                // just a dummy asset until XCMv3 rolls around with its validate function
                let asset = MultiAsset {
                    id: AssetId::Concrete(MultiLocation {
                        parents: 0,
                        interior: Junctions::Here,
                    }),
                    fun: Fungibility::Fungible(0),
                };
                env.write(
                    VersionedMultiAsset::from(asset).encode().as_ref(),
                    true,
                    None,
                )?;
            }
            Command::Send => {
                let input = unwrap!(self.validated_send.take().ok_or(()), Error::NoPrep);
                unwrap!(
                    C::XcmRouter::send_xcm(input.dest, input.xcm),
                    Error::SendFailed
                );
            }
        }

        Ok(RetVal::Converging(Error::Success.into()))
    }
}

impl<C: Config> RegisteredChainExtension<C> for XcmExtension<C> {
    const ID: u16 = 1;
}
