//! A message requires a custom decoding & encoding, meeting the
//! LiquidityPool Generic Message Passing Format (GMPF): Every message is
//! encoded with a u8 at head flagging the message type, followed by its field.
//! Integers are big-endian encoded and enum values (such as `[crate::Domain]`)
//! also have a custom GMPF implementation, aiming for a fixed-size encoded
//! representation for each message variant.

use cfg_traits::{
	liquidity_pools::{LpMessage, Proof},
	Seconds,
};
use cfg_types::domain_address::Domain;
use frame_support::{pallet_prelude::RuntimeDebug, BoundedVec};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::{prelude::string::ToString, TypeInfo};
use serde::{
	de::{Deserializer, Error as _, SeqAccess, Visitor},
	ser::{Error as _, SerializeTuple},
	Deserialize, Serialize, Serializer,
};
use sp_io::hashing::keccak_256;
use sp_runtime::{traits::ConstU32, DispatchError, DispatchResult};
use sp_std::{vec, vec::Vec};

use crate::gmpf; // Generic Message Passing Format

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

// Max amount of messages a batch can have
const MAX_BATCH_MESSAGES: u32 = 16;

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

/// A message type that can not be a batch.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct NoBatchMessage(Message);

impl TryFrom<Message> for NoBatchMessage {
	type Error = DispatchError;

	fn try_from(message: Message) -> Result<Self, DispatchError> {
		match message {
			Message::Batch { .. } => Err(DispatchError::Other("A submessage can not be a batch")),
			_ => Ok(Self(message)),
		}
	}
}

impl MaxEncodedLen for NoBatchMessage {
	fn max_encoded_len() -> usize {
		// This message use a non batch message version to obtain the encoded
		// len to avoid an infite recursion: message -> batch -> message -> batch ...
		Message::<()>::max_encoded_len()
	}
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
pub struct BatchMessages(BoundedVec<NoBatchMessage, ConstU32<MAX_BATCH_MESSAGES>>);

impl Serialize for BatchMessages {
	fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		// Serializing as a tuple to avoid the prefix length used for dynamic lists
		let mut tuple = serializer.serialize_tuple(self.0.len())?;
		for msg in self.0.iter() {
			let encoded = gmpf::to_vec(&msg.0).map_err(|e| S::Error::custom(e.to_string()))?;

			// Serializing as bytes automatically encodes the prefix size
			tuple.serialize_element(&encoded)?;
		}
		tuple.end()
	}
}

impl<'de> Deserialize<'de> for BatchMessages {
	fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		// We need a custom visitor because we do not know the length upfront
		struct MsgVisitor;

		impl<'de> Visitor<'de> for MsgVisitor {
			type Value = BatchMessages;

			fn expecting(&self, formatter: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
				formatter.write_str("A sequence of pairs size-submessage")
			}

			fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
				let mut batch = BatchMessages::default();

				// We only stop on error trying to deserialize the length of the submessage.
				// The error will happen when we reach EOF
				while seq.next_element::<u16>().unwrap_or(None).is_some() {
					let msg = seq
						.next_element()?
						.ok_or(A::Error::custom("expected submessage"))?;

					batch
						.try_add(msg)
						.map_err(|e| A::Error::custom::<&'static str>(e.into()))?;
				}

				Ok(batch)
			}
		}

		let limit = MAX_BATCH_MESSAGES as usize * 2; // Lengths and messages
		deserializer.deserialize_tuple(limit, MsgVisitor)
	}
}

impl TryFrom<Vec<Message>> for BatchMessages {
	type Error = DispatchError;

	fn try_from(messages: Vec<Message>) -> Result<Self, DispatchError> {
		Ok(Self(
			messages
				.into_iter()
				.map(TryFrom::try_from)
				.collect::<Result<Vec<_>, _>>()?
				.try_into()
				.map_err(|_| DispatchError::Other("Batch limit reached"))?,
		))
	}
}

impl IntoIterator for BatchMessages {
	type IntoIter = sp_std::vec::IntoIter<Self::Item>;
	type Item = Message;

	fn into_iter(self) -> Self::IntoIter {
		let messages: Vec<_> = self.0.into_iter().map(|msg| msg.0).collect();
		messages.into_iter()
	}
}

