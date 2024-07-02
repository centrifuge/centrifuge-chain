//! A message requires a custom decoding & encoding, meeting the
//! LiquidityPool Generic Message Passing Format (GMPF): Every message is
//! encoded with a u8 at head flagging the message type, followed by its field.
//! Integers are big-endian encoded and enum values (such as `[crate::Domain]`)
//! also have a custom GMPF implementation, aiming for a fixed-size encoded
//! representation for each message variant.

use cfg_traits::{liquidity_pools::LPEncoding, Seconds};
use cfg_types::domain_address::Domain;
use frame_support::pallet_prelude::RuntimeDebug;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_runtime::DispatchError;
use sp_std::vec::Vec;

use crate::data_format as gmpf; // Generic Message Passing Format

/// Address type
/// Note: It can be used to represent any address type with a length <= 32
/// bytes; For example, it can represent an Ethereum address (20-bytes long) by
/// padding it with 12 zeros.
type Address = [u8; 32];

type TrancheId = [u8; 16];

/// The fixed size for the array representing a tranche token name
pub const TOKEN_NAME_SIZE: usize = 128;

// The fixed size for the array representing a tranche token symbol
pub const TOKEN_SYMBOL_SIZE: usize = 32;

/// An isometric type to `Domain` that serializes as expected
#[derive(
	Encode,
	Decode,
	Serialize,
	Deserialize,
	Clone,
	PartialEq,
	Eq,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
)]
pub struct SerializableDomain(u8, u64);

impl From<Domain> for SerializableDomain {
	fn from(domain: Domain) -> Self {
		match domain {
			Domain::Centrifuge => Self(0, 0),
			Domain::EVM(chain_id) => Self(1, chain_id),
		}
	}
}

impl TryInto<Domain> for SerializableDomain {
	type Error = DispatchError;

	fn try_into(self) -> Result<Domain, DispatchError> {
		match self.0 {
			0 => Ok(Domain::Centrifuge),
			1 => Ok(Domain::EVM(self.1)),
			_ => Err(DispatchError::Other("Unknown domain")),
		}
	}
}

