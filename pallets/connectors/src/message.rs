use cfg_primitives::Moment;
use cfg_utils::{decode, decode_be_bytes, encode_be};
use codec::{Decode, Encode, EncodeLike, Input};
use scale_info::TypeInfo;
use sp_std::{vec, vec::Vec};

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
#[derive(Clone, PartialEq, Eq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Message<Domain, PoolId, TrancheId, Balance, Rate>
where
	Domain: Encode + Decode,
	PoolId: Encode + Decode,
	TrancheId: Encode + Decode,
	Balance: Encode + Decode,
	Rate: Encode + Decode,
{
	Invalid,
	AddPool {
		pool_id: PoolId,
	},
	AddTranche {
		pool_id: PoolId,
		tranche_id: TrancheId,
		token_name: [u8; TOKEN_NAME_SIZE],
		token_symbol: [u8; TOKEN_SYMBOL_SIZE],
		price: Rate,
	},
	UpdateTokenPrice {
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
	Transfer {
		pool_id: PoolId,
		tranche_id: TrancheId,
		domain: Domain,
		address: Address,
		amount: Balance,
	},
}

impl<
		Domain: Encode + Decode,
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
			Self::Invalid => 0,
			Self::AddPool { .. } => 1,
			Self::AddTranche { .. } => 2,
			Self::UpdateTokenPrice { .. } => 3,
			Self::UpdateMember { .. } => 4,
			Self::Transfer { .. } => 5,
		}
	}
}

impl<
		Domain: Encode + Decode,
		PoolId: Encode + Decode,
		TrancheId: Encode + Decode,
		Balance: Encode + Decode,
		Rate: Encode + Decode,
	> EncodeLike for Message<Domain, PoolId, TrancheId, Balance, Rate>
{
}

impl<
		Domain: Encode + Decode,
		PoolId: Encode + Decode,
		TrancheId: Encode + Decode,
		Balance: Encode + Decode,
		Rate: Encode + Decode,
	> Decode for Message<Domain, PoolId, TrancheId, Balance, Rate>
{
	fn decode<I: Input>(input: &mut I) -> Result<Self, codec::Error> {
		let call_type = input.read_byte()?;

		match call_type {
			0 => Ok(Self::Invalid),
			1 => Ok(Self::AddPool {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
			}),
			2 => Ok(Self::AddTranche {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				token_name: decode::<TOKEN_NAME_SIZE, _, _>(input)?,
				token_symbol: decode::<TOKEN_SYMBOL_SIZE, _, _>(input)?,
				price: decode_be_bytes::<16, _, _>(input)?,
			}),
			3 => Ok(Self::UpdateTokenPrice {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				price: decode_be_bytes::<16, _, _>(input)?,
			}),
			4 => Ok(Self::UpdateMember {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				address: decode::<32, _, _>(input)?,
				valid_until: decode_be_bytes::<8, _, _>(input)?,
			}),
			5 => Ok(Self::Transfer {
				pool_id: decode_be_bytes::<8, _, _>(input)?,
				tranche_id: decode::<16, _, _>(input)?,
				domain: decode::<9, _, _>(input)?,
				address: decode::<32, _, _>(input)?,
				amount: decode_be_bytes::<16, _, _>(input)?,
			}),
			_ => Err(codec::Error::from(
				"Unsupported decoding for this Message variant",
			)),
		}
	}
}