impl BatchMessages {
	pub fn try_add(&mut self, message: Message) -> DispatchResult {
		self.0
			.try_push(message.try_into()?)
			.map_err(|_| DispatchError::Other("Batch limit reached"))?;

		Ok(())
	}

	pub fn len(&self) -> usize {
		self.0.len()
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
	Default,
)]
pub enum Message<BatchContent = BatchMessages> {
	#[default]
	Invalid,
	// --- Gateway ---
	/// Proof a message has been executed.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	MessageProof {
		// Hash of the message for which the proof is provided
		hash: [u8; 32],
	},
	/// Initiate the recovery of a message.
	///
	/// Must only be callable by root.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	InitiateMessageRecovery {
		/// The hash of the message which shall be recovered
		hash: [u8; 32],
		/// The address of the router
		address: Address,
	},
	/// Dispute the recovery of a message.
	///
	/// Must only be callable by root.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	DisputeMessageRecovery {
		/// The hash of the message which shall be disputed
		message: [u8; 32],
		/// The address of the router
		router: [u8; 32],
	},
	/// A batch of ordered messages.
	/// Don't allow nested batch messages.
	Batch(BatchContent),
	// --- Root ---
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
	/// Allows Governance to recover ERC-20 assets sent to the wrong contract by
	/// mistake.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	RecoverAssets {
		/// The EVM contract address to which the tokens were wrongfully sent
		contract: Address,
		/// The ERC-20 asset to recover
		asset: Address,
		/// The user address which receives the recovered tokens
		recipient: Address,
		/// The amount of tokens to recover.
		///
		/// NOTE: Represents `sp_core::U256` because EVM balances are `u256`.
		amount: [u8; 32],
	},
	// --- Gas service ---
	/// Updates the gas price which should cover transaction fees on Centrifuge
	/// Chain side.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	UpdateCentrifugeGasPrice {
		/// The new gas price
		price: u128,
	},
	// --- Pool Manager ---
	/// Add a currency to a domain, i.e, register the mapping of a currency id
	/// to the corresponding EVM Address.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	AddAsset {
		currency: u128,
		evm_address: [u8; 20],
	},
	/// Add a pool to a domain.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	AddPool { pool_id: u64 },
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
		hook: Address,
	},
	/// Allow a currency to be used as a pool currency and to invest in a pool.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	AllowAsset { pool_id: u64, currency: u128 },
	/// Disallow a currency to be used as a pool currency and to invest in a
	/// pool.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	DisallowAsset { pool_id: u64, currency: u128 },
	/// Update the price of a tranche token on the target domain.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	UpdateTranchePrice {
		pool_id: u64,
		tranche_id: TrancheId,
		currency: u128,
		price: u128,
		/// The timestamp at which the price was computed
		computed_at: Seconds,
	},
	/// Updates the name and symbol of a tranche token.
	///
	/// NOTE: We do not allow updating the decimals as this would require
	/// migrating all associated balances.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	UpdateTrancheMetadata {
		pool_id: u64,
		tranche_id: TrancheId,
		#[serde(with = "serde_big_array::BigArray")]
		token_name: [u8; TOKEN_NAME_SIZE],
		token_symbol: [u8; TOKEN_SYMBOL_SIZE],
	},
	UpdateTrancheHook {
		pool_id: u64,
		tranche_id: TrancheId,
		/// The address to be used for this tranche token on the domain it will
		/// be added and subsequently deployed in.
		hook: Address,
	},
	/// Transfer non-tranche tokens fungibles. For v2, it will only support
	/// stable-coins.
	///
	/// Directionality: Centrifuge <-> EVM Domain.
	///
	/// NOTE: Receiving domain must not accept every incoming token.
	/// For Centrifuge -> EVM Domain: `AddAsset` should have been called
	/// beforehand. For Centrifuge <- EVM Domain: We can assume `AddAsset`
	/// has been called for that domain already.
	TransferAssets {
		currency: u128,
		receiver: Address,
		amount: u128,
	},
	/// Transfer tranche tokens between domains.
	///
	/// Directionality: Centrifuge <-> EVM Domain.
	TransferTrancheTokens {
		pool_id: u64,
		tranche_id: TrancheId,
		domain: SerializableDomain,
		receiver: Address,
		amount: u128,
	},
	/// Update the restriction on a foreign domain.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	UpdateRestriction {
		pool_id: u64,
		tranche_id: TrancheId,
		update: UpdateRestrictionMessage,
	},
	/// Increase the invest order amount for the specified pair of pool and
	/// tranche token.
	///
	/// Directionality: Centrifuge <- EVM Domain.
	DepositRequest {
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
	RedeemRequest {
		pool_id: u64,
		tranche_id: TrancheId,
		investor: Address,
		currency: u128,
		amount: u128,
	},
	/// The message sent back to the domain from which a `DepositRequest`
	/// originated from after the deposit was fully processed during epoch
	/// execution. Ensures the `investor` gets the payout respective to
	/// their investment.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	FulfilledDepositRequest {
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
	},
	/// The message sent back to the domain from which a `RedeemRequest`
	/// originated from after the redemption was fully processed during epoch
	/// execution. Ensures the `investor` gets the payout respective to
	/// their redemption.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	FulfilledRedeemRequest {
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
	},
	/// Cancel an unprocessed invest order for the specified pair of pool and
	/// tranche token.
	///
	/// On success, triggers a message sent back to the sending domain.
	/// The message will take care of re-funding the investor with the given
	/// amount the order was reduced with. The `investor` address is used as
	/// the receiver of that tokens.
	///
	/// Directionality: Centrifuge <- EVM Domain.
	CancelDepositRequest {
		pool_id: u64,
		tranche_id: TrancheId,
		investor: Address,
		currency: u128,
	},
	/// Cancel an unprocessed redemption for the specified pair of pool and
	/// tranche token.
	///
	/// On success, triggers a message sent back to the sending domain.
	/// The message will take care of re-funding the investor with the given
	/// amount the order was reduced with. The `investor` address is used as
	/// the receiver of that tokens.
	///
	/// Directionality: Centrifuge <- EVM Domain.
	CancelRedeemRequest {
		pool_id: u64,
		tranche_id: TrancheId,
		investor: Address,
		currency: u128,
	},
	/// The message sent back to the domain from which a `CancelDepositRequest`
	/// message was received, ensuring the correct state update on said domain
	/// and that the `investor`'s wallet is updated accordingly.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	FulfilledCancelDepositRequest {
		/// The pool id
		pool_id: u64,
		/// The tranche id
		tranche_id: TrancheId,
		/// The investor's address
		investor: Address,
		/// The currency in which `CancelDepositRequest` was realised
		currency: u128,
		/// The amount of `currency` by which the
		/// investment order was actually decreased by.
		currency_payout: u128,
		/// The fulfilled investment amount of `currency`. It reflects the
		/// amount of investments which were processed independent of whether
		/// they were collected.
		fulfilled_invest_amount: u128,
	},
	/// The message sent back to the domain from which a `CancelRedeemRequest`
	/// message was received, ensuring the correct state update on said domain
	/// and that the `investor`'s wallet is updated accordingly.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	FulfilledCancelRedeemRequest {
		/// The pool id
		pool_id: u64,
		/// The tranche id
		tranche_id: TrancheId,
		/// The investor's address
		investor: Address,
		/// The currency in which `CancelRedeemRequest` was realised in.
		currency: u128,
		/// The amount of tranche tokens by which the redeem order was actually
		/// decreased by.
		tranche_tokens_payout: u128,
	},
	TriggerRedeemRequest {
		/// The pool id
		pool_id: u64,
		/// The tranche id
		tranche_id: TrancheId,
		/// The investor's address
		investor: Address,
		/// The currency in which the redeem request should be realised in.
		currency: u128,
		/// The amount of tranche tokens which should be redeemed.
		amount: u128,
	},
}