/// A LiquidityPools Message
#[derive(
	Encode,
	Decode,
	Serialize,
	Deserialize,
	Clone,
	PartialEq,
	Eq,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
)]
pub enum Message {
	Invalid,
	/// Add a currency to a domain, i.e, register the mapping of a currency id
	/// to the corresponding EVM Address.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	AddCurrency {
		currency: u128,
		evm_address: [u8; 20],
	},
	/// Add a pool to a domain.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	AddPool {
		pool_id: u64,
	},
	/// Allow a currency to be used as a pool currency and to invest in a pool.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	AllowInvestmentCurrency {
		pool_id: u64,
		currency: u128,
	},
	/// Add a tranche to an already existing pool on the target domain.
	/// The decimals of a tranche MUST be equal to the decimals of a pool.
	/// Thus, consuming domains MUST take care of storing the decimals upon
	/// receiving an AddPool message.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	AddTranche {
		pool_id: u64,
		tranche_id: TrancheId,
		#[serde(with = "serde_big_array::BigArray")]
		token_name: [u8; TOKEN_NAME_SIZE],
		token_symbol: [u8; TOKEN_SYMBOL_SIZE],
		decimals: u8,
		/// The RestrictionManager implementation to be used for this tranche
		/// token on the domain it will be added and subsequently deployed in.
		restriction_set: u8,
	},
	/// Update the price of a tranche token on the target domain.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	UpdateTrancheTokenPrice {
		pool_id: u64,
		tranche_id: TrancheId,
		currency: u128,
		price: u128,
		/// The timestamp at which the price was computed
		computed_at: Seconds,
	},
	/// Whitelist an address for the specified pair of pool and tranche token on
	/// the target domain.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	UpdateMember {
		pool_id: u64,
		tranche_id: TrancheId,
		member: Address,
		valid_until: Seconds,
	},
	/// Transfer non-tranche tokens fungibles. For v2, it will only support
	/// stable-coins.
	///
	/// Directionality: Centrifuge <-> EVM Domain.
	///
	/// NOTE: Receiving domain must not accept every incoming token.
	/// For Centrifuge -> EVM Domain: `AddCurrency` should have been called
	/// beforehand. For Centrifuge <- EVM Domain: We can assume `AddCurrency`
	/// has been called for that domain already.
	Transfer {
		currency: u128,
		sender: Address,
		receiver: Address,
		amount: u128,
	},
	/// Transfer tranche tokens between domains.
	///
	/// Directionality: Centrifuge <-> EVM Domain.
	TransferTrancheTokens {
		pool_id: u64,
		tranche_id: TrancheId,
		sender: Address,
		domain: SerializableDomain,
		receiver: Address,
		amount: u128,
	},
	/// Increase the invest order amount for the specified pair of pool and
	/// tranche token.
	///
	/// Directionality: Centrifuge <- EVM Domain.
	IncreaseInvestOrder {
		pool_id: u64,
		tranche_id: TrancheId,
		investor: Address,
		currency: u128,
		amount: u128,
	},
	/// Reduce the invest order amount for the specified pair of pool and
	/// tranche token.
	///
	/// On success, triggers a message sent back to the sending domain.
	/// The message will take care of re-funding the investor with the given
	/// amount the order was reduced with. The `investor` address is used as
	/// the receiver of that tokens.
	///
	/// Directionality: Centrifuge <- EVM Domain.
	DecreaseInvestOrder {
		pool_id: u64,
		tranche_id: TrancheId,
		investor: Address,
		currency: u128,
		amount: u128,
	},
	/// Increase the redeem order amount for the specified pair of pool and
	/// tranche token.
	///
	/// Directionality: Centrifuge <- EVM Domain.
	IncreaseRedeemOrder {
		pool_id: u64,
		tranche_id: TrancheId,
		investor: Address,
		currency: u128,
		amount: u128,
	},
	/// Reduce the redeem order amount for the specified pair of pool and
	/// tranche token.
	///
	/// On success, triggers a message sent back to the sending domain.
	/// The message will take care of re-funding the investor with the given
	/// amount the order was reduced with. The `investor` address is used as
	/// the receiver of that tokens.
	///
	/// Directionality: Centrifuge <- EVM Domain.
	DecreaseRedeemOrder {
		pool_id: u64,
		tranche_id: TrancheId,
		investor: Address,
		currency: u128,
		amount: u128,
	},
	/// Collect the investment for the specified pair of pool and
	/// tranche token.
	///
	/// On success, triggers a message sent back to the sending domain.
	/// The message will take care of re-funding the investor with the given
	/// amount the order was reduced with. The `investor` address is used as
	/// the receiver of that tokens.
	///
	/// Directionality: Centrifuge <- EVM Domain.
	CollectInvest {
		pool_id: u64,
		tranche_id: TrancheId,
		investor: Address,
		currency: u128,
	},
	/// Collect the proceeds for the specified pair of pool and
	/// tranche token.
	///
	/// On success, triggers a message sent back to the sending domain.
	/// The message will take care of re-funding the investor with the given
	/// amount the order was reduced with. The `investor` address is used as
	/// the receiver of that tokens.
	///
	/// Directionality: Centrifuge <- EVM Domain.
	CollectRedeem {
		pool_id: u64,
		tranche_id: TrancheId,
		investor: Address,
		currency: u128,
	},
	/// The message sent back to the domain from which a `DecreaseInvestOrder`
	/// message was received, ensuring the correct state update on said domain
	/// and that the `investor`'s wallet is updated accordingly.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	ExecutedDecreaseInvestOrder {
		/// The pool id
		pool_id: u64,
		/// The tranche id
		tranche_id: TrancheId,
		/// The investor's address
		investor: Address,
		/// The currency in which `DecreaseInvestOrder` was realised
		currency: u128,
		/// The amount of `currency` that was actually executed in the original
		/// `DecreaseInvestOrder` message, i.e., the amount by which the
		/// investment order was actually decreased by.
		currency_payout: u128,
		/// The remaining investment amount denominated in the `foreign` payment
		/// currency. It reflects the sum of the unprocessed as well as the
		/// processed but not yet collected amounts.
		remaining_invest_amount: u128,
	},
	/// The message sent back to the domain from which a `DecreaseRedeemOrder`
	/// message was received, ensuring the correct state update on said domain
	/// and that the `investor`'s wallet is updated accordingly.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	ExecutedDecreaseRedeemOrder {
		/// The pool id
		pool_id: u64,
		/// The tranche id
		tranche_id: TrancheId,
		/// The investor's address
		investor: Address,
		/// The currency in which `DecreaseRedeemOrder` was realised
		currency: u128,
		/// The amount of tranche tokens that was actually executed in the
		/// original `DecreaseRedeemOrder` message, i.e., the amount by which
		/// the redeem order was actually decreased by.
		tranche_tokens_payout: u128,
		/// The remaining redemption amount. It reflects the sum of the
		/// unprocessed as well as the processed but not yet collected amount of
		/// tranche tokens.
		remaining_redeem_amount: u128,
	},
	/// The message sent back to the domain from which a `CollectInvest` message
	/// has been received, which will ensure the `investor` gets the payout
	/// respective to their investment.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	ExecutedCollectInvest {
		/// The pool id
		pool_id: u64,
		/// The tranche
		tranche_id: TrancheId,
		/// The investor's address
		investor: Address,
		/// The currency in which the investment was realised
		currency: u128,
		/// The amount that was actually collected, in `currency` units
		currency_payout: u128,
		/// The amount of tranche tokens received for the investment made
		tranche_tokens_payout: u128,
		/// The remaining investment amount denominated in the `foreign` payment
		/// currency. It reflects the sum of the unprocessed as well as the
		/// processed but not yet collected amounts.
		remaining_invest_amount: u128,
	},
	/// The message sent back to the domain from which a `CollectRedeem` message
	/// has been received, which will ensure the `investor` gets the payout
	/// respective to their redemption.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	ExecutedCollectRedeem {
		/// The pool id
		pool_id: u64,
		/// The tranche id
		tranche_id: TrancheId,
		/// The investor's address
		investor: Address,
		/// The stable coin currency in which the payout takes place
		currency: u128,
		/// The amount of `currency` being paid out to the investor
		currency_payout: u128,
		/// How many tranche tokens were actually redeemed
		tranche_tokens_payout: u128,
		/// The remaining redemption amount. It reflects the sum of the
		/// unprocessed as well as the processed but not yet collected amount of
		/// tranche tokens.
		remaining_redeem_amount: u128,
	},
	/// Cancel an unprocessed invest order for the specified pair of pool and
	/// tranche token.
	///
	/// Special instance of `DecreaseInvestOrder` where the amount is chosen
	/// properly to cancel out the ongoing investment. Required for ERC4646.
	///
	/// On success, triggers a message sent back to the sending domain.
	/// The message will take care of re-funding the investor with the given
	/// amount the order was reduced with. The `investor` address is used as
	/// the receiver of that tokens.
	///
	/// Directionality: Centrifuge <- EVM Domain.
	CancelInvestOrder {
		pool_id: u64,
		tranche_id: TrancheId,
		investor: Address,
		currency: u128,
	},
	/// Reduce the redeem order amount for the specified pair of pool and
	/// tranche token.
	///
	/// Special instance of `DecreaseRedeemOrder` where the amount is chosen
	/// properly to cancel out the ongoing redemption. Required for ERC4646.
	///
	/// On success, triggers a message sent back to the sending domain.
	/// The message will take care of re-funding the investor with the given
	/// amount the order was reduced with. The `investor` address is used as
	/// the receiver of that tokens.
	///
	/// Directionality: Centrifuge <- EVM Domain.
	CancelRedeemOrder {
		pool_id: u64,
		tranche_id: TrancheId,
		investor: Address,
		currency: u128,
	},
	/// Schedules an EVM address to become rely-able by the gateway. Intended to
	/// be used via governance to execute EVM spells.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	ScheduleUpgrade {
		/// The EVM contract address
		contract: [u8; 20],
	},
	/// Cancel the scheduled process for an EVM address to become rely-able by
	/// the gateway. Intended to be used via governance to execute EVM spells.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	CancelUpgrade {
		/// The EVM contract address
		contract: [u8; 20],
	},
	/// Updates the name and symbol of a tranche token.
	///
	/// NOTE: We do not allow updating the decimals as this would require
	/// migrating all associated balances.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	UpdateTrancheTokenMetadata {
		pool_id: u64,
		tranche_id: TrancheId,
		#[serde(with = "serde_big_array::BigArray")]
		token_name: [u8; TOKEN_NAME_SIZE],
		token_symbol: [u8; TOKEN_SYMBOL_SIZE],
	},
	/// Disallow a currency to be used as a pool currency and to invest in a
	/// pool.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	DisallowInvestmentCurrency {
		pool_id: u64,
		currency: u128,
	},
}

