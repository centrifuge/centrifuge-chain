use cfg_primitives::Moment;
use sp_std::{vec, vec::Vec};

use crate::*;

/// Address type
/// Note: It can be used to represent any address type with a length <= 32 bytes;
/// For example, it can represent an Ethereum address (20-bytes long) by padding it with 12 zeros.
type Address = [u8; 32];

/// The fixed size for the array representing a tranche token name
pub const TOKEN_NAME_SIZE: usize = 128;

// The fixed size for the array representing a tranche token symbol
pub const TOKEN_SYMBOL_SIZE: usize = 32;

#[derive(Decode, Clone, PartialEq, Eq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Message<Domain, PoolId, TrancheId, Balance, Rate>
where
	Domain: ConnectorEncode,
	PoolId: Encode,
	TrancheId: Encode,
	Balance: Encode,
	Rate: Encode,
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

impl<Domain: ConnectorEncode, PoolId: Encode, TrancheId: Encode, Balance: Encode, Rate: Encode>
	Message<Domain, PoolId, TrancheId, Balance, Rate>
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

impl<Domain: ConnectorEncode, PoolId: Encode, TrancheId: Encode, Balance: Encode, Rate: Encode>
	Encode for Message<Domain, PoolId, TrancheId, Balance, Rate>
{
	fn encode(&self) -> Vec<u8> {
		match self {
			Message::Invalid => vec![self.call_type()],
			Message::AddPool { pool_id } => {
				let mut message: Vec<u8> = vec![];
				message.push(self.call_type());
				message.append(&mut to_be(pool_id));

				message
			}
			Message::AddTranche {
				pool_id,
				tranche_id,
				token_name,
				token_symbol,
				price,
			} => {
				let mut message: Vec<u8> = vec![];
				message.push(self.call_type());
				message.append(&mut to_be(pool_id));
				message.append(&mut tranche_id.encode());
				message.append(&mut token_name.encode());
				message.append(&mut token_symbol.encode());
				message.append(&mut to_be(price));

				message
			}
			Message::UpdateTokenPrice {
				pool_id,
				tranche_id,
				price,
			} => {
				let mut message: Vec<u8> = vec![];
				message.push(self.call_type());
				message.append(&mut to_be(pool_id));
				message.append(&mut tranche_id.encode());
				message.append(&mut to_be(price));

				message
			}
			Message::UpdateMember {
				pool_id,
				tranche_id,
				address,
				valid_until,
			} => {
				let mut message: Vec<u8> = vec![];
				message.push(self.call_type());
				message.append(&mut to_be(pool_id));
				message.append(&mut tranche_id.encode());
				message.append(&mut address.encode());
				message.append(&mut valid_until.to_be_bytes().to_vec());

				message
			}
			Message::Transfer {
				pool_id,
				tranche_id,
				domain,
				address,
				amount,
			} => {
				let mut message: Vec<u8> = vec![];
				message.push(self.call_type());
				message.append(&mut to_be(pool_id));
				message.append(&mut tranche_id.encode());
				message.append(&mut domain.connector_encode());
				message.append(&mut address.encode());
				message.append(&mut to_be(amount));

				message
			}
		}
	}
}

// Encode a value in its big-endian representation. We use this for number types to make
// sure they are encoded the way they are expected to be decoded on the Solidity side.
fn to_be(x: impl Encode) -> Vec<u8> {
	let mut output = x.encode();
	output.reverse();
	output
}

#[cfg(test)]
mod tests {
	use cfg_types::fixed_point::Rate;
	use codec::Encode;
	use hex::FromHex;
	use sp_runtime::traits::One;

	use crate::Message;

	type PoolId = u64;
	type TrancheId = [u8; 16];
	type Balance = cfg_primitives::Balance;

	pub mod encode {
		use cfg_utils::vec_to_fixed_array;

		use super::*;
		use crate::{Domain, DomainAddress};

		#[test]
		fn invalid() {
			let msg = Message::<Domain, PoolId, TrancheId, Balance, Rate>::Invalid;
			assert_eq!(msg.encode(), vec![msg.call_type()]);
			assert_eq!(msg.encode(), vec![0]);
		}

		#[test]
		fn encoding_domain() {
			use crate::ConnectorEncode;

			// The Centrifuge substrate chain
			assert_eq!(
				hex::encode(Domain::Centrifuge.connector_encode()),
				"000000000000000000"
			);
			// Ethereum MainNet
			assert_eq!(
				hex::encode(Domain::EVM(1).connector_encode()),
				"010000000000000001"
			);
			// Moonbeam EVM chain
			assert_eq!(
				hex::encode(Domain::EVM(1284).connector_encode()),
				"010000000000000504"
			);
			// Avalanche Chain
			assert_eq!(
				hex::encode(Domain::EVM(43114).connector_encode()),
				"01000000000000a86a"
			);
		}

		#[test]
		fn add_pool_zero() {
			let msg = Message::<Domain, PoolId, TrancheId, Balance, Rate>::AddPool { pool_id: 0 };
			let encoded = msg.encode();

			let expected_hex = "010000000000000000";
			let expected = <[u8; 9]>::from_hex(expected_hex).expect("Decoding failed");
			assert_eq!(encoded, expected);
		}