impl LpMessage for Message {
	fn serialize(&self) -> Vec<u8> {
		gmpf::to_vec(self).unwrap_or_default()
	}

	fn deserialize(data: &[u8]) -> Result<Self, DispatchError> {
		gmpf::from_slice(data).map_err(|_| DispatchError::Other("LP Deserialization issue"))
	}

	fn pack_with(&mut self, other: Self) -> Result<(), DispatchError> {
		match self {
			Message::Batch(content) => content.try_add(other),
			this => {
				*this = Message::Batch(BatchMessages::try_from(vec![this.clone(), other])?);
				Ok(())
			}
		}
	}

	fn submessages(&self) -> Vec<Self> {
		match self {
			Message::Batch(content) => content.clone().into_iter().collect(),
			message => vec![message.clone()],
		}
	}

	fn empty() -> Message {
		Message::Batch(BatchMessages::default())
	}

	fn get_proof(&self) -> Option<Proof> {
		match self {
			Message::MessageProof { hash } => Some(*hash),
			_ => None,
		}
	}

	fn to_proof_message(&self) -> Self {
		let hash = keccak_256(&LpMessage::serialize(self));

		Message::MessageProof { hash }
	}
}

/// A Liquidity Pool message for updating restrictions on foreign domains.
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
pub enum UpdateRestrictionMessage {
	Invalid,
	/// Whitelist an address for the specified pair of pool and tranche token on
	/// the target domain.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	UpdateMember {
		member: Address,
		valid_until: Seconds,
	},
	/// Disallow an investor to further invest into the given liquidity pool
	///
	/// Directionality: Centrifuge -> EVM Domain.
	Freeze {
		// The address of the user which is being frozen
		address: Address,
	},
	/// Revert a previous `Freeze.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	Unfreeze {
		// The address of the user which is allowed to invest again
		address: Address,
	},
}

