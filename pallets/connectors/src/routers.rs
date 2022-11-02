use codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_core::H160;
use sp_std::vec::Vec;
use xcm::VersionedMultiLocation;

#[derive(Encode, Decode, Clone, PartialEq, TypeInfo)]
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
	pub ethereum_xcm_transact_call_index: Vec<u8>,
	/// The ConnectorsXcmRouter contract address on a given domain
	pub contract_address: H160,
	/// The currency in which execution fees will be paid on
	pub fee_currency: CurrencyId,
}
