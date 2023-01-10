use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::H160;
use sp_runtime::{traits::ConstU32, BoundedVec};
use xcm::VersionedMultiLocation;

#[derive(Encode, Decode, Clone, PartialEq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Router<CurrencyId> {
	// An XCM-based router
	Xcm(XcmDomain<CurrencyId>),
}

/// XcmDomain gathers all the required fields to build and send remote
/// calls to a specific XCM-based Domain.
#[derive(Encode, Decode, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct XcmDomain<CurrencyId> {
	/// the xcm multilocation of the domain
	pub location: VersionedMultiLocation,
	/// The ethereum_xcm::Call::transact call index on a given domain.
	/// It should contain the pallet index + the `transact` call index, to which
	/// we will append the eth_tx param. You can obtain this value by building
	/// an ethereum_xcm::transact call with Polkadot JS on the target chain.
	pub ethereum_xcm_transact_call_index:
		BoundedVec<u8, ConstU32<{ xcm_primitives::MAX_ETHEREUM_XCM_INPUT_SIZE }>>,
	/// The ConnectorsXcmRouter contract address on a given domain
	pub contract_address: H160,
	/// The currency in which execution fees will be paid on
	pub fee_currency: CurrencyId,
}

// NOTE: Remove this custom implementation once the following underlying data implements MaxEncodedLen:
/// * Polkadot Repo: xcm::VersionedMultiLocation
/// * PureStake Repo: pallet_xcm_transactor::Config<Self = T>::CurrencyId
impl<CurrencyId> MaxEncodedLen for XcmDomain<CurrencyId>
where
	XcmDomain<CurrencyId>: Encode,
{
	fn max_encoded_len() -> usize {
		// custom MEL bound for `VersionedMultiLocation`
		xcm::v1::MultiLocation::max_encoded_len()
			// VersionedMultiLocation is enum with two variants
			.saturating_add(2)
			.saturating_add(BoundedVec::<
				u8,
				ConstU32<{ xcm_primitives::MAX_ETHEREUM_XCM_INPUT_SIZE }>,
			>::max_encoded_len())
			// custom MEL bound for CurrencyId
			.saturating_add(cfg_types::tokens::CurrencyId::max_encoded_len())
			.saturating_add(H160::max_encoded_len())
	}
}
