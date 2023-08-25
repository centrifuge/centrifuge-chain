use cfg_primitives::Moment;
use cfg_traits::liquidity_pools::Codec;
use cfg_utils::{decode, decode_be_bytes, encode_be};
use codec::{Decode, Encode, Input};
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
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Message<Domain, PoolId, TrancheId, Balance, Rate>
where
	Domain: Codec,
	PoolId: Encode + Decode,
	TrancheId: Encode + Decode,
	Balance: Encode + Decode,
	Rate: Encode + Decode,
{
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
		pool_id: PoolId,
	},
	/// Allow a currency to be used as a pool currency and to invest in a pool.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	AllowPoolCurrency {
		pool_id: PoolId,
		currency: u128,
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
		price: Rate,
	},
	/// Update the price of a tranche token on the target domain.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	UpdateTrancheTokenPrice {
		pool_id: PoolId,
		tranche_id: TrancheId,
		price: Rate,
	},
	/// Whitelist an address for the specified pair of pool and tranche token on
	/// the target domain.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	UpdateMember {
		pool_id: PoolId,
		tranche_id: TrancheId,
		member: Address,
		valid_until: Moment,
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
	/// Increase the invest order amount for the specified pair of pool and
	/// tranche token.
	///
	/// Directionality: Centrifuge <- EVM Domain.
	IncreaseInvestOrder {
		pool_id: PoolId,
		tranche_id: TrancheId,
		investor: Address,
		currency: u128,
		amount: Balance,
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
	IncreaseRedeemOrder {
		pool_id: PoolId,
		tranche_id: TrancheId,
		investor: Address,
		currency: u128,
		amount: Balance,
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
		pool_id: PoolId,
		tranche_id: TrancheId,
		investor: Address,
		currency: u128,
		amount: Balance,
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
		pool_id: PoolId,
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
		pool_id: PoolId,
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
		pool_id: PoolId,
		/// The tranche id
		tranche_id: TrancheId,
		/// The investor's address
		investor: Address,
		/// The currency in which `DecreaseInvestOrder` was realised
		currency: u128,
		/// The amount of `currency` that was actually executed in the original
		/// `DecreaseInvestOrder` message, i.e., the amount by which the
		/// investment order was actually decreased by.
		currency_payout: Balance,
	},
	/// The message sent back to the domain from which a `DecreaseRedeemOrder`
	/// message was received, ensuring the correct state update on said domain
	/// and that the `investor`'s wallet is updated accordingly.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	ExecutedDecreaseRedeemOrder {
		/// The pool id
		pool_id: PoolId,
		/// The tranche id
		tranche_id: TrancheId,
		/// The investor's address
		investor: Address,
		/// The currency in which `DecreaseRedeemOrder` was realised
		currency: u128,
		/// The amount of tranche tokens that was actually executed in the
		/// original `DecreaseRedeemOrder` message, i.e., the amount by which
		/// the redeem order was actually decreased by.
		tranche_tokens_payout: Balance,
	},
	/// The message sent back to the domain from which a `CollectInvest` message
	/// has been received, which will ensure the `investor` gets the payout
	/// respective to their investment.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	ExecutedCollectInvest {
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
	},
	/// The message sent back to the domain from which a `CollectRedeem` message
	/// has been received, which will ensure the `investor` gets the payout
	/// respective to their redemption.
	///
	/// Directionality: Centrifuge -> EVM Domain.
	ExecutedCollectRedeem {
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
		pool_id: PoolId,
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
		pool_id: PoolId,
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
}

impl<
		Domain: Codec,
		PoolId: Encode + Decode,
		TrancheId: Encode + Decode,
		Balance: Encode + Decode,
		Rate: Encode + Decode,
	> Message<Domain, PoolId, TrancheId, Balance, Rate>
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
			Self::AddCurrency { .. } => 1,
			Self::AddPool { .. } => 2,
			Self::AllowPoolCurrency { .. } => 3,
			Self::AddTranche { .. } => 4,
			Self::UpdateTrancheTokenPrice { .. } => 5,
			Self::UpdateMember { .. } => 6,
			Self::Transfer { .. } => 7,
			Self::TransferTrancheTokens { .. } => 8,
			Self::IncreaseInvestOrder { .. } => 9,
			Self::DecreaseInvestOrder { .. } => 10,
			Self::IncreaseRedeemOrder { .. } => 11,
			Self::DecreaseRedeemOrder { .. } => 12,
			Self::CollectInvest { .. } => 13,
			Self::CollectRedeem { .. } => 14,
			Self::ExecutedDecreaseInvestOrder { .. } => 15,
			Self::ExecutedDecreaseRedeemOrder { .. } => 16,
			Self::ExecutedCollectInvest { .. } => 17,
			Self::ExecutedCollectRedeem { .. } => 18,
			Self::CancelInvestOrder { .. } => 19,
			Self::CancelRedeemOrder { .. } => 20,
			Self::ScheduleUpgrade { .. } => 21,
		}
	}
}

