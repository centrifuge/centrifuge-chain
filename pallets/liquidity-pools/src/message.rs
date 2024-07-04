use cfg_traits::{liquidity_pools::Codec, Seconds};
use cfg_utils::{decode, decode_be_bytes, encode_be};
use frame_support::pallet_prelude::RuntimeDebug;
use parity_scale_codec::{Decode, Encode, Input, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_std::{vec, vec::Vec};

/// Address type
/// Note: It can be used to represent any address type with a length <= 32
/// bytes; For example, it can represent an Ethereum address (20-bytes long) by
/// padding it with 12 zeros.
type Address = [u8; 32];

/// The fixed size for the array representing a tranche token name
pub const TOKEN_NAME_SIZE: usize = 128;

// The fixed size for the array representing a tranche token symbol
pub const TOKEN_SYMBOL_SIZE: usize = 32;

/// A LiquidityPools Message
///
/// A message requires a custom decoding & encoding, meeting the
/// LiquidityPool Generic Message Passing Format (GMPF): Every message is
/// encoded with a u8 at head flagging the message type, followed by its field.
/// Integers are big-endian encoded and enum values (such as `[crate::Domain]`)
/// also have a custom GMPF implementation, aiming for a fixed-size encoded
/// representation for each message variant.
///
/// NOTE: The sender of a message cannot ensure whether the
/// corresponding receiver rejects it.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum Message<Domain, PoolId, TrancheId, Balance, Ratio>
where
	Domain: Codec,
	PoolId: Encode + Decode,
	TrancheId: Encode + Decode,
	Balance: Encode + Decode,
	Ratio: Encode + Decode,
{
	Invalid,
	// --- Gateway ---
	/// Proof a message has been executed.
	///
	/// Directionality: Centrifuge -> EVM Domain. // TODO(@william): Check
	MessageProof {
		hash: [u8; 32],
	},
	/// Initiate the recovery of a message.
	///
	/// Must only be callable by root.
	///
	/// Directionality: Centrifuge -> EVM Domain. // TODO(@william): Check
	InitiateMessageRecovery {
		/// The hash of the message which shall be recovered
		hash: [u8; 32],
	},
	/// Dispute the recovery of a message.
	///
	/// Must only be callable by root.
	///
	/// Directionality: Centrifuge -> EVM Domain. // TODO(@william): Check
	DisputeMessageRecovery {
		/// The hash of the message which shall be disputed
		hash: [u8; 32],
	},
	// TODO(@william): Fields + docs
	Batch,
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
	/// Allows Governance to recover tokens sent to the wrong contract by
	/// mistake.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	RecoverTokens {
		/// The EVM contract address to which the tokens were wrongfully sent
		contract: Address,
		/// The tranche token to recover
		tranche_token: Address,
		/// The user address which receives the recovered tokens
		recipient: Address,
		/// The amount of tokens to recover
		amount: u128,
	},
	// --- Gas service ---
	/// Updates the gas price which should cover transaction fees on Centrifuge
	/// Chain side.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	UpdateCentrifugeGasPrice {
		/// The new gas price
		price: u64,
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
	AddPool {
		pool_id: PoolId,
	},
	/// Add a tranche to an already existing pool on the target domain.
	/// The decimals of a tranche MUST be equal to the decimals of a pool.
	/// Thus, consuming domains MUST take care of storing the decimals upon
	/// receiving an AddPool message.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	AddTranche {
		pool_id: PoolId,
		tranche_id: TrancheId,
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
	AllowAsset {
		pool_id: PoolId,
		currency: u128,
	},
	/// Disallow a currency to be used as a pool currency and to invest in a
	/// pool.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	DisallowAsset {
		pool_id: PoolId,
		currency: u128,
	},
	/// Update the price of a tranche token on the target domain.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	UpdateTrancheTokenPrice {
		pool_id: PoolId,
		tranche_id: TrancheId,
		currency: u128,
		price: Ratio,
		/// The timestamp at which the price was computed
		computed_at: Seconds,
	},
	/// Updates the name and symbol of a tranche token.
	///
	/// NOTE: We do not allow updating the decimals as this would require
	/// migrating all associated balances.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	UpdateTrancheTokenMetadata {
		pool_id: PoolId,
		tranche_id: TrancheId,
		token_name: [u8; TOKEN_NAME_SIZE],
		token_symbol: [u8; TOKEN_SYMBOL_SIZE],
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
	Transfer {
		currency: u128,
		sender: Address,
		receiver: Address,
		amount: Balance,
	},
	/// Transfer tranche tokens between domains.
	///
	/// Directionality: Centrifuge <-> EVM Domain.
	TransferTrancheTokens {
		pool_id: PoolId,
		tranche_id: TrancheId,
		sender: Address,
		domain: Domain,
		receiver: Address,
		amount: Balance,
	},
	/// Update the restriction on a foreign domain.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	UpdateRestriction {
		pool_id: PoolId,
		tranche_id: TrancheId,
		update: UpdateRestrictionMessage,
	},
	/// Increase the invest order amount for the specified pair of pool and
	/// tranche token.
	///
	/// Directionality: Centrifuge <- EVM Domain.
	DepositRequest {
		pool_id: PoolId,
		tranche_id: TrancheId,
		investor: Address,
		currency: u128,
		amount: Balance,
	},
	/// Increase the redeem order amount for the specified pair of pool and
	/// tranche token.
	///
	/// Directionality: Centrifuge <- EVM Domain.
	RedeemRequest {
		pool_id: PoolId,
		tranche_id: TrancheId,
		investor: Address,
		currency: u128,
		amount: Balance,
	},
	/// The message sent back to the domain from which a `DepositRequest`
	/// originated from after the deposit was fully processed during epoch
	/// execution. Ensures the `investor` gets the payout respective to
	/// their investment.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	FulfilledDepositRequest {
		/// The pool id
		pool_id: PoolId,
		/// The tranche
		tranche_id: TrancheId,
		/// The investor's address
		investor: Address,
		/// The currency in which the investment was realised
		currency: u128,
		/// The amount that was actually collected, in `currency` units
		currency_payout: Balance,
		/// The amount of tranche tokens received for the investment made
		tranche_tokens_payout: Balance,
		/// The fulfilled investment amount denominated in the `foreign` payment
		/// currency. It reflects the amount of investments which were processed
		/// independent of whether they were collected.
		// TODO(@Luis): Apply delta instead of remaining to foreign investments
		fulfilled_invest_amount: Balance,
	},
	/// The message sent back to the domain from which a `RedeemRequest`
	/// originated from after the redemption was fully processed during epoch
	/// execution. Ensures the `investor` gets the payout respective to
	/// their redemption.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	FulfilledRedeemRequest {
		/// The pool id
		pool_id: PoolId,
		/// The tranche id
		tranche_id: TrancheId,
		/// The investor's address
		investor: Address,
		/// The stable coin currency in which the payout takes place
		currency: u128,
		/// The amount of `currency` being paid out to the investor
		currency_payout: Balance,
		/// How many tranche tokens were actually redeemed
		tranche_tokens_payout: Balance,
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
		pool_id: PoolId,
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
		pool_id: PoolId,
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
		pool_id: PoolId,
		/// The tranche id
		tranche_id: TrancheId,
		/// The investor's address
		investor: Address,
		/// The currency in which `CancelDepositRequest` was realised
		currency: u128,
		/// The amount of `currency` by which the
		/// investment order was actually decreased by.
		currency_payout: Balance,
		/// The fulfilled investment amount of `currency`. It reflects the
		/// amount of investments which were processed independent of whether
		/// they were collected.
		// TODO(@Luis): Apply delta instead of remaining to foreign investments
		fulfilled_invest_amount: Balance,
	},
	/// The message sent back to the domain from which a `CancelRedeemRequest`
	/// message was received, ensuring the correct state update on said domain
	/// and that the `investor`'s wallet is updated accordingly.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	FulfilledCancelRedeemRequest {
		/// The pool id
		pool_id: PoolId,
		/// The tranche id
		tranche_id: TrancheId,
		/// The investor's address
		investor: Address,
		/// The currency in which `CancelRedeemRequest` was realised in.
		currency: u128,
		/// The amount of tranche tokens by which the redeem order was actually
		/// decreased by.
		tranche_tokens_payout: Balance,
		/// The fulfilled redemption amount. It reflects the amount of tranche
		/// tokens which were redeemed and processed during epoch execution
		/// independent of whether they were collected.
		// TODO(@Luis): Apply delta instead of remaining to foreign investments
		fulfilled_redeem_amount: Balance,
	},
	// TODO(@william): Add fields + docs
	TriggerRedeemRequest,
}