impl LPEncoding for Message {
	fn serialize(&self) -> Vec<u8> {
		gmpf::to_vec(self).unwrap_or_default()
	}

	fn deserialize(data: &[u8]) -> Result<Self, DispatchError> {
		gmpf::from_slice(data).map_err(|_| DispatchError::Other("LP Deserialization issue"))
	}
}

#[cfg(test)]
mod tests {
	use cfg_primitives::{Balance, PoolId, TrancheId};
	use cfg_types::fixed_point::Ratio;
	use cfg_utils::vec_to_fixed_array;
	use hex::FromHex;
	use sp_runtime::{traits::One, FixedPointNumber};

	use super::*;
	use crate::{Domain, DomainAddress};

	const AMOUNT: Balance = 100000000000000000000000000;
	const POOL_ID: PoolId = 12378532;
	const TOKEN_ID: u128 = 246803579;

	#[test]
	fn invalid() {
		let msg = Message::Invalid;
		assert_eq!(gmpf::to_vec(&msg).unwrap(), vec![0]);
	}

	#[test]
	fn encoding_domain() {
		// The Centrifuge substrate chain
		assert_eq!(
			hex::encode(gmpf::to_vec(&SerializableDomain::from(Domain::Centrifuge)).unwrap()),
			"000000000000000000"
		);
		// Ethereum MainNet
		assert_eq!(
			hex::encode(gmpf::to_vec(&SerializableDomain::from(Domain::EVM(1))).unwrap()),
			"010000000000000001"
		);
		// Moonbeam EVM chain
		assert_eq!(
			hex::encode(gmpf::to_vec(&SerializableDomain::from(Domain::EVM(1284))).unwrap()),
			"010000000000000504"
		);
		// Avalanche Chain
		assert_eq!(
			hex::encode(gmpf::to_vec(&SerializableDomain::from(Domain::EVM(43114))).unwrap()),
			"01000000000000a86a"
		);
	}