impl<
		Domain: Codec,
		PoolId: Encode + Decode,
		TrancheId: Encode + Decode,
		Balance: Encode + Decode,
		Rate: Encode + Decode,
	> Codec for Message<Domain, PoolId, TrancheId, Balance, Rate>
{
	fn serialize(&self) -> Vec<u8> {
		match self {
			Message::Invalid => vec![self.call_type()],
			Message::AddCurrency {
				currency,
				evm_address,
			} => encoded_message(
				self.call_type(),
				vec![encode_be(currency), evm_address.to_vec()],
			),
			Message::AddPool { pool_id } => {
				encoded_message(self.call_type(), vec![encode_be(pool_id)])
			}
			Message::AllowPoolCurrency { pool_id, currency } => encoded_message(
				self.call_type(),
				vec![encode_be(pool_id), encode_be(currency)],
			),
			Message::AddTranche {
				pool_id,
				tranche_id,
				token_name,
				token_symbol,
				decimals,
				price,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					token_name.encode(),
					token_symbol.encode(),
					decimals.encode(),
					encode_be(price),
				],
			),
			Message::UpdateTrancheTokenPrice {
				pool_id,
				tranche_id,
				price,
			} => encoded_message(
				self.call_type(),
				vec![encode_be(pool_id), tranche_id.encode(), encode_be(price)],
			),
			Message::UpdateMember {
				pool_id,
				tranche_id,
				member,
				valid_until,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					member.to_vec(),
					valid_until.to_be_bytes().to_vec(),
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
			Message::IncreaseInvestOrder {
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
			Message::DecreaseInvestOrder {
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
			Message::IncreaseRedeemOrder {
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
			Message::DecreaseRedeemOrder {
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
			Message::CollectInvest {
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
			Message::CollectRedeem {
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
			Message::ExecutedDecreaseInvestOrder {
				pool_id,
				tranche_id,
				investor,
				currency,
				currency_payout,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					investor.to_vec(),
					encode_be(currency),
					encode_be(currency_payout),
				],
			),
			Message::ExecutedDecreaseRedeemOrder {
				pool_id,
				tranche_id,
				investor,
				currency,
				tranche_tokens_payout,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					investor.to_vec(),
					encode_be(currency),
					encode_be(tranche_tokens_payout),
				],
			),
			Message::ExecutedCollectInvest {
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
			Message::ExecutedCollectRedeem {
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
			Message::CancelInvestOrder {
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
			Message::CancelRedeemOrder {
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
			Message::ScheduleUpgrade { contract } => {
				encoded_message(self.call_type(), vec![contract.to_vec()])
			}
		}
	}

	fn deserialize<I: Input>(input: &mut I) -> Result<Self, codec::Error> {
		let call_type = input.read_byte()?;

		match call_type {
			0 => Ok(Self::Invalid),
			1 => Ok(Self::AddCurrency {
				currency: decode_be_bytes::<16, _, _>(input)?,
				evm_address: decode::<20, _, _>(input)?,
			}),
			2 => Ok(Self::AddPool {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
			}),
			3 => Ok(Self::AllowPoolCurrency {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
			}),
			4 => Ok(Self::AddTranche {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				token_name: decode::<TOKEN_NAME_SIZE, _, _>(input)?,
				token_symbol: decode::<TOKEN_SYMBOL_SIZE, _, _>(input)?,
				decimals: decode::<1, _, _>(input)?,
				price: decode_be_bytes::<16, _, _>(input)?,
			}),
			5 => Ok(Self::UpdateTrancheTokenPrice {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				price: decode_be_bytes::<16, _, _>(input)?,
			}),
			6 => Ok(Self::UpdateMember {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				member: decode::<32, _, _>(input)?,
				valid_until: decode_be_bytes::<8, _, _>(input)?,
			}),
			7 => Ok(Self::Transfer {
				currency: decode_be_bytes::<16, _, _>(input)?,
				sender: decode::<32, _, _>(input)?,
				receiver: decode::<32, _, _>(input)?,
				amount: decode_be_bytes::<16, _, _>(input)?,
			}),
			8 => Ok(Self::TransferTrancheTokens {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				sender: decode::<32, _, _>(input)?,
				domain: deserialize::<9, _, _>(input)?,
				receiver: decode::<32, _, _>(input)?,
				amount: decode_be_bytes::<16, _, _>(input)?,
			}),
			9 => Ok(Self::IncreaseInvestOrder {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				investor: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
				amount: decode_be_bytes::<16, _, _>(input)?,
			}),
			10 => Ok(Self::DecreaseInvestOrder {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				investor: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
				amount: decode_be_bytes::<16, _, _>(input)?,
			}),
			11 => Ok(Self::IncreaseRedeemOrder {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				investor: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
				amount: decode_be_bytes::<16, _, _>(input)?,
			}),
			12 => Ok(Self::DecreaseRedeemOrder {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				investor: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
				amount: decode_be_bytes::<16, _, _>(input)?,
			}),
			13 => Ok(Self::CollectInvest {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				investor: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
			}),
			14 => Ok(Self::CollectRedeem {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				investor: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
			}),
			15 => Ok(Self::ExecutedDecreaseInvestOrder {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				investor: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
				currency_payout: decode_be_bytes::<16, _, _>(input)?,
			}),
			16 => Ok(Self::ExecutedDecreaseRedeemOrder {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				investor: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
				tranche_tokens_payout: decode_be_bytes::<16, _, _>(input)?,
			}),
			17 => Ok(Self::ExecutedCollectInvest {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				investor: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
				currency_payout: decode_be_bytes::<16, _, _>(input)?,
				tranche_tokens_payout: decode_be_bytes::<16, _, _>(input)?,
			}),
			18 => Ok(Self::ExecutedCollectRedeem {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				investor: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
				currency_payout: decode_be_bytes::<16, _, _>(input)?,
				tranche_tokens_payout: decode_be_bytes::<16, _, _>(input)?,
			}),
			19 => Ok(Self::CancelInvestOrder {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				investor: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
			}),
			20 => Ok(Self::CancelRedeemOrder {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				investor: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
			}),
			21 => Ok(Self::ScheduleUpgrade {
				contract: decode::<20, _, _>(input)?,
			}),
			_ => Err(codec::Error::from(
				"Unsupported decoding for this Message variant",
			)),
		}
	}
}

/// Decode a type that implements our custom [Codec] trait
pub fn deserialize<const S: usize, O: Codec, I: Input>(input: &mut I) -> Result<O, codec::Error> {
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
	use cfg_types::fixed_point::Rate;
	use cfg_utils::vec_to_fixed_array;
	use hex::FromHex;
	use sp_runtime::traits::One;

	use super::*;
	use crate::{Domain, DomainAddress};

	pub type LiquidityPoolsMessage = Message<Domain, PoolId, TrancheId, Balance, Rate>;

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
			LiquidityPoolsMessage::AddCurrency {
				currency: 0,
				evm_address: default_address_20(),
			},
			"01000000000000000000000000000000001231231231231231231231231231231231231231",
		)
	}

	#[test]
	fn add_currency() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::AddCurrency {
				currency: TOKEN_ID,
				evm_address: default_address_20(),
			},
			"010000000000000000000000000eb5ec7b1231231231231231231231231231231231231231",
		)
	}

	#[test]
	fn add_pool_zero() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::AddPool { pool_id: 0 },
			"020000000000000000",
		)
	}

	#[test]
	fn add_pool_long() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::AddPool { pool_id: POOL_ID },
			"020000000000bce1a4",
		)
	}

	#[test]
	fn allow_pool_currency() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::AllowPoolCurrency {
				currency: TOKEN_ID,
				pool_id: POOL_ID,
			},
			"030000000000bce1a40000000000000000000000000eb5ec7b",
		)
	}

	#[test]
	fn allow_pool_currency_zero() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::AllowPoolCurrency {
				currency: 0,
				pool_id: 0,
			},
			"03000000000000000000000000000000000000000000000000",
		)
	}

	#[test]
	fn add_tranche() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::AddTranche {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				token_name: vec_to_fixed_array("Some Name".to_string().into_bytes()),
				token_symbol: vec_to_fixed_array("SYMBOL".to_string().into_bytes()),
				decimals: 15,
				price: Rate::one(),
			},
			"040000000000000001811acd5b3f17c06841c7e41e9e04cb1b536f6d65204e616d65000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000053594d424f4c00000000000000000000000000000000000000000000000000000f00000000033b2e3c9fd0803ce8000000",
		)
	}

	#[test]
	fn update_tranche_token_price() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::UpdateTrancheTokenPrice {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				price: Rate::one(),
			},
			"050000000000000001811acd5b3f17c06841c7e41e9e04cb1b00000000033b2e3c9fd0803ce8000000",
		)
	}

	#[test]
	fn update_member() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::UpdateMember {
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
			LiquidityPoolsMessage::Transfer {
					currency: TOKEN_ID,
					sender: default_address_32(),
					receiver: vec_to_fixed_array(default_address_20().to_vec()),
					amount: AMOUNT,
				},
			"070000000000000000000000000eb5ec7b45645645645645645645645645645645645645645645645645645645645645641231231231231231231231231231231231231231000000000000000000000000000000000052b7d2dcc80cd2e4000000"
			);
	}

	#[test]
	fn transfer_to_centrifuge() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::Transfer {
        			currency: TOKEN_ID,
					sender: vec_to_fixed_array(default_address_20().to_vec()),
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
			LiquidityPoolsMessage::TransferTrancheTokens {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				sender: default_address_32(),
				domain: domain_address.clone().into(),
				receiver: domain_address.address(),
				amount: AMOUNT,
			},
			"080000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640100000000000005041231231231231231231231231231231231231231000000000000000000000000000000000052b7d2dcc80cd2e4000000"
		);
	}

	#[test]
	fn transfer_tranche_tokens_to_centrifuge() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::TransferTrancheTokens {
				pool_id: 1,
				tranche_id: default_tranche_id(),
				sender: vec_to_fixed_array(default_address_20().to_vec()),
				domain: Domain::Centrifuge,
				receiver: default_address_32(),
				amount: AMOUNT,
			},
			"080000000000000001811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000000000000000000000004564564564564564564564564564564564564564564564564564564564564564000000000052b7d2dcc80cd2e4000000"
		)
	}

	#[test]
	fn increase_invest_order() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::IncreaseInvestOrder {
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
			LiquidityPoolsMessage::DecreaseInvestOrder {
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
			LiquidityPoolsMessage::CancelInvestOrder {
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
			LiquidityPoolsMessage::IncreaseRedeemOrder {
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
			LiquidityPoolsMessage::DecreaseRedeemOrder {
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
			LiquidityPoolsMessage::CancelRedeemOrder {
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
			LiquidityPoolsMessage::CollectInvest {
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
			LiquidityPoolsMessage::CollectRedeem {
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
			LiquidityPoolsMessage::ExecutedDecreaseInvestOrder {
				pool_id: POOL_ID,
				tranche_id: default_tranche_id(),
				investor: vec_to_fixed_array(default_address_20().to_vec()),
				currency: TOKEN_ID,
				currency_payout: AMOUNT / 2,
			},
			"0f0000000000bce1a4811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000000000000000000000000000000eb5ec7b0000000000295be96e64066972000000",
		)
	}

	#[test]
	fn executed_decrease_redeem_order() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::ExecutedDecreaseRedeemOrder {
				pool_id: POOL_ID,
				tranche_id: default_tranche_id(),
				investor: vec_to_fixed_array(default_address_20().to_vec()),
				currency: TOKEN_ID,
				tranche_tokens_payout: AMOUNT / 2,
			},
			"100000000000bce1a4811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000000000000000000000000000000eb5ec7b0000000000295be96e64066972000000",
		)
	}

	#[test]
	fn executed_collect_invest() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::ExecutedCollectInvest {
				pool_id: POOL_ID,
				tranche_id: default_tranche_id(),
				investor: vec_to_fixed_array(default_address_20().to_vec()),
				currency: TOKEN_ID,
				currency_payout: AMOUNT,
				tranche_tokens_payout: AMOUNT / 2,
			},
			"110000000000bce1a4811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000000000000000000000000000000eb5ec7b000000000052b7d2dcc80cd2e40000000000000000295be96e64066972000000",
		)
	}

	#[test]
	fn executed_collect_redeem() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::ExecutedCollectRedeem {
				pool_id: POOL_ID,
				tranche_id: default_tranche_id(),
				investor: vec_to_fixed_array(default_address_20().to_vec()),
				currency: TOKEN_ID,
				currency_payout: AMOUNT,
				tranche_tokens_payout: AMOUNT / 2,
			},
			"120000000000bce1a4811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000000000000000000000000000000eb5ec7b000000000052b7d2dcc80cd2e40000000000000000295be96e64066972000000",
		)
	}

	#[test]
	fn schedule_upgrade() {
		test_encode_decode_identity(
			LiquidityPoolsMessage::ScheduleUpgrade {
				contract: default_address_20(),
			},
			"151231231231231231231231231231231231231231",
		)
	}

	/// Verify the identity property of decode . encode on a Message value and
	/// that it in fact encodes to and can be decoded from a given hex string.
	fn test_encode_decode_identity(
		msg: Message<Domain, PoolId, TrancheId, Balance, Rate>,
		expected_hex: &str,
	) {
		let encoded = msg.serialize();
		assert_eq!(hex::encode(encoded.clone()), expected_hex);

		let decoded: Message<Domain, PoolId, TrancheId, Balance, Rate> = Message::deserialize(
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