/// A Liquidity Pool message for updating restrictions on foreign domains.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
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

impl UpdateRestrictionMessage {
	fn call_type(&self) -> u8 {
		match self {
			Self::Invalid { .. } => 0,
			Self::UpdateMember { .. } => 1,
			Self::Freeze { .. } => 2,
			Self::Unfreeze { .. } => 3,
		}
	}
}

impl Codec for UpdateRestrictionMessage {
	fn serialize(&self) -> Vec<u8> {
		match &self {
			UpdateRestrictionMessage::UpdateMember {
				member,
				valid_until,
			} => encoded_message(
				self.call_type(),
				vec![member.to_vec(), valid_until.to_be_bytes().to_vec()],
			),
			_ => todo!("@william"),
		}
	}

	fn deserialize<I: Input>(input: &mut I) -> Result<Self, parity_scale_codec::Error> {
		let call_type = input.read_byte()?;

		match call_type {
			1 => Ok(Self::UpdateMember {
				member: decode::<32, _, _>(input)?,
				valid_until: decode_be_bytes::<8, _, _>(input)?,
			}),
			_ => todo!("@william"),
		}
	}
}

impl<
		Domain: Codec,
		PoolId: Encode + Decode,
		TrancheId: Encode + Decode,
		Balance: Encode + Decode,
		Ratio: Encode + Decode,
	> Message<Domain, PoolId, TrancheId, Balance, Ratio>
{
	/// The call type that identifies a specific Message variant. This value is
	/// used to encode/decode a Message to/from a bytearray, whereas the head of
	/// the bytearray is the call type, followed by each message's param values.
	///
	/// NOTE: Each message must immutably  map to the same u8. Messages are
	/// decoded in other domains and MUST follow the defined standard.
	fn call_type(&self) -> u8 {
		match self {
			Self::Invalid { .. } => 0,
			Self::MessageProof { .. } => 1,
			Self::InitiateMessageRecovery { .. } => 2,
			Self::DisputeMessageRecovery { .. } => 3,
			Self::Batch { .. } => 4,
			Self::ScheduleUpgrade { .. } => 5,
			Self::CancelUpgrade { .. } => 6,
			Self::RecoverTokens { .. } => 7,
			Self::UpdateCentrifugeGasPrice { .. } => 8,
			Self::AddAsset { .. } => 9,
			Self::AddPool { .. } => 10,
			Self::AddTranche { .. } => 11,
			Self::AllowAsset { .. } => 12,
			Self::DisallowAsset { .. } => 13,
			Self::UpdateTrancheTokenPrice { .. } => 14,
			Self::UpdateTrancheTokenMetadata { .. } => 15,
			Self::Transfer { .. } => 16,
			Self::TransferTrancheTokens { .. } => 17,
			Self::UpdateRestriction { .. } => 18,
			Self::DepositRequest { .. } => 21,
			Self::RedeemRequest { .. } => 22,
			Self::FulfilledDepositRequest { .. } => 23,
			Self::FulfilledRedeemRequest { .. } => 24,
			Self::CancelDepositRequest { .. } => 25,
			Self::CancelRedeemRequest { .. } => 26,
			Self::FulfilledCancelDepositRequest { .. } => 27,
			Self::FulfilledCancelRedeemRequest { .. } => 28,
			Self::TriggerRedeemRequest { .. } => 29,
		}
	}
}