impl<
		Domain: Encode + Decode,
		PoolId: Encode + Decode,
		TrancheId: Encode + Decode,
		Balance: Encode + Decode,
		Rate: Encode + Decode,
	> Encode for Message<Domain, PoolId, TrancheId, Balance, Rate>
{
	fn encode(&self) -> Vec<u8> {
		match self {
			Message::Invalid => vec![self.call_type()],
			Message::AddPool { pool_id } => {
				encoded_message(self.call_type(), vec![encode_be(pool_id)])
			}
			Message::AddTranche {
				pool_id,
				tranche_id,
				token_name,
				token_symbol,
				price,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					token_name.encode(),
					token_symbol.encode(),
					encode_be(price),
				],
			),
			Message::UpdateTokenPrice {
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
					address.encode(),
					valid_until.to_be_bytes().to_vec(),
				],
			),
			Message::Transfer {
				pool_id,
				tranche_id,
				domain,
				address,
				amount,
			} => encoded_message(
				self.call_type(),
				vec![
					encode_be(pool_id),
					tranche_id.encode(),
					domain.encode(),
					address.encode(),
					encode_be(amount),
				],
			),
		}
	}
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
	use codec::{Decode, Encode};
	use hex::FromHex;
	use sp_runtime::traits::One;

	use super::*;
	use crate::{Domain, DomainAddress};

	pub type ConnectorMessage = Message<Domain, PoolId, TrancheId, Balance, Rate>;

	#[test]
	fn invalid() {
		let msg = ConnectorMessage::Invalid;
		assert_eq!(msg.encode(), vec![msg.call_type()]);
		assert_eq!(msg.encode(), vec![0]);
	}

	#[test]
	fn encoding_domain() {
		use crate::Encode;

		// The Centrifuge substrate chain
		assert_eq!(
			hex::encode(Domain::Centrifuge.encode()),
			"000000000000000000"
		);
		// Ethereum MainNet
		assert_eq!(hex::encode(Domain::EVM(1).encode()), "010000000000000001");
		// Moonbeam EVM chain
		assert_eq!(
			hex::encode(Domain::EVM(1284).encode()),
			"010000000000000504"
		);
		// Avalanche Chain
		assert_eq!(
			hex::encode(Domain::EVM(43114).encode()),
			"01000000000000a86a"
		);
	}

	#[test]
	fn add_pool_zero() {
		test_encode_decode_identity(
			ConnectorMessage::AddPool { pool_id: 0 },
			"010000000000000000",
		)
	}

	#[test]
	fn add_pool_long() {
		test_encode_decode_identity(
			ConnectorMessage::AddPool { pool_id: 12378532 },
			"010000000000bce1a4",
		)
	}

	#[test]
	fn add_tranche() {
		test_encode_decode_identity(
				ConnectorMessage::AddTranche {
					pool_id: 12378532,
					tranche_id: <[u8; 16]>::from_hex("811acd5b3f17c06841c7e41e9e04cb1b").expect(""),
					token_name: vec_to_fixed_array("Some Name".to_string().into_bytes()),
					token_symbol: vec_to_fixed_array("SYMBOL".to_string().into_bytes()),
					price: Rate::one(),
				},
				"020000000000bce1a4811acd5b3f17c06841c7e41e9e04cb1b536f6d65204e616d65000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000053594d424f4c000000000000000000000000000000000000000000000000000000000000033b2e3c9fd0803ce8000000"
			)
	}

	#[test]
	fn update_token_price() {
		test_encode_decode_identity(
			ConnectorMessage::UpdateTokenPrice {
				pool_id: 1,
				tranche_id: <[u8; 16]>::from_hex("811acd5b3f17c06841c7e41e9e04cb1b").expect(""),
				price: Rate::one(),
			},
			"030000000000000001811acd5b3f17c06841c7e41e9e04cb1b00000000033b2e3c9fd0803ce8000000",
		)
	}

	#[test]
	fn update_member() {
		test_encode_decode_identity(
				ConnectorMessage::UpdateMember {
					pool_id: 2,
					tranche_id: <[u8; 16]>::from_hex("811acd5b3f17c06841c7e41e9e04cb1b").expect(""),
					address: <[u8; 32]>::from_hex(
						"1231231231231231231231231231231231231231231231231231231231231231",
					)
						.expect(""),
					valid_until: 1706260138,
				},
				"040000000000000002811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312312312312312312312312312310000000065b376aa"
			)
	}

	#[test]
	fn transfer_to_moonbeam() {
		let domain_address = DomainAddress::EVM(
			1284,
			<[u8; 20]>::from_hex("1231231231231231231231231231231231231231").expect(""),
		);

		test_encode_decode_identity(
				ConnectorMessage::Transfer {
					pool_id: 1,
					tranche_id: tranche_id_from_hex("811acd5b3f17c06841c7e41e9e04cb1b"),
					domain: domain_address.clone().into(),
					address: domain_address.address(),
					amount: 1000000000000000000000000000,
				},
				"050000000000000001811acd5b3f17c06841c7e41e9e04cb1b010000000000000504123123123123123123123123123123123123123100000000000000000000000000000000033b2e3c9fd0803ce8000000"
			);
	}

	#[test]
	fn transfer_to_centrifuge() {
		test_encode_decode_identity(
				ConnectorMessage::Transfer {
					pool_id: 1,
					tranche_id: tranche_id_from_hex("811acd5b3f17c06841c7e41e9e04cb1b"),
					domain: Domain::Centrifuge,
					address: vec_to_fixed_array(<[u8; 20]>::from_hex("1231231231231231231231231231231231231231").expect("").to_vec()),
					amount: 1000000000000000000000000000,
				},
				"050000000000000001811acd5b3f17c06841c7e41e9e04cb1b000000000000000000123123123123123123123123123123123123123100000000000000000000000000000000033b2e3c9fd0803ce8000000"
			)
	}

	/// Verify the identity property of decode . encode on a Message value and
	/// that it in fact encodes to and can be decoded from a given hex string.
	fn test_encode_decode_identity(
		msg: Message<Domain, PoolId, TrancheId, Balance, Rate>,
		expected_hex: &str,
	) {
		let encoded = msg.encode();
		assert_eq!(hex::encode(encoded.clone()), expected_hex);

		let decoded: Message<Domain, PoolId, TrancheId, Balance, Rate> =
			Message::decode(&mut hex::decode(expected_hex).expect("").as_slice()).expect("");
		assert_eq!(msg, decoded);
	}

	fn tranche_id_from_hex(hex: &str) -> TrancheId {
		<[u8; 16]>::from_hex(hex).expect("Should be valid tranche id")
	}
}