	#[test]
	fn add_currency_zero() {
		test_encode_decode_identity(
			Message::AddCurrency {
				currency: 0,
				evm_address: default_address_20(),
			},
			"01000000000000000000000000000000001231231231231231231231231231231231231231",
		)
	}

	#[test]
	fn add_currency() {
		test_encode_decode_identity(
			Message::AddCurrency {
				currency: TOKEN_ID,
				evm_address: default_address_20(),
			},
			"010000000000000000000000000eb5ec7b1231231231231231231231231231231231231231",
		)
	}

	#[test]
	fn add_pool_zero() {
		test_encode_decode_identity(Message::AddPool { pool_id: 0 }, "020000000000000000")
	}

	#[test]
	fn add_pool_long() {
		test_encode_decode_identity(Message::AddPool { pool_id: POOL_ID }, "020000000000bce1a4")
	}

	#[test]
	fn allow_investment_currency() {
		test_encode_decode_identity(
			Message::AllowInvestmentCurrency {
				currency: TOKEN_ID,
				pool_id: POOL_ID,
			},
			"030000000000bce1a40000000000000000000000000eb5ec7b",
		)
	}

	#[test]
	fn allow_investment_currency_zero() {
		test_encode_decode_identity(
			Message::AllowInvestmentCurrency {
				currency: 0,
				pool_id: 0,
			},
			"03000000000000000000000000000000000000000000000000",
		)
	}

