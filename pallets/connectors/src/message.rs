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
			5 => Ok(Self::Transfer {
				pool_id: decode_be_bytes::<PoolId, 8, _>(input)?,
				tranche_id: decode::<TrancheId, 16, _>(input)?,
				domain: decode::<Domain, 9, _>(input)?,
				address: decode::<Address, 32, _>(input)?,
				amount: decode_be_bytes::<Balance, 16, _>(input)?,
			}),
			_ => Err(codec::Error::from(
				"Unsupported decoding for this Message variant",
			)),
		}
	}
}

/// Decode a type O by reading S bytes from I. Those bytes are expected to be encoded
/// as big-endian and thus needs reversing to little-endian before decoding to O.
fn decode_be_bytes<O: Decode, const S: usize, I: Input>(input: &mut I) -> Result<O, codec::Error> {
	let mut bytes = [0; S];
	input.read(&mut bytes[..])?;
	bytes.reverse();

	O::decode(&mut bytes.as_slice())
}

/// Decode a type 0 by reading S bytes from I.
fn decode<O: Decode, const S: usize, I: Input>(input: &mut I) -> Result<O, codec::Error> {
	let mut bytes = [0; S];
	input.read(&mut bytes[..])?;

	O::decode(&mut bytes.as_slice())
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
				message.append(&mut domain.encode());
				message.append(&mut address.encode());
				message.append(&mut to_be(amount));

				message
			}
		}
	}
}

/// Encode a value in its big-endian representation. We use this for number types to make
/// sure they are encoded the way they are expected to be decoded on the Solidity side.
fn to_be(x: impl Encode) -> Vec<u8> {
	let mut output = x.encode();
	output.reverse();
	output
}

#[cfg(test)]
mod tests {
	use cfg_types::fixed_point::Rate;
	use codec::{Decode, Encode};
	use hex::FromHex;
	use sp_runtime::traits::One;

	use crate::{Domain, Message};

	type PoolId = u64;
	type TrancheId = [u8; 16];
	type Balance = cfg_primitives::Balance;

	pub mod decode {
		use super::*;

		/// Test that decode . encode results in the original value
		#[test]
		fn transfer() {
			let msg = Message::Transfer {
				pool_id: 1,
				tranche_id: tranche_id_from_hex("811acd5b3f17c06841c7e41e9e04cb1b"),
				domain: Domain::Centrifuge,
				address: <[u8; 32]>::from_hex(
					"1231231231231231231231231231231231231231231231231231231231231231",
				)
				.expect(""),
				amount: 1000000000000000000000000000,
			};
			let encoded = msg.encode();
			let decoded: Message<Domain, PoolId, TrancheId, Balance, Rate> =
				Message::decode(&mut encoded.as_slice()).expect("");

			assert_eq!(msg, decoded);
		}
	}

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
			let msg = Message::<Domain, PoolId, TrancheId, Balance, Rate>::AddPool { pool_id: 0 };
			let encoded = msg.encode();

			let expected = "010000000000000000";
			assert_eq!(hex::encode(encoded), expected);
		}

		#[test]
		fn add_pool_long() {
			let msg =
				Message::<Domain, PoolId, TrancheId, Balance, Rate>::AddPool { pool_id: 12378532 };
			let encoded = msg.encode();

			let expected = "010000000000bce1a4";
			assert_eq!(hex::encode(encoded), expected);
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
			let encoded = msg.encode();

			let expected = "020000000000bce1a4811acd5b3f17c06841c7e41e9e04cb1b536f6d65204e616d65000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000053594d424f4c000000000000000000000000000000000000000000000000000000000000033b2e3c9fd0803ce8000000";
			assert_eq!(hex::encode(encoded), expected);
		}

		#[test]
		fn update_token_price() {
			let msg = Message::<Domain, PoolId, TrancheId, Balance, Rate>::UpdateTokenPrice {
				pool_id: 1,
				tranche_id: <[u8; 16]>::from_hex("811acd5b3f17c06841c7e41e9e04cb1b").expect(""),
				price: Rate::one(),
			};
			let encoded = msg.encode();

			let expected = "030000000000000001811acd5b3f17c06841c7e41e9e04cb1b00000000033b2e3c9fd0803ce8000000";
			assert_eq!(hex::encode(encoded), expected);
		}

		#[test]
		fn update_member() {
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

			let expected = "040000000000000002811acd5b3f17c06841c7e41e9e04cb1b12312312312312312312312312312312312312312312312312312312312312310000000065b376aa";
			assert_eq!(hex::encode(encoded), expected);
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

			assert_eq!(hex::encode(encoded), expected);
		}
	}

	fn tranche_id_from_hex(hex: &str) -> TrancheId {
		<[u8; 16]>::from_hex(hex).expect("Should be valid tranche id")
	}
}