impl<
		Domain: Codec,
		PoolId: Encode + Decode,
		TrancheId: Encode + Decode,
		Balance: Encode + Decode,
		Ratio: Encode + Decode,
	> Codec for Message<Domain, PoolId, TrancheId, Balance, Ratio>
{
	fn serialize(&self) -> Vec<u8> {
		match self {
			Message::Invalid => vec![self.call_type()],
			Message::MessageProof { .. } => unimplemented!("todo @william"),
			Message::InitiateMessageRecovery { .. } => unimplemented!("todo @william"),
			Message::DisputeMessageRecovery { .. } => unimplemented!("todo @william"),
			Message::Batch { .. } => unimplemented!("todo @william"),
			Message::ScheduleUpgrade { contract } => {
				encoded_message(self.call_type(), vec![contract.to_vec()])
			}
			Message::CancelUpgrade { contract } => {
				encoded_message(self.call_type(), vec![contract.to_vec()])
			}
			Message::RecoverTokens { .. } => unimplemented!("todo @william"),
			Message::UpdateCentrifugeGasPrice { .. } => unimplemented!("todo @william"),
			Message::AddAsset {
				currency,
				evm_address,
			} => encoded_message(
				self.call_type(),
				vec![encode_be(currency), evm_address.to_vec()],
			),
			Message::AddPool { pool_id } => {
				encoded_message(self.call_type(), vec![encode_be(pool_id)])
			}
			Message::AddTranche {
				pool_id,
				tranche_id,
				token_name,
				token_symbol,
				decimals,
				hook,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					token_name.encode(),
					token_symbol.encode(),
					decimals.encode(),
					hook.encode(),
				],
			),
			Message::AllowAsset { pool_id, currency } => encoded_message(
				self.call_type(),
				vec![encode_be(pool_id), encode_be(currency)],
			),
			Message::DisallowAsset { pool_id, currency } => encoded_message(
				self.call_type(),
				vec![encode_be(pool_id), encode_be(currency)],
			),
			Message::UpdateTrancheTokenPrice {
				pool_id,
				tranche_id,
				currency,
				price,
				computed_at,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					encode_be(currency),
					encode_be(price),
					computed_at.to_be_bytes().to_vec(),
				],
			),
			Message::UpdateTrancheTokenMetadata {
				pool_id,
				tranche_id,
				token_name,
				token_symbol,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					token_name.encode(),
					token_symbol.encode(),
				],
			),
			Message::Transfer {
				currency,
				sender,
				receiver,
				amount,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(currency),
					sender.to_vec(),
					receiver.to_vec(),
					encode_be(amount),
				],
			),
			Message::TransferTrancheTokens {
				pool_id,
				tranche_id,
				sender,
				domain,
				receiver,
				amount,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					sender.to_vec(),
					domain.serialize(),
					receiver.to_vec(),
					encode_be(amount),
				],
			),
			Message::UpdateRestriction {
				pool_id,
				tranche_id,
				update,
			} => encoded_message(
				self.call_type(),
				vec![encode_be(pool_id), tranche_id.encode(), update.serialize()],
			),
			Message::DepositRequest {
				pool_id,
				tranche_id,
				investor: address,
				currency,
				amount,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					address.to_vec(),
					encode_be(currency),
					encode_be(amount),
				],
			),
			Message::RedeemRequest {
				pool_id,
				tranche_id,
				investor,
				currency,
				amount,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					investor.to_vec(),
					encode_be(currency),
					encode_be(amount),
				],
			),
			Message::FulfilledDepositRequest {
				pool_id,
				tranche_id,
				investor,
				currency,
				currency_payout,
				tranche_tokens_payout,
				fulfilled_invest_amount: remaining_invest_amount,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					investor.to_vec(),
					encode_be(currency),
					encode_be(currency_payout),
					encode_be(tranche_tokens_payout),
					encode_be(remaining_invest_amount),
				],
			),
			Message::FulfilledRedeemRequest {
				pool_id,
				tranche_id,
				investor,
				currency,
				currency_payout,
				tranche_tokens_payout,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					investor.to_vec(),
					encode_be(currency),
					encode_be(currency_payout),
					encode_be(tranche_tokens_payout),
				],
			),
			Message::CancelDepositRequest {
				pool_id,
				tranche_id,
				investor,
				currency,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					investor.to_vec(),
					encode_be(currency),
				],
			),
			Message::CancelRedeemRequest {
				pool_id,
				tranche_id,
				investor,
				currency,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					investor.to_vec(),
					encode_be(currency),
				],
			),
			Message::FulfilledCancelDepositRequest {
				pool_id,
				tranche_id,
				investor,
				currency,
				currency_payout,
				fulfilled_invest_amount: remaining_invest_amount,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					investor.to_vec(),
					encode_be(currency),
					encode_be(currency_payout),
					encode_be(remaining_invest_amount),
				],
			),
			Message::FulfilledCancelRedeemRequest {
				pool_id,
				tranche_id,
				investor,
				currency,
				tranche_tokens_payout,
				// TODO(@Luis): Apply delta instead of remaining to foreign investments
				fulfilled_redeem_amount: remaining_redeem_amount,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					investor.to_vec(),
					encode_be(currency),
					encode_be(tranche_tokens_payout),
					encode_be(remaining_redeem_amount),
				],
			),
			Message::TriggerRedeemRequest { .. } => unimplemented!("todo @william"),
		}
	}

	fn deserialize<I: Input>(input: &mut I) -> Result<Self, parity_scale_codec::Error> {
		let call_type = input.read_byte()?;

		match call_type {
			0 => Ok(Self::Invalid),
			1 => unimplemented!(""),
			2 => unimplemented!(""),
			3 => unimplemented!(""),
			4 => unimplemented!(""),
			5 => Ok(Self::ScheduleUpgrade {
				contract: decode::<20, _, _>(input)?,
			}),
			6 => Ok(Self::CancelUpgrade {
				contract: decode::<20, _, _>(input)?,
			}),
			7 => unimplemented!(""),
			8 => unimplemented!(""),
			9 => Ok(Self::AddAsset {
				currency: decode_be_bytes::<16, _, _>(input)?,
				evm_address: decode::<20, _, _>(input)?,
			}),
			10 => Ok(Self::AddPool {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
			}),
			11 => Ok(Self::AddTranche {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				token_name: decode::<TOKEN_NAME_SIZE, _, _>(input)?,
				token_symbol: decode::<TOKEN_SYMBOL_SIZE, _, _>(input)?,
				decimals: decode::<1, _, _>(input)?,
				hook: decode::<32, _, _>(input)?,
			}),
			12 => Ok(Self::AllowAsset {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
			}),
			13 => Ok(Self::DisallowAsset {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
			}),
			14 => Ok(Self::UpdateTrancheTokenPrice {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
				price: decode_be_bytes::<16, _, _>(input)?,
				computed_at: decode_be_bytes::<8, _, _>(input)?,
			}),
			15 => Ok(Self::UpdateTrancheTokenMetadata {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				token_name: decode::<TOKEN_NAME_SIZE, _, _>(input)?,
				token_symbol: decode::<TOKEN_SYMBOL_SIZE, _, _>(input)?,
			}),
			16 => Ok(Self::Transfer {
				currency: decode_be_bytes::<16, _, _>(input)?,
				sender: decode::<32, _, _>(input)?,
				receiver: decode::<32, _, _>(input)?,
				amount: decode_be_bytes::<16, _, _>(input)?,
			}),
			17 => Ok(Self::TransferTrancheTokens {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				sender: decode::<32, _, _>(input)?,
				domain: deserialize::<9, _, _>(input)?,
				receiver: decode::<32, _, _>(input)?,
				amount: decode_be_bytes::<16, _, _>(input)?,
			}),
			18 => Ok(Self::UpdateRestriction {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				update: <UpdateRestrictionMessage as Codec>::deserialize(input)?,
			}),
			19 => unimplemented!(""),
			20 => unimplemented!(""),
			21 => Ok(Self::DepositRequest {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				investor: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
				amount: decode_be_bytes::<16, _, _>(input)?,
			}),
			22 => Ok(Self::RedeemRequest {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				investor: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
				amount: decode_be_bytes::<16, _, _>(input)?,
			}),
			23 => Ok(Self::FulfilledDepositRequest {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				investor: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
				currency_payout: decode_be_bytes::<16, _, _>(input)?,
				tranche_tokens_payout: decode_be_bytes::<16, _, _>(input)?,
				fulfilled_invest_amount: decode_be_bytes::<16, _, _>(input)?,
			}),
			24 => Ok(Self::FulfilledRedeemRequest {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				investor: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
				currency_payout: decode_be_bytes::<16, _, _>(input)?,
				tranche_tokens_payout: decode_be_bytes::<16, _, _>(input)?,
			}),
			25 => Ok(Self::CancelDepositRequest {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				investor: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
			}),
			26 => Ok(Self::CancelRedeemRequest {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				investor: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
			}),
			27 => Ok(Self::FulfilledCancelDepositRequest {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				investor: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
				currency_payout: decode_be_bytes::<16, _, _>(input)?,
				fulfilled_invest_amount: decode_be_bytes::<16, _, _>(input)?,
			}),
			28 => Ok(Self::FulfilledCancelRedeemRequest {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				investor: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
				tranche_tokens_payout: decode_be_bytes::<16, _, _>(input)?,
				fulfilled_redeem_amount: decode_be_bytes::<16, _, _>(input)?,
			}),
			29 => unimplemented!(""),
			_ => Err(parity_scale_codec::Error::from(
				"Unsupported decoding for this Message variant",
			)),
		}
	}
}