	#[test]
	fn add_tranche() {
		test_encode_decode_identity(
			Message::AddTranche {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				token_name: vec_to_fixed_array(b"Some Name"),
				token_symbol: vec_to_fixed_array( b"SYMBOL"),
				decimals: 15,
				restriction_set: 1,
			},
			"040000000000000001811acd5b3f17c06841c7e41e9e04cb1b536f6d65204e616d65000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000053594d424f4c00000000000000000000000000000000000000000000000000000f01",
		)
	}

	#[test]
	fn update_tranche_token_price() {
		test_encode_decode_identity(
			Message::UpdateTrancheTokenPrice {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				currency: TOKEN_ID,
				price: Ratio::one().into_inner(),
				computed_at: 1698131924,
			},
			"050000000000000001811acd5b3f17c06841c7e41e9e04cb1b0000000000000000000000000eb5ec7b00000000000000000de0b6b3a76400000000000065376fd4",
		)
	}

	#[test]
	fn update_member() {
		test_encode_decode_identity(
			Message::UpdateMember {
					pool_id: 2,
					tranche_id: default_tranche_id(),
					member: default_address_32(),
					valid_until: 1706260138,
				},
			"060000000000000002811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640000000065b376aa"
			)
	}

	#[test]
	fn transfer_to_evm_address() {
		test_encode_decode_identity(
			Message::Transfer {
					currency: TOKEN_ID,
					sender: default_address_32(),
					receiver: vec_to_fixed_array(default_address_20()),
					amount: AMOUNT,
				},
			"070000000000000000000000000eb5ec7b45645645645645645645645645645645645645645645645645645645645645641231231231231231231231231231231231231231000000000000000000000000000000000052b7d2dcc80cd2e4000000"
			);
	}

	#[test]
	fn transfer_to_centrifuge() {
		test_encode_decode_identity(
			Message::Transfer {
        			currency: TOKEN_ID,
					sender: vec_to_fixed_array(default_address_20()),
					receiver: default_address_32(),
					amount: AMOUNT,
				},
			"070000000000000000000000000eb5ec7b12312312312312312312312312312312312312310000000000000000000000004564564564564564564564564564564564564564564564564564564564564564000000000052b7d2dcc80cd2e4000000"
			);
	}

	#[test]
	fn transfer_tranche_tokens_to_moonbeam() {
		let domain_address = DomainAddress::EVM(1284, default_address_20());

		test_encode_decode_identity(
			Message::TransferTrancheTokens {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				sender: default_address_32(),
				domain: domain_address.domain().into(),
				receiver: domain_address.address(),
				amount: AMOUNT,
			},
			"080000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640100000000000005041231231231231231231231231231231231231231000000000000000000000000000000000052b7d2dcc80cd2e4000000"
		);
	}

	#[test]
	fn transfer_tranche_tokens_to_centrifuge() {
		test_encode_decode_identity(
			Message::TransferTrancheTokens {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				sender: vec_to_fixed_array(default_address_20()),
				domain: Domain::Centrifuge.into(),
				receiver: default_address_32(),
				amount: AMOUNT,
			},
			"080000000000000001811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000000000000000000000004564564564564564564564564564564564564564564564564564564564564564000000000052b7d2dcc80cd2e4000000"
		)
	}