#[cfg(test)]
mod tests {
	use cfg_primitives::{PoolId, TrancheId};
	use cfg_types::fixed_point::Ratio;
	use cfg_utils::vec_to_fixed_array;
	use frame_support::assert_err;
	use hex::FromHex;
	use sp_runtime::{traits::One, FixedPointNumber};

	use super::*;
	use crate::{Domain, DomainAddress};

	const AMOUNT: u128 = 100000000000000000000000000;
	const POOL_ID: PoolId = 12378532;
	const TOKEN_ID: u128 = 246803579;

	#[test]
	fn invalid() {
		let msg: Message<BatchMessages> = Message::Invalid;
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
	fn add_currency() {
		test_encode_decode_identity(
			Message::AddAsset {
				currency: TOKEN_ID,
				evm_address: default_address_20(),
			},
			"090000000000000000000000000eb5ec7b1231231231231231231231231231231231231231",
		)
	}

	#[test]
	fn add_pool_long() {
		test_encode_decode_identity(Message::AddPool { pool_id: POOL_ID }, "0a0000000000bce1a4")
	}

	#[test]
	fn allow_asset() {
		test_encode_decode_identity(
			Message::AllowAsset {
				currency: TOKEN_ID,
				pool_id: POOL_ID,
			},
			"0c0000000000bce1a40000000000000000000000000eb5ec7b",
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
				hook: default_address_32(),
			},
			"0b0000000000000001811acd5b3f17c06841c7e41e9e04cb1b536f6d65204e616d65000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000053594d424f4c00000000000000000000000000000000000000000000000000000f4564564564564564564564564564564564564564564564564564564564564564",
		)
	}

	#[test]
	fn update_tranche_token_price() {
		test_encode_decode_identity(
			Message::UpdateTranchePrice {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				currency: TOKEN_ID,
				price: Ratio::one().into_inner(),
				computed_at: 1698131924,
			},
			"0e0000000000000001811acd5b3f17c06841c7e41e9e04cb1b0000000000000000000000000eb5ec7b00000000000000000de0b6b3a76400000000000065376fd4",
		)
	}

	#[test]
	fn update_member() {
		test_encode_decode_identity(
			Message::UpdateRestriction{
				pool_id: 2,
				tranche_id: default_tranche_id(),
				update: UpdateRestrictionMessage::UpdateMember {
					member: default_address_32(),
					valid_until: 1706260138,
				}
			},
			"130000000000000002811acd5b3f17c06841c7e41e9e04cb1b0145645645645645645645645645645645645645645645645645645645645645640000000065b376aa",
		)
	}

	#[test]
	fn transfer_to_evm_address() {
		test_encode_decode_identity(
			Message::TransferAssets {
					currency: TOKEN_ID,
					receiver: vec_to_fixed_array(default_address_20()),
					amount: AMOUNT,
				},
			"110000000000000000000000000eb5ec7b1231231231231231231231231231231231231231000000000000000000000000000000000052b7d2dcc80cd2e4000000"
			);
	}