/// Decode a type that implements our custom [Codec] trait
pub fn deserialize<const S: usize, O: Codec, I: Input>(
	input: &mut I,
) -> Result<O, parity_scale_codec::Error> {
	let mut bytes = [0; S];
	input.read(&mut bytes[..])?;

	O::deserialize(&mut bytes.as_slice())
}

fn encoded_message(call_type: u8, fields: Vec<Vec<u8>>) -> Vec<u8> {
	let mut message: Vec<u8> = vec![];
	message.push(call_type);
	for x in fields {
		message.append(&mut x.clone());
	}

	message
}

#[cfg(test)]
mod tests {
	use cfg_primitives::{Balance, PoolId, TrancheId};
	use cfg_types::fixed_point::Ratio;
	use cfg_utils::vec_to_fixed_array;
	use hex::FromHex;
	use sp_runtime::traits::One;

	use super::*;
	use crate::{Domain, DomainAddress};

	pub type LiquidityPoolsMessage = Message<Domain, PoolId, TrancheId, Balance, Ratio>;

	const AMOUNT: Balance = 100000000000000000000000000;
	const POOL_ID: PoolId = 12378532;
	const TOKEN_ID: u128 = 246803579;

	#[test]
	fn invalid() {
		let msg = LiquidityPoolsMessage::Invalid;
		assert_eq!(msg.serialize(), vec![msg.call_type()]);
		assert_eq!(msg.serialize(), vec![0]);
	}