	#[test]
	fn increase_invest_order() {
		test_encode_decode_identity(
			Message::IncreaseInvestOrder {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				investor: default_address_32(),
				currency: TOKEN_ID,
				amount: AMOUNT,
			},
			"090000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640000000000000000000000000eb5ec7b000000000052b7d2dcc80cd2e4000000",
		)
	}

	#[test]
	fn decrease_invest_order() {
		test_encode_decode_identity(
			Message::DecreaseInvestOrder {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				investor: default_address_32(),
				currency: TOKEN_ID,
				amount: AMOUNT,
			},
			"0a0000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640000000000000000000000000eb5ec7b000000000052b7d2dcc80cd2e4000000",
		)
	}

	#[test]
	fn cancel_invest_order() {
		test_encode_decode_identity(
			Message::CancelInvestOrder {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				investor: default_address_32(),
				currency: TOKEN_ID,
			},
			"130000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640000000000000000000000000eb5ec7b",
		)
	}

	#[test]
	fn increase_redeem_order() {
		test_encode_decode_identity(
			Message::IncreaseRedeemOrder {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				investor: default_address_32(),
				currency: TOKEN_ID,
				amount: AMOUNT,
			},
			"0b0000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640000000000000000000000000eb5ec7b000000000052b7d2dcc80cd2e4000000",
		)
	}

	#[test]
	fn decrease_redeem_order() {
		test_encode_decode_identity(
			Message::DecreaseRedeemOrder {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				investor: default_address_32(),
				currency: TOKEN_ID,
				amount: AMOUNT,
			},
			"0c0000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640000000000000000000000000eb5ec7b000000000052b7d2dcc80cd2e4000000",
		)
	}

	#[test]
	fn cancel_redeem_order() {
		test_encode_decode_identity(
			Message::CancelRedeemOrder {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				investor: default_address_32(),
				currency: TOKEN_ID,
			},
			"140000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640000000000000000000000000eb5ec7b",
		)
	}

	#[test]
	fn collect_invest() {
		test_encode_decode_identity(
			Message::CollectInvest {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				investor: default_address_32(),
				currency: TOKEN_ID,
			},
			"0d0000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640000000000000000000000000eb5ec7b",
		)
	}

	#[test]
	fn collect_redeem() {
		test_encode_decode_identity(
			Message::CollectRedeem {
				pool_id: POOL_ID,
				tranche_id: default_tranche_id(),
				investor: default_address_32(),
				currency: TOKEN_ID
			},
			"0e0000000000bce1a4811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640000000000000000000000000eb5ec7b",
		)
	}

	#[test]
	fn executed_decrease_invest_order() {
		test_encode_decode_identity(
			Message::ExecutedDecreaseInvestOrder {
				pool_id: POOL_ID,
				tranche_id: default_tranche_id(),
				investor: vec_to_fixed_array(default_address_20()),
				currency: TOKEN_ID,
				currency_payout: AMOUNT / 2,
				remaining_invest_amount: AMOUNT / 4,
			},
			"0f0000000000bce1a4811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000000000000000000000000000000eb5ec7b0000000000295be96e64066972000000000000000014adf4b7320334b9000000",
		)
	}

	#[test]
	fn executed_decrease_redeem_order() {
		test_encode_decode_identity(
			Message::ExecutedDecreaseRedeemOrder {
				pool_id: POOL_ID,
				tranche_id: default_tranche_id(),
				investor: vec_to_fixed_array(default_address_20()),
				currency: TOKEN_ID,
				tranche_tokens_payout: AMOUNT / 2,
				remaining_redeem_amount: AMOUNT / 4,
			},
			"100000000000bce1a4811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000000000000000000000000000000eb5ec7b0000000000295be96e64066972000000000000000014adf4b7320334b9000000",
		)
	}