	#[test]
	fn transfer_to_centrifuge() {
		test_encode_decode_identity(
			Message::TransferAssets {
        			currency: TOKEN_ID,
					receiver: default_address_32(),
					amount: AMOUNT,
				},
			"110000000000000000000000000eb5ec7b4564564564564564564564564564564564564564564564564564564564564564000000000052b7d2dcc80cd2e4000000"
			);
	}

	#[test]
	fn transfer_tranche_tokens_to_moonbeam() {
		let domain_address = DomainAddress::EVM(1284, default_address_20());

		test_encode_decode_identity(
			Message::TransferTrancheTokens {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				domain: domain_address.domain().into(),
				receiver: domain_address.address(),
				amount: AMOUNT,
			},
			"120000000000000001811acd5b3f17c06841c7e41e9e04cb1b0100000000000005041231231231231231231231231231231231231231000000000000000000000000000000000052b7d2dcc80cd2e4000000"
		);
	}

	#[test]
	fn transfer_tranche_tokens_to_centrifuge() {
		test_encode_decode_identity(
			Message::TransferTrancheTokens {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				domain: Domain::Centrifuge.into(),
				receiver: default_address_32(),
				amount: AMOUNT,
			},
			"120000000000000001811acd5b3f17c06841c7e41e9e04cb1b0000000000000000004564564564564564564564564564564564564564564564564564564564564564000000000052b7d2dcc80cd2e4000000"
		)
	}

	#[test]
	fn deposit_request() {
		test_encode_decode_identity(
			Message::DepositRequest {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				investor: default_address_32(),
				currency: TOKEN_ID,
				amount: AMOUNT,
			},
			"140000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640000000000000000000000000eb5ec7b000000000052b7d2dcc80cd2e4000000",
		)
	}

	#[test]
	fn cancel_deposit_request() {
		test_encode_decode_identity(
			Message::CancelDepositRequest {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				investor: default_address_32(),
				currency: TOKEN_ID,
			},
			"180000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640000000000000000000000000eb5ec7b",
		)
	}

	#[test]
	fn redeem_request() {
		test_encode_decode_identity(
			Message::RedeemRequest {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				investor: default_address_32(),
				currency: TOKEN_ID,
				amount: AMOUNT,
			},
			"150000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640000000000000000000000000eb5ec7b000000000052b7d2dcc80cd2e4000000",
		)
	}

	#[test]
	fn cancel_redeem_request() {
		test_encode_decode_identity(
			Message::CancelRedeemRequest {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				investor: default_address_32(),
				currency: TOKEN_ID,
			},
			"190000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640000000000000000000000000eb5ec7b",
		)
	}

	#[test]
	fn fulfilled_cancel_deposit_request() {
		test_encode_decode_identity(
			Message::FulfilledCancelDepositRequest {
				pool_id: POOL_ID,
				tranche_id: default_tranche_id(),
				investor: vec_to_fixed_array(default_address_20()),
				currency: TOKEN_ID,
				currency_payout: AMOUNT / 2,
				fulfilled_invest_amount: AMOUNT / 4,
			},
			"1a0000000000bce1a4811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000000000000000000000000000000eb5ec7b0000000000295be96e64066972000000000000000014adf4b7320334b9000000",
		)
	}

	#[test]
	fn fulfilled_cancel_redeem_request() {
		test_encode_decode_identity(
			Message::FulfilledCancelRedeemRequest {
				pool_id: POOL_ID,
				tranche_id: default_tranche_id(),
				investor: vec_to_fixed_array(default_address_20()),
				currency: TOKEN_ID,
				tranche_tokens_payout: AMOUNT / 2,
			},
			"1b0000000000bce1a4811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000000000000000000000000000000eb5ec7b0000000000295be96e64066972000000",
		)
	}

	#[test]
	fn fulfilled_deposit_request() {
		test_encode_decode_identity(
			Message::FulfilledDepositRequest {
				pool_id: POOL_ID,
				tranche_id: default_tranche_id(),
				investor: vec_to_fixed_array(default_address_20()),
				currency: TOKEN_ID,
				currency_payout: AMOUNT,
				tranche_tokens_payout: AMOUNT / 2,
			},
			"160000000000bce1a4811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000000000000000000000000000000eb5ec7b000000000052b7d2dcc80cd2e40000000000000000295be96e64066972000000",
		)
	}