	#[test]
	fn encoding_domain() {
		// The Centrifuge substrate chain
		assert_eq!(
			hex::encode(Domain::Centrifuge.serialize()),
			"000000000000000000"
		);
		// Ethereum MainNet
		assert_eq!(
			hex::encode(Domain::EVM(1).serialize()),
			"010000000000000001"
		);
		// Moonbeam EVM chain
		assert_eq!(
			hex::encode(Domain::EVM(1284).serialize()),
			"010000000000000504"
		);
		// Avalanche Chain
		assert_eq!(
			hex::encode(Domain::EVM(43114).serialize()),
			"01000000000000a86a"
		);
	}

	#[test]
	fn add_currency_zero() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::AddAsset {
				currency: 0,
				evm_address: default_address_20(),
			},
			"09000000000000000000000000000000001231231231231231231231231231231231231231",
		)
	}

	#[test]
	fn add_currency() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::AddAsset {
				currency: TOKEN_ID,
				evm_address: default_address_20(),
			},
			"090000000000000000000000000eb5ec7b1231231231231231231231231231231231231231",
		)
	}

	#[test]
	fn add_pool_zero() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::AddPool { pool_id: 0 },
			"0a0000000000000000",
		)
	}

	#[test]
	fn add_pool_long() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::AddPool { pool_id: POOL_ID },
			"0a0000000000bce1a4",
		)
	}

	#[test]
	fn allow_asset() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::AllowAsset {
				currency: TOKEN_ID,
				pool_id: POOL_ID,
			},
			"0c0000000000bce1a40000000000000000000000000eb5ec7b",
		)
	}

	#[test]
	fn allow_asset_zero() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::AllowAsset {
				currency: 0,
				pool_id: 0,
			},
			"0c000000000000000000000000000000000000000000000000",
		)
	}

	#[test]
	fn add_tranche() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::AddTranche {
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
			LiquidityPoolsMessage::UpdateTrancheTokenPrice {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				currency: TOKEN_ID,
				price: Ratio::one(),
				computed_at: 1698131924,
			},
			"0e0000000000000001811acd5b3f17c06841c7e41e9e04cb1b0000000000000000000000000eb5ec7b00000000000000000de0b6b3a76400000000000065376fd4",
		)
	}

	#[test]
	fn update_member() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::UpdateRestriction{
				pool_id: 2,
				tranche_id: default_tranche_id(),
				update: UpdateRestrictionMessage::UpdateMember {
					member: default_address_32(),
					valid_until: 1706260138,
				}
			},
			"120000000000000002811acd5b3f17c06841c7e41e9e04cb1b0145645645645645645645645645645645645645645645645645645645645645640000000065b376aa",
		)
	}

	#[test]
	fn transfer_to_evm_address() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::Transfer {
					currency: TOKEN_ID,
					sender: default_address_32(),
					receiver: vec_to_fixed_array(default_address_20()),
					amount: AMOUNT,
				},
			"100000000000000000000000000eb5ec7b45645645645645645645645645645645645645645645645645645645645645641231231231231231231231231231231231231231000000000000000000000000000000000052b7d2dcc80cd2e4000000"
			);
	}

	#[test]
	fn transfer_to_centrifuge() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::Transfer {
        			currency: TOKEN_ID,
					sender: vec_to_fixed_array(default_address_20()),
					receiver: default_address_32(),
					amount: AMOUNT,
				},
			"100000000000000000000000000eb5ec7b12312312312312312312312312312312312312310000000000000000000000004564564564564564564564564564564564564564564564564564564564564564000000000052b7d2dcc80cd2e4000000"
			);
	}

	#[test]
	fn transfer_tranche_tokens_to_moonbeam() {
		let domain_address = DomainAddress::EVM(1284, default_address_20());

		test_encode_decode_identity(
			LiquidityPoolsMessage::TransferTrancheTokens {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				sender: default_address_32(),
				domain: domain_address.clone().into(),
				receiver: domain_address.address(),
				amount: AMOUNT,
			},
			"110000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640100000000000005041231231231231231231231231231231231231231000000000000000000000000000000000052b7d2dcc80cd2e4000000"
		);
	}

	#[test]
	fn transfer_tranche_tokens_to_centrifuge() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::TransferTrancheTokens {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				sender: vec_to_fixed_array(default_address_20()),
				domain: Domain::Centrifuge,
				receiver: default_address_32(),
				amount: AMOUNT,
			},
			"110000000000000001811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000000000000000000000004564564564564564564564564564564564564564564564564564564564564564000000000052b7d2dcc80cd2e4000000"
		)
	}

	#[test]
	fn deposit_request() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::DepositRequest {
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
	fn cancel_deposit_request() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::CancelDepositRequest {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				investor: default_address_32(),
				currency: TOKEN_ID,
			},
			"190000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640000000000000000000000000eb5ec7b",
		)
	}

	#[test]
	fn redeem_request() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::RedeemRequest {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				investor: default_address_32(),
				currency: TOKEN_ID,
				amount: AMOUNT,
			},
			"160000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640000000000000000000000000eb5ec7b000000000052b7d2dcc80cd2e4000000",
		)
	}

	#[test]
	fn cancel_redeem_request() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::CancelRedeemRequest {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				investor: default_address_32(),
				currency: TOKEN_ID,
			},
			"1a0000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640000000000000000000000000eb5ec7b",
		)
	}

	#[test]
	fn fulfilled_cancel_deposit_request() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::FulfilledCancelDepositRequest {
				pool_id: POOL_ID,
				tranche_id: default_tranche_id(),
				investor: vec_to_fixed_array(default_address_20()),
				currency: TOKEN_ID,
				currency_payout: AMOUNT / 2,
				fulfilled_invest_amount: AMOUNT / 4,
			},
			"1b0000000000bce1a4811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000000000000000000000000000000eb5ec7b0000000000295be96e64066972000000000000000014adf4b7320334b9000000",
		)
	}

	#[test]
	fn fulfilled_cancel_redeem_request() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::FulfilledCancelRedeemRequest {
				pool_id: POOL_ID,
				tranche_id: default_tranche_id(),
				investor: vec_to_fixed_array(default_address_20()),
				currency: TOKEN_ID,
				tranche_tokens_payout: AMOUNT / 2,
				fulfilled_redeem_amount: AMOUNT / 4,
			},
			"1c0000000000bce1a4811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000000000000000000000000000000eb5ec7b0000000000295be96e64066972000000000000000014adf4b7320334b9000000",
		)
	}

	#[test]
	fn fulfilled_deposit_request() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::FulfilledDepositRequest {
				pool_id: POOL_ID,
				tranche_id: default_tranche_id(),
				investor: vec_to_fixed_array(default_address_20()),
				currency: TOKEN_ID,
				currency_payout: AMOUNT,
				tranche_tokens_payout: AMOUNT / 2,
				fulfilled_invest_amount: AMOUNT / 4,
			},
			"170000000000bce1a4811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000000000000000000000000000000eb5ec7b000000000052b7d2dcc80cd2e40000000000000000295be96e64066972000000000000000014adf4b7320334b9000000",
		)
	}

	#[test]
	fn fulfilled_redeem_request() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::FulfilledRedeemRequest {
				pool_id: POOL_ID,
				tranche_id: default_tranche_id(),
				investor: vec_to_fixed_array(default_address_20()),
				currency: TOKEN_ID,
				currency_payout: AMOUNT,
				tranche_tokens_payout: AMOUNT / 2,
			},
			"180000000000bce1a4811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000000000000000000000000000000eb5ec7b000000000052b7d2dcc80cd2e40000000000000000295be96e64066972000000",
		)
	}

	#[test]
	fn schedule_upgrade() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::ScheduleUpgrade {
				contract: default_address_20(),
			},
			"051231231231231231231231231231231231231231",
		)
	}

	#[test]
	fn cancel_upgrade() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::CancelUpgrade {
				contract: default_address_20(),
			},
			"061231231231231231231231231231231231231231",
		)
	}

	#[test]
	fn update_tranche_token_metadata() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::UpdateTrancheTokenMetadata {
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
			LiquidityPoolsMessage::DisallowAsset {
				pool_id: POOL_ID,
				currency: TOKEN_ID,
			},
			"0d0000000000bce1a40000000000000000000000000eb5ec7b",
		)
	}

	#[test]
	fn disallow_asset_zero() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::DisallowAsset {
				pool_id: 0,
				currency: 0,
			},
			"0d000000000000000000000000000000000000000000000000",
		)
	}

	/// Verify the identity property of decode . encode on a Message value and
	/// that it in fact encodes to and can be decoded from a given hex string.
	fn test_encode_decode_identity(
		msg: Message<Domain, PoolId, TrancheId, Balance, Ratio>,
		expected_hex: &str,
	) {
		let encoded = msg.serialize();
		assert_eq!(hex::encode(encoded.clone()), expected_hex);

		let decoded: Message<Domain, PoolId, TrancheId, Balance, Ratio> = Message::deserialize(
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