	#[test]
	fn executed_collect_invest() {
		test_encode_decode_identity(
			Message::ExecutedCollectInvest {
				pool_id: POOL_ID,
				tranche_id: default_tranche_id(),
				investor: vec_to_fixed_array(default_address_20()),
				currency: TOKEN_ID,
				currency_payout: AMOUNT,
				tranche_tokens_payout: AMOUNT / 2,
				remaining_invest_amount: AMOUNT / 4,
			},
			"110000000000bce1a4811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000000000000000000000000000000eb5ec7b000000000052b7d2dcc80cd2e40000000000000000295be96e64066972000000000000000014adf4b7320334b9000000",
		)
	}

	#[test]
	fn executed_collect_redeem() {
		test_encode_decode_identity(
			Message::ExecutedCollectRedeem {
				pool_id: POOL_ID,
				tranche_id: default_tranche_id(),
				investor: vec_to_fixed_array(default_address_20()),
				currency: TOKEN_ID,
				currency_payout: AMOUNT,
				tranche_tokens_payout: AMOUNT / 2,
				remaining_redeem_amount: AMOUNT / 4,
			},
			"120000000000bce1a4811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000000000000000000000000000000eb5ec7b000000000052b7d2dcc80cd2e40000000000000000295be96e64066972000000000000000014adf4b7320334b9000000",
		)
	}

	#[test]
	fn schedule_upgrade() {
		test_encode_decode_identity(
			Message::ScheduleUpgrade {
				contract: default_address_20(),
			},
			"151231231231231231231231231231231231231231",
		)
	}

	#[test]
	fn cancel_upgrade() {
		test_encode_decode_identity(
			Message::CancelUpgrade {
				contract: default_address_20(),
			},
			"161231231231231231231231231231231231231231",
		)
	}

	#[test]
	fn update_tranche_token_metadata() {
		test_encode_decode_identity(
			Message::UpdateTrancheTokenMetadata {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				token_name: vec_to_fixed_array(b"Some Name"),
				token_symbol: vec_to_fixed_array(b"SYMBOL"),
			},
			"170000000000000001811acd5b3f17c06841c7e41e9e04cb1b536f6d65204e616d65000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000053594d424f4c0000000000000000000000000000000000000000000000000000",
		)
	}

	#[test]
	fn disallow_investment_currency() {
		test_encode_decode_identity(
			Message::DisallowInvestmentCurrency {
				pool_id: POOL_ID,
				currency: TOKEN_ID,
			},
			"180000000000bce1a40000000000000000000000000eb5ec7b",
		)
	}

	#[test]
	fn disallow_investment_currency_zero() {
		test_encode_decode_identity(
			Message::DisallowInvestmentCurrency {
				pool_id: 0,
				currency: 0,
			},
			"18000000000000000000000000000000000000000000000000",
		)
	}

	/// Verify the identity property of decode . encode on a Message value and
	/// that it in fact encodes to and can be decoded from a given hex string.
	fn test_encode_decode_identity(msg: Message, expected_hex: &str) {
		let encoded = gmpf::to_vec(&msg).unwrap();
		assert_eq!(hex::encode(encoded.clone()), expected_hex);

		let decoded = gmpf::from_slice(
			&mut hex::decode(expected_hex)
				.expect("Decode should work")
				.as_slice(),
		)
		.expect("Deserialization should work");
		assert_eq!(msg, decoded);
	}

	fn default_address_20() -> [u8; 20] {
		<[u8; 20]>::from_hex("1231231231231231231231231231231231231231")
			.expect("Should be valid 20 bytes")
	}
	fn default_address_32() -> [u8; 32] {
		<[u8; 32]>::from_hex("4564564564564564564564564564564564564564564564564564564564564564")
			.expect("Should be valid 32 bytes")
	}
	fn default_tranche_id() -> TrancheId {
		<[u8; 16]>::from_hex("811acd5b3f17c06841c7e41e9e04cb1b")
			.expect("Should be valid tranche id")
	}
}