		#[test]
		fn add_pool_long() {
			let msg =
				Message::<Domain, PoolId, TrancheId, Balance, Rate>::AddPool { pool_id: 12378532 };
			let encoded = msg.encode();

			let expected_hex = "010000000000bce1a4";
			let expected = <[u8; 9]>::from_hex(expected_hex).expect("Decoding failed");
			assert_eq!(encoded, expected);
		}

		#[test]
		fn add_tranche() {
			let msg = Message::<Domain, PoolId, TrancheId, Balance, Rate>::AddTranche {
				pool_id: 12378532,
				tranche_id: <[u8; 16]>::from_hex("811acd5b3f17c06841c7e41e9e04cb1b").expect(""),
				token_name: vec_to_fixed_array("Some Name".to_string().into_bytes()),
				token_symbol: vec_to_fixed_array("SYMBOL".to_string().into_bytes()),
				price: Rate::one(),
			};
			let encoded_bytes = msg.encode();

			// We encode the encoded bytes as hex to verify it's what we expect
			let encoded_hex = hex::encode(encoded_bytes.clone());
			let expected_hex = "020000000000bce1a4811acd5b3f17c06841c7e41e9e04cb1b536f6d65204e616d65000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000053594d424f4c000000000000000000000000000000000000000000000000000000000000033b2e3c9fd0803ce8000000";
			assert_eq!(expected_hex, encoded_hex);

			// Now decode the bytes encoded as hex back to bytes and verify it's the same as
			// the original `encoded_bytes`
			let hex_as_bytes = hex::decode(encoded_hex).expect("Should go vec -> hex -> vec");
			assert_eq!(hex_as_bytes, encoded_bytes);
		}

		#[test]
		fn update_token_price() {
			let msg = Message::<Domain, PoolId, TrancheId, Balance, Rate>::UpdateTokenPrice {
				pool_id: 1,
				tranche_id: <[u8; 16]>::from_hex("811acd5b3f17c06841c7e41e9e04cb1b").expect(""),
				price: Rate::one(),
			};
			let encoded = msg.encode();

			let input = "030000000000000001811acd5b3f17c06841c7e41e9e04cb1b00000000033b2e3c9fd0803ce8000000";
			let expected = <[u8; 41]>::from_hex(input).expect("Decoding failed");
			assert_eq!(encoded, expected);
		}

		#[test]
		fn update_member() {
			let msg = Message::<Domain, PoolId, TrancheId, Balance, Rate>::UpdateMember {
				pool_id: 1,
				tranche_id: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
				address: [1; 32],
				valid_until: 100,
			};
			let encoded = msg.encode();

			let input = "0400000000000000010000000000000000000000000000000101010101010101010101010101010101010101010101010101010101010101010000000000000064";
			let expected = <[u8; 65]>::from_hex(input).expect("Decoding failed");
			assert_eq!(encoded, expected);
		}

		#[test]
		fn update_member_that_failed() {
			let msg = Message::<Domain, PoolId, TrancheId, Balance, Rate>::UpdateMember {
				pool_id: 2,
				tranche_id: <[u8; 16]>::from_hex("811acd5b3f17c06841c7e41e9e04cb1b").expect(""),
				address: <[u8; 32]>::from_hex(
					"1231231231231231231231231231231231231231231231231231231231231231",
				)
				.expect(""),
				valid_until: 1706260138,
			};
			let encoded = msg.encode();

			let input = "040000000000000002811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312312312312312312312312312310000000065b376aa";
			let expected = <[u8; 65]>::from_hex(input).expect("Decoding failed");
			assert_eq!(hex::encode(encoded), hex::encode(expected));
		}

		#[test]
		fn transfer_to_moonbeam() {
			let domain_address = DomainAddress::EVM(
				1284,
				<[u8; 20]>::from_hex("1231231231231231231231231231231231231231").expect(""),
			);

			let msg = Message::<Domain, PoolId, TrancheId, Balance, Rate>::Transfer {
				pool_id: 1,
				tranche_id: tranche_id_from_hex("811acd5b3f17c06841c7e41e9e04cb1b"),
				domain: domain_address.clone().into(),
				address: domain_address.address(),
				amount: 1000000000000000000000000000,
			};
			let encoded = msg.encode();
			let expected = "050000000000000001811acd5b3f17c06841c7e41e9e04cb1b010000000000000504123123123123123123123123123123123123123100000000000000000000000000000000033b2e3c9fd0803ce8000000";

			assert_eq!(hex::encode(encoded), expected);
		}

		#[test]
		fn transfer_to_centrifuge() {
			let address =
				<[u8; 20]>::from_hex("1231231231231231231231231231231231231231").expect("");

			let msg = Message::<Domain, PoolId, TrancheId, Balance, Rate>::Transfer {
				pool_id: 1,
				tranche_id: tranche_id_from_hex("811acd5b3f17c06841c7e41e9e04cb1b"),
				domain: Domain::Centrifuge,
				address: vec_to_fixed_array(address.to_vec()),
				amount: 1000000000000000000000000000,
			};
			let encoded = msg.encode();

			let expected = "050000000000000001811acd5b3f17c06841c7e41e9e04cb1b000000000000000000123123123123123123123123123123123123123100000000000000000000000000000000033b2e3c9fd0803ce8000000";

			// solidity is 172 chars, 86 bytes
			assert_eq!(hex::encode(encoded), expected);
		}
	}

	fn tranche_id_from_hex(hex: &str) -> TrancheId {
		<[u8; 16]>::from_hex(hex).expect("Should be valid tranche id")
	}
}