	#[test]
	fn fulfilled_redeem_request() {
		test_encode_decode_identity(
			Message::FulfilledRedeemRequest {
				pool_id: POOL_ID,
				tranche_id: default_tranche_id(),
				investor: vec_to_fixed_array(default_address_20()),
				currency: TOKEN_ID,
				currency_payout: AMOUNT,
				tranche_tokens_payout: AMOUNT / 2,
			},
			"170000000000bce1a4811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000000000000000000000000000000eb5ec7b000000000052b7d2dcc80cd2e40000000000000000295be96e64066972000000",
		)
	}

	#[test]
	fn schedule_upgrade() {
		test_encode_decode_identity(
			Message::ScheduleUpgrade {
				contract: default_address_20(),
			},
			"051231231231231231231231231231231231231231",
		)
	}

	#[test]
	fn cancel_upgrade() {
		test_encode_decode_identity(
			Message::CancelUpgrade {
				contract: default_address_20(),
			},
			"061231231231231231231231231231231231231231",
		)
	}

	#[test]
	fn recover_assets() {
		let msg = Message::RecoverAssets {
			contract: [2u8; 32],
			asset: [1u8; 32],
			recipient: [3u8; 32],
			amount: (sp_core::U256::MAX - 1).into(),
		};
		test_encode_decode_identity(
			msg,
			concat!(
				"07",
				"0202020202020202020202020202020202020202020202020202020202020202",
				"0101010101010101010101010101010101010101010101010101010101010101",
				"0303030303030303030303030303030303030303030303030303030303030303",
				"fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe",
			),
		);
	}

	#[test]
	fn update_tranche_token_metadata() {
		test_encode_decode_identity(
			Message::UpdateTrancheMetadata {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				token_name: vec_to_fixed_array(b"Some Name"),
				token_symbol: vec_to_fixed_array(b"SYMBOL"),
			},
			"0f0000000000000001811acd5b3f17c06841c7e41e9e04cb1b536f6d65204e616d65000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000053594d424f4c0000000000000000000000000000000000000000000000000000",
		)
	}

	#[test]
	fn disallow_asset() {
		test_encode_decode_identity(
			Message::DisallowAsset {
				pool_id: POOL_ID,
				currency: TOKEN_ID,
			},
			"0d0000000000bce1a40000000000000000000000000eb5ec7b",
		)
	}

	#[test]
	fn disallow_asset_zero() {
		test_encode_decode_identity(
			Message::DisallowAsset {
				pool_id: 0,
				currency: 0,
			},
			"0d000000000000000000000000000000000000000000000000",
		)
	}

	#[test]
	fn batch_empty() {
		test_encode_decode_identity(Message::Batch(BatchMessages::default()), concat!("04"))
	}

	#[test]
	fn batch_messages() {
		test_encode_decode_identity(
			Message::Batch(
				BatchMessages::try_from(vec![
					Message::AddPool { pool_id: 0 },
					Message::AllowAsset {
						currency: TOKEN_ID,
						pool_id: POOL_ID,
					},
				])
				.unwrap(),
			),
			concat!(
				"04",                                                 // Batch index
				"0009",                                               // AddPool length
				"0a0000000000000000",                                 // AddPool content
				"0019",                                               // AddAsset length
				"0c0000000000bce1a40000000000000000000000000eb5ec7b", // AllowAsset content
			),
		)
	}

	#[test]
	fn batch_of_batches_deserialization() {
		// The message can not be created directly
		let encoded = concat!(
			"04",                 // Batch index
			"000c",               // Submessage length
			"04",                 // Inner batch index
			"0009",               // AddPool length
			"0a0000000000000000", // AddPool content
		);

		assert_err!(
			gmpf::from_slice::<Message>(&hex::decode(encoded).unwrap()),
			gmpf::Error::Message("A submessage can not be a batch".into()),
		);
	}

	/// Verify the identity property of decode . encode on a Message value and
	/// that it in fact encodes to and can be decoded from a given hex string.
	fn test_encode_decode_identity(msg: Message, expected_hex: &str) {
		let encoded = gmpf::to_vec(&msg).unwrap();
		assert_eq!(hex::encode(encoded.clone()), expected_hex);

		let decoded: Message =
			gmpf::from_slice(&hex::decode(expected_hex).expect("Decode should work"))
				.expect("Deserialization should work");
		assert_eq!(decoded, msg);
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
