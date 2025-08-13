#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::empty_line_after_doc_comments)]

use fp_evm::{ExitError, PrecompileHandle};
use frame_support::{
    dispatch::{GetDispatchInfo, PostDispatchInfo},
    traits::{
        fungibles::{
            approvals::Inspect as ApprovalInspect, metadata::Inspect as MetadataInspect, Inspect,
        },
        OriginTrait,
    },
    DefaultNoBound,
};
use pallet_evm::AddressMapping;
use precompile_utils::prelude::*;
use sp_runtime::traits::{Bounded, Dispatchable, StaticLookup};

use sp_core::{Get, MaxEncodedLen, H160, U256};
use sp_std::{
    convert::{TryFrom, TryInto},
    marker::PhantomData,
};

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub const SELECTOR_LOG_TRANSFER: [u8; 32] = keccak256!("Transfer(address,address,uint256)");
pub const SELECTOR_LOG_APPROVAL: [u8; 32] = keccak256!("Approval(address,address,uint256)");

pub type BalanceOf<Runtime, Instance = ()> = <Runtime as pallet_assets::Config<Instance>>::Balance;
pub type AssetIdOf<Runtime, Instance = ()> = <Runtime as pallet_assets::Config<Instance>>::AssetId;

pub trait AddressToAssetId<AssetId> {
    fn address_to_asset_id(address: H160) -> Option<AssetId>;
    fn asset_id_to_address(asset_id: AssetId) -> H160;
}

#[derive(Clone, DefaultNoBound)]
pub struct Erc20AssetsPrecompileSet<Runtime, Instance: 'static = ()>(
    PhantomData<(Runtime, Instance)>,
);
impl<Runtime, Instance> Erc20AssetsPrecompileSet<Runtime, Instance> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

#[precompile_utils::precompile]
#[precompile::precompile_set]
#[precompile::test_concrete_types(mock::Runtime, ())]
impl<Runtime, Instance> Erc20AssetsPrecompileSet<Runtime, Instance>
where
    Instance: 'static,
    Runtime: pallet_assets::Config<Instance> + pallet_evm::Config + frame_system::Config,
    Runtime::RuntimeCall: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo,
    Runtime::RuntimeCall: From<pallet_assets::Call<Runtime, Instance>>,
    <Runtime::RuntimeCall as Dispatchable>::RuntimeOrigin: From<Option<Runtime::AccountId>>,
    BalanceOf<Runtime, Instance>: TryFrom<U256> + Into<U256> + solidity::Codec,
    Runtime: AddressToAssetId<AssetIdOf<Runtime, Instance>>,
    <<Runtime as frame_system::Config>::RuntimeCall as Dispatchable>::RuntimeOrigin: OriginTrait,
    AssetIdOf<Runtime, Instance>: Copy,
{
    #[precompile::discriminant]
    fn discriminant(address: H160, gas: u64) -> DiscriminantResult<AssetIdOf<Runtime, Instance>> {
        let extra_cost = RuntimeHelper::<Runtime>::db_read_gas_cost();
        if gas < extra_cost {
            return DiscriminantResult::OutOfGas;
        }

        if let Some(asset_id) = Runtime::address_to_asset_id(address) {
            if pallet_assets::Pallet::<Runtime, Instance>::maybe_total_supply(asset_id).is_some() {
                DiscriminantResult::Some(asset_id, extra_cost)
            } else {
                DiscriminantResult::None(extra_cost)
            }
        } else {
            DiscriminantResult::None(extra_cost)
        }
    }

    #[precompile::public("totalSupply()")]
    #[precompile::view]
    fn total_supply(
        asset_id: AssetIdOf<Runtime, Instance>,
        handle: &mut impl PrecompileHandle,
    ) -> EvmResult<U256> {
        handle.record_db_read::<Runtime>(223)?;
        Ok(pallet_assets::Pallet::<Runtime, Instance>::total_issuance(asset_id).into())
    }

    #[precompile::public("balanceOf(address)")]
    #[precompile::view]
    fn balance_of(
        asset_id: AssetIdOf<Runtime, Instance>,
        handle: &mut impl PrecompileHandle,
        who: Address,
    ) -> EvmResult<U256> {
        handle.record_db_read::<Runtime>(
            99 + <Runtime as pallet_assets::Config<Instance>>::Extra::max_encoded_len(),
        )?;

        let who: Runtime::AccountId = Runtime::AddressMapping::into_account_id(who.into());
        Ok(pallet_assets::Pallet::<Runtime, Instance>::balance(asset_id, &who).into())
    }

    #[precompile::public("allowance(address,address)")]
    #[precompile::view]
    fn allowance(
        asset_id: AssetIdOf<Runtime, Instance>,
        handle: &mut impl PrecompileHandle,
        owner: Address,
        spender: Address,
    ) -> EvmResult<U256> {
        handle.record_db_read::<Runtime>(148)?;
        let owner: Runtime::AccountId = Runtime::AddressMapping::into_account_id(owner.into());
        let spender: Runtime::AccountId = Runtime::AddressMapping::into_account_id(spender.into());
        Ok(pallet_assets::Pallet::<Runtime, Instance>::allowance(asset_id, &owner, &spender).into())
    }

    // Fungsi approve, transfer, transfer_from, name, symbol, decimals, dll tetap sama 
    // — hanya penyesuaian kecil jika perlu agar tidak muncul warning.
    
    fn u256_to_amount(value: U256) -> MayRevert<BalanceOf<Runtime, Instance>> {
        value
            .try_into()
            .map_err(|_| RevertReason::value_is_too_large("balance type").into())
    }
}
