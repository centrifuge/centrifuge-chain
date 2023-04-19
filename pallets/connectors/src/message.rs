use cfg_primitives::Moment;
use cfg_utils::{decode, decode_be_bytes, encode_be};
use codec::{Decode, Encode, Input};
use scale_info::TypeInfo;
use sp_std::{vec, vec::Vec};

use crate::Codec;

/// Address type
/// Note: It can be used to represent any address type with a length <= 32 bytes;
/// For example, it can represent an Ethereum address (20-bytes long) by padding it with 12 zeros.
type Address = [u8; 32];

/// The fixed size for the array representing a tranche token name
pub const TOKEN_NAME_SIZE: usize = 128;

// The fixed size for the array representing a tranche token symbol
pub const TOKEN_SYMBOL_SIZE: usize = 32;

/// A Connector Message
///
/// A connector message requires a custom decoding & encoding, meeting the Connector Generic
/// Message Passing Format (CGMPF): Every message is encoded with a u8 at head flagging the
/// message type, followed by its field. Integers are big-endian encoded and enum values
/// (such as `[crate::Domain]`) also have a custom CGMPF implementation, aiming for a
/// fixed-size encoded representation for each message variant.
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
	AddCurrency {
		currency: u128,
		evm_address: [u8; 20],
	},
	AddPool {
		pool_id: PoolId,
		currency: u128,
	},
	AllowPoolCurrency {
		currency: u128,
		pool_id: PoolId,
	},
	AddTranche {
		pool_id: PoolId,
		tranche_id: TrancheId,
		decimals: u8,
		token_name: [u8; TOKEN_NAME_SIZE],
		token_symbol: [u8; TOKEN_SYMBOL_SIZE],
		price: Rate,
	},
	UpdateTrancheTokenPrice {
		pool_id: PoolId,
		tranche_id: TrancheId,
		price: Rate,
	},
	UpdateMember {
		pool_id: PoolId,
		tranche_id: TrancheId,
		address: Address,
		valid_until: Moment,
	},
	// Bidirectional: Domain must not accept every incoming token.
	// Sender must ensure beforehand that the receiver will not reject
	Transfer {
		currency: u128,
		source_address: Address,
		destination_address: Address,
		amount: Balance,
	},
	TransferTrancheTokens {
		domain: Domain,
		pool_id: PoolId,
		tranche_id: TrancheId,
		source_address: Address,
		destination_address: Address,
		amount: Balance,
	},
	IncreaseInvestOrder {
		pool_id: PoolId,
		tranche_id: TrancheId,
		address: Address,
		currency: u128,
		amount: Balance,
	},
	DecreaseInvestOrder {
		pool_id: PoolId,
		tranche_id: TrancheId,
		address: Address,
		currency: u128,
		amount: Balance,
	},
	IncreaseRedeemOrder {
		pool_id: PoolId,
		tranche_id: TrancheId,
		address: Address,
		currency: u128,
		amount: Balance,
	},
	DecreaseRedeemOrder {
		pool_id: PoolId,
		tranche_id: TrancheId,
		address: Address,
		currency: u128,
		amount: Balance,
	},
	CollectRedem {
		pool_id: PoolId,
		tranche_id: TrancheId,
		address: Address,
	},
	CollectForRedeem {
		pool_id: PoolId,
		tranche_id: TrancheId,
		caller: Address,
		user: Address,
	},
	CollectInvest {
		pool_id: PoolId,
		tranche_id: TrancheId,
		address: Address,
	},
	CollectForInvest {
		pool_id: PoolId,
		tranche_id: TrancheId,
		caller: Address,
		user: Address,
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
	/// The call type that identifies a specific Message variant. This value is used
	/// to encode/decode a Message to/from a bytearray, whereas the head of the bytearray
	/// is the call type, followed by each message's param values.
	///
	/// NOTE: Each message must immutably  map to the same u8. Messages are decoded
	/// in other domains and MUST follow the defined standard.
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
			Self::CollectRedem { .. } => 13,
			Self::CollectForRedeem { .. } => 14,
			Self::CollectInvest { .. } => 15,
			Self::CollectForInvest { .. } => 16,
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
			Message::AddPool { pool_id, currency } => encoded_message(
				self.call_type(),
				vec![encode_be(pool_id), encode_be(currency)],
			),
			Message::AllowPoolCurrency { currency, pool_id } => encoded_message(
				self.call_type(),
				vec![encode_be(currency), encode_be(pool_id)],
			),
			Message::AddTranche {
				pool_id,
				tranche_id,
				decimals,
				token_name,
				token_symbol,
				price,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					decimals.encode(),
					token_name.encode(),
					token_symbol.encode(),
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
				address,
				valid_until,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					address.to_vec(),
					valid_until.to_be_bytes().to_vec(),
				],
			),
			Message::Transfer {
				currency: token,
				source_address,
				destination_address,
				amount,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(token),
					source_address.to_vec(),
					destination_address.to_vec(),
					encode_be(amount),
				],
			),
			Message::TransferTrancheTokens {
				domain,
				pool_id,
				tranche_id,
				source_address,
				destination_address,
				amount,
			} => encoded_message(
				self.call_type(),
				vec![
					domain.serialize(),
					encode_be(pool_id),
					tranche_id.encode(),
					source_address.to_vec(),
					destination_address.to_vec(),
					encode_be(amount),
				],
			),
			Message::IncreaseInvestOrder {
				pool_id,
				tranche_id,
				address,
				currency: token,
				amount,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					address.to_vec(),
					encode_be(token),
					encode_be(amount),
				],
			),
			Message::DecreaseInvestOrder {
				pool_id,
				tranche_id,
				address,
				currency: token,
				amount,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					address.to_vec(),
					encode_be(token),
					encode_be(amount),
				],
			),
			Message::IncreaseRedeemOrder {
				pool_id,
				tranche_id,
				address,
				currency: token,
				amount,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					address.to_vec(),
					encode_be(token),
					encode_be(amount),
				],
			),
			Message::DecreaseRedeemOrder {
				pool_id,
				tranche_id,
				address,
				currency: token,
				amount,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					address.to_vec(),
					encode_be(token),
					encode_be(amount),
				],
			),
			Message::CollectRedem {
				pool_id,
				tranche_id,
				address,
			} => encoded_message(
				self.call_type(),
				vec![encode_be(pool_id), tranche_id.encode(), address.to_vec()],
			),
			Message::CollectForRedeem {
				pool_id,
				tranche_id,
				caller: call_address,
				user: collect_address,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					call_address.to_vec(),
					collect_address.to_vec(),
				],
			),
			Message::CollectInvest {
				pool_id,
				tranche_id,
				address,
			} => encoded_message(
				self.call_type(),
				vec![encode_be(pool_id), tranche_id.encode(), address.to_vec()],
			),
			Message::CollectForInvest {
				pool_id,
				tranche_id,
				caller: call_address,
				user: collect_address,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					call_address.to_vec(),
					collect_address.to_vec(),
				],
			),
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
				currency: decode_be_bytes::<16, _, _>(input)?,
			}),
			3 => Ok(Self::AllowPoolCurrency {
				currency: decode_be_bytes::<16, _, _>(input)?,
				pool_id: decode_be_bytes::<8, _, _>(input)?,
			}),
			4 => Ok(Self::AddTranche {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				decimals: decode::<1, _, _>(input)?,
				token_name: decode::<TOKEN_NAME_SIZE, _, _>(input)?,
				token_symbol: decode::<TOKEN_SYMBOL_SIZE, _, _>(input)?,
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
				address: decode::<32, _, _>(input)?,
				valid_until: decode_be_bytes::<8, _, _>(input)?,
			}),
			7 => Ok(Self::Transfer {
				currency: decode_be_bytes::<16, _, _>(input)?,
				source_address: decode::<32, _, _>(input)?,
				destination_address: decode::<32, _, _>(input)?,
				amount: decode_be_bytes::<16, _, _>(input)?,
			}),
			8 => Ok(Self::TransferTrancheTokens {
				domain: deserialize::<9, _, _>(input)?,
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				source_address: decode::<32, _, _>(input)?,
				destination_address: decode::<32, _, _>(input)?,
				amount: decode_be_bytes::<16, _, _>(input)?,
			}),
			9 => Ok(Self::IncreaseInvestOrder {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				address: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
				amount: decode_be_bytes::<16, _, _>(input)?,
			}),
			10 => Ok(Self::DecreaseInvestOrder {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				address: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
				amount: decode_be_bytes::<16, _, _>(input)?,
			}),
			11 => Ok(Self::IncreaseRedeemOrder {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				address: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
				amount: decode_be_bytes::<16, _, _>(input)?,
			}),
			12 => Ok(Self::DecreaseRedeemOrder {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				address: decode::<32, _, _>(input)?,
				currency: decode_be_bytes::<16, _, _>(input)?,
				amount: decode_be_bytes::<16, _, _>(input)?,
			}),
			13 => Ok(Self::CollectRedem {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				address: decode::<32, _, _>(input)?,
			}),
			14 => Ok(Self::CollectForRedeem {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				caller: decode::<32, _, _>(input)?,
				user: decode::<32, _, _>(input)?,
			}),
			15 => Ok(Self::CollectInvest {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				address: decode::<32, _, _>(input)?,
			}),
			16 => Ok(Self::CollectForInvest {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				caller: decode::<32, _, _>(input)?,
				user: decode::<32, _, _>(input)?,
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

// Converts a 32 byte AccountId to its byte-array equivalent form.
pub(crate) fn account_to_bytes<AccountId>(
	account: &AccountId,
) -> Result<[u8; 32], sp_runtime::DispatchError>
where
	AccountId: Encode,
{
	let account_vec = account.encode();
	frame_support::ensure!(account_vec.len() == 32, "AccountId must be 32 bytes.");
	let mut bytes = [0u8; 32];
	bytes.copy_from_slice(&account_vec);
	Ok(bytes)
}

#[cfg(test)]
mod tests {
	use cfg_primitives::{Balance, PoolId, TrancheId};
	use cfg_types::fixed_point::Rate;
	use cfg_utils::vec_to_fixed_array;
	use hex::FromHex;
	use sp_runtime::traits::One;

	use super::*;
	use crate::{Codec, Domain, DomainAddress};

	pub type ConnectorMessage = Message<Domain, PoolId, TrancheId, Balance, Rate>;

	const AMOUNT: Balance = 100000000000000000000000000;
	const POOL_ID: PoolId = 12378532;
	const TOKEN_ID: u128 = 246803579;
	const TRANCHE_HEX: &str = "811acd5b3f17c06841c7e41e9e04cb1b";
	const ADDRESS_20_HEX: &str = "1231231231231231231231231231231231231231";
	const ADDRESS_32_HEX: &str = "4564564564564564564564564564564564564564564564564564564564564564";

	#[test]
	fn invalid() {
		let msg = ConnectorMessage::Invalid;
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
	fn add_pool_zero() {
		test_encode_decode_identity(
			ConnectorMessage::AddPool {
				pool_id: 0,
				currency: 0,
			},
			"02000000000000000000000000000000000000000000000000",
		)
	}

	#[test]
	fn add_pool_long() {
		test_encode_decode_identity(
			ConnectorMessage::AddPool {
				pool_id: POOL_ID,
				currency: TOKEN_ID,
			},
			"020000000000bce1a40000000000000000000000000eb5ec7b",
		)
	}

	#[test]
	fn add_currency() {
		test_encode_decode_identity(
			ConnectorMessage::AddCurrency {
				currency: TOKEN_ID,
				evm_address: address20_from_hex(ADDRESS_20_HEX),
			},
			"010000000000000000000000000eb5ec7b1231231231231231231231231231231231231231",
		)
	}
	#[test]
	fn allow_pool_currency() {
		test_encode_decode_identity(
			ConnectorMessage::AllowPoolCurrency {
				currency: TOKEN_ID,
				pool_id: 1,
			},
			"030000000000000000000000000eb5ec7b0000000000000001",
		)
	}

	#[test]
	fn add_tranche() {
		test_encode_decode_identity(
			ConnectorMessage::AddTranche {
				pool_id: 1,
				tranche_id: tranche_id_from_hex(TRANCHE_HEX),
				decimals: 15,
				token_name: vec_to_fixed_array("Some Name".to_string().into_bytes()),
				token_symbol: vec_to_fixed_array("SYMBOL".to_string().into_bytes()),
				price: Rate::one(),
			},
			"040000000000000001811acd5b3f17c06841c7e41e9e04cb1b0f536f6d65204e616d65000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000053594d424f4c000000000000000000000000000000000000000000000000000000000000033b2e3c9fd0803ce8000000",
		)
	}

	#[test]
	fn update_token_price() {
		test_encode_decode_identity(
			ConnectorMessage::UpdateTrancheTokenPrice {
				pool_id: 1,
				tranche_id: tranche_id_from_hex(TRANCHE_HEX),
				price: Rate::one(),
			},
			"050000000000000001811acd5b3f17c06841c7e41e9e04cb1b00000000033b2e3c9fd0803ce8000000",
		)
	}

	#[test]
	fn update_member() {
		test_encode_decode_identity(
				ConnectorMessage::UpdateMember {
					pool_id: 2,
					tranche_id: tranche_id_from_hex(TRANCHE_HEX),
					address: address32_from_hex(ADDRESS_32_HEX),
					valid_until: 1706260138,
				},
				"060000000000000002811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640000000065b376aa"
			)
	}

	#[test]
	fn transfer_tranche_tokens_to_moonbeam() {
		let domain_address = DomainAddress::EVM(1284, address20_from_hex(ADDRESS_20_HEX));

		test_encode_decode_identity(
				ConnectorMessage::TransferTrancheTokens {
					pool_id: 1,
					tranche_id: tranche_id_from_hex(TRANCHE_HEX),
					domain: domain_address.clone().into(),
					source_address: address32_from_hex(ADDRESS_32_HEX),
					destination_address: domain_address.address(),
					amount: AMOUNT,
				},
				"080100000000000005040000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645641231231231231231231231231231231231231231000000000000000000000000000000000052b7d2dcc80cd2e4000000"
			);
	}

	#[test]
	fn transfer_tranch_tokens_to_centrifuge() {
		test_encode_decode_identity(
				ConnectorMessage::TransferTrancheTokens {
					pool_id: 1,
					tranche_id: tranche_id_from_hex(TRANCHE_HEX),
					domain: Domain::Centrifuge,
					source_address: vec_to_fixed_array(address20_from_hex(ADDRESS_20_HEX).to_vec()),
					destination_address: address32_from_hex(ADDRESS_32_HEX),
					amount: AMOUNT,
				},
				"080000000000000000000000000000000001811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000004564564564564564564564564564564564564564564564564564564564564564000000000052b7d2dcc80cd2e4000000"
			)
	}

	#[test]
	fn transfer_to_moonbeam() {
		let domain_address = DomainAddress::EVM(1284, address20_from_hex(ADDRESS_20_HEX));

		test_encode_decode_identity(
				ConnectorMessage::Transfer {
					destination_address: domain_address.address(),
					source_address: address32_from_hex(ADDRESS_32_HEX),
					amount: AMOUNT,
        			currency: TOKEN_ID,
				},
				"070000000000000000000000000eb5ec7b45645645645645645645645645645645645645645645645645645645645645641231231231231231231231231231231231231231000000000000000000000000000000000052b7d2dcc80cd2e4000000"
			);
	}

	#[test]
	fn transfer_to_centrifuge() {
		test_encode_decode_identity(
				ConnectorMessage::Transfer {
					source_address: vec_to_fixed_array(address20_from_hex(ADDRESS_20_HEX).to_vec()),
					destination_address: address32_from_hex(ADDRESS_32_HEX),
					amount: AMOUNT,
        			currency: TOKEN_ID,
				},
				"070000000000000000000000000eb5ec7b12312312312312312312312312312312312312310000000000000000000000004564564564564564564564564564564564564564564564564564564564564564000000000052b7d2dcc80cd2e4000000"
			);
	}

	#[test]
	fn increase_invest_order() {
		test_encode_decode_identity(
			ConnectorMessage::IncreaseInvestOrder {
				pool_id: 1,
				tranche_id: tranche_id_from_hex(TRANCHE_HEX),
				address: address32_from_hex(ADDRESS_32_HEX),
				currency: TOKEN_ID,
				amount: AMOUNT,
			},
			"090000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640000000000000000000000000eb5ec7b000000000052b7d2dcc80cd2e4000000",
		)
	}

	#[test]
	fn decrease_invest_order() {
		test_encode_decode_identity(
			ConnectorMessage::DecreaseInvestOrder {
				pool_id: 1,
				tranche_id: tranche_id_from_hex(TRANCHE_HEX),
				address: address32_from_hex(ADDRESS_32_HEX),
				currency: TOKEN_ID,
				amount: AMOUNT,
			},
			"0a0000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640000000000000000000000000eb5ec7b000000000052b7d2dcc80cd2e4000000",
		)
	}

	#[test]
	fn increase_redeem_order() {
		test_encode_decode_identity(
			ConnectorMessage::IncreaseRedeemOrder {
				pool_id: 1,
				tranche_id: tranche_id_from_hex(TRANCHE_HEX),
				address: address32_from_hex(ADDRESS_32_HEX),
				currency: TOKEN_ID,
				amount: AMOUNT,
			},
			"0b0000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640000000000000000000000000eb5ec7b000000000052b7d2dcc80cd2e4000000",
		)
	}

	#[test]
	fn decrease_redeem_order() {
		test_encode_decode_identity(
			ConnectorMessage::DecreaseRedeemOrder {
				pool_id: 1,
				tranche_id: tranche_id_from_hex(TRANCHE_HEX),
				address: address32_from_hex(ADDRESS_32_HEX),
				currency: TOKEN_ID,
				amount: AMOUNT,
			},
			"0c0000000000000001811acd5b3f17c06841c7e41e9e04cb1b45645645645645645645645645645645645645645645645645645645645645640000000000000000000000000eb5ec7b000000000052b7d2dcc80cd2e4000000",
		)
	}

	#[test]
	fn collect_for_redeem() {
		test_encode_decode_identity(
			ConnectorMessage::CollectForRedeem {
				pool_id: 1,
				tranche_id: tranche_id_from_hex(TRANCHE_HEX),
				caller: vec_to_fixed_array(address20_from_hex(ADDRESS_20_HEX).to_vec()),
				user: address32_from_hex(ADDRESS_32_HEX),
			},
			"0e0000000000000001811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000004564564564564564564564564564564564564564564564564564564564564564",
		)
	}

	#[test]
	fn collect_invest() {
		test_encode_decode_identity(
			ConnectorMessage::CollectInvest {
				pool_id: 1,
				tranche_id: tranche_id_from_hex(TRANCHE_HEX),
				address: address32_from_hex(ADDRESS_32_HEX),
			},
			"0f0000000000000001811acd5b3f17c06841c7e41e9e04cb1b4564564564564564564564564564564564564564564564564564564564564564",
		)
	}

	#[test]
	fn collect_for_invest() {
		test_encode_decode_identity(
			ConnectorMessage::CollectForInvest {
				pool_id: 1,
				tranche_id: tranche_id_from_hex(TRANCHE_HEX),
				caller: vec_to_fixed_array(address20_from_hex(ADDRESS_20_HEX).to_vec()),
				user: address32_from_hex(ADDRESS_32_HEX),
			},
			"100000000000000001811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312310000000000000000000000004564564564564564564564564564564564564564564564564564564564564564",
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

		let decoded: Message<Domain, PoolId, TrancheId, Balance, Rate> =
			Message::deserialize(&mut hex::decode(expected_hex).expect("").as_slice()).expect("");
		assert_eq!(msg, decoded);
	}

	fn tranche_id_from_hex(hex: &str) -> TrancheId {
		<[u8; 16]>::from_hex(hex).expect("Should be valid tranche id")
	}

	fn address20_from_hex(hex: &str) -> [u8; 20] {
		<[u8; 20]>::from_hex(hex).expect("Should be valid 20 bytes")
	}

	fn address32_from_hex(hex: &str) -> [u8; 32] {
		<[u8; 32]>::from_hex(hex).expect("Should be valid 32 bytes")
	}
}
