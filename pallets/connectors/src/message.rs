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

#[derive(Decode, Clone, PartialEq, TypeInfo)]
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
		destination: Address,
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
	> Encode for Message<Domain, PoolId, TrancheId, Balance, Rate>
{
	fn encode(&self) -> Vec<u8> {
		match self {
			Message::Invalid => vec![self.call_type()],
			Message::AddPool { pool_id } => {
				let mut message: Vec<u8> = vec![];
				message.push(self.call_type());

				let mut encoded_pool_id = pool_id.encode();
				encoded_pool_id.reverse();
				message.append(&mut encoded_pool_id);

				message
			}
			Message::AddTranche {
				pool_id,
				tranche_id,
				token_name,
				token_symbol,
			} => {
				let mut message: Vec<u8> = vec![];
				message.push(self.call_type());

				let mut encoded_pool_id = pool_id.encode();
				encoded_pool_id.reverse();
				message.append(&mut encoded_pool_id);

				message.append(&mut tranche_id.encode());
				message.append(&mut token_name.encode());
				message.append(&mut token_symbol.encode());

				message
			}
			Message::UpdateTokenPrice {
				pool_id,
				tranche_id,
				price,
			} => {
				let mut message: Vec<u8> = vec![];
				message.push(self.call_type());

				let mut encoded_pool_id = pool_id.encode();
				encoded_pool_id.reverse();
				message.append(&mut encoded_pool_id);

				message.append(&mut tranche_id.encode());
				message.append(&mut price.encode());

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

				let mut encoded_pool_id = pool_id.encode();
				encoded_pool_id.reverse();
				message.append(&mut encoded_pool_id);

				message.append(&mut tranche_id.encode());
				message.append(&mut address.encode());
				message.append(&mut valid_until.encode());

				message
			}
			Message::Transfer {
				pool_id,
				tranche_id,
				domain,
				destination,
				amount,
			} => {
				let mut message: Vec<u8> = vec![];
				message.push(self.call_type());

				let mut encoded_pool_id = pool_id.encode();
				encoded_pool_id.reverse();
				message.append(&mut encoded_pool_id);

				message.append(&mut tranche_id.encode());
				message.append(&mut domain.encode());
				message.append(&mut destination.encode());
				message.append(&mut amount.encode());

				message
			}
		}
	}
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

	const CURRENCY: Balance = 1_000_000_000_000_000_000;

	pub mod encode {
		use super::*;
		use crate::{Domain, ParachainId};

		#[test]
		fn invalid() {
			let msg = Message::<Domain, PoolId, TrancheId, Balance, Rate>::Invalid;
			assert_eq!(msg.encode(), vec![msg.call_type()]);
			assert_eq!(msg.encode(), vec![0]);
		}

		#[test]
		fn encoding_domain_evm_ethereum_mainnet() {
			let domain = Domain::EVM(1);

			let expected_hex = "000100000000000000";
			let expected = <[u8; 9]>::from_hex(expected_hex).expect("Decoding failed");
			assert_eq!(domain.encode(), expected);
		}

		#[test]
		fn encoding_domain_evm_avalanche() {
			let domain = Domain::EVM(43114);

			let expected_hex = "006aa8000000000000";
			let expected = <[u8; 9]>::from_hex(expected_hex).expect("Decoding failed");
			assert_eq!(domain.encode(), expected);
		}

		#[test]
		fn encoding_domain_parachain() {
			let domain = Domain::Parachain(ParachainId::Moonbeam);

			let expected_hex = "0100";
			let expected = <[u8; 2]>::from_hex(expected_hex).expect("Decoding failed");
			assert_eq!(domain.encode(), expected);
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
				tranche_id: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
				token_name: [5; 128],
				token_symbol: [6; 32],
			};
			let encoded_bytes = msg.encode();

			// We encode the encoded bytes as hex to verify it's what we expect
			let encoded_hex = hex::encode(encoded_bytes.clone());
			let expected_hex = "020000000000bce1a40000000000000000000000000000000105050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050505050606060606060606060606060606060606060606060606060606060606060606";
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
				tranche_id: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
				price: Rate::one(),
			};
			let encoded = msg.encode();

			let input = "03000000000000000100000000000000000000000000000001000000e83c80d09f3c2e3b0300000000";
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

			let input = "0400000000000000010000000000000000000000000000000101010101010101010101010101010101010101010101010101010101010101016400000000000000";
			let expected = <[u8; 65]>::from_hex(input).expect("Decoding failed");
			assert_eq!(encoded, expected);
		}

		#[test]
		fn transfer() {
			let msg = Message::<Domain, PoolId, TrancheId, Balance, Rate>::Transfer {
				pool_id: 1,
				tranche_id: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
				domain: Domain::Parachain(ParachainId::Moonbeam),
				destination: [1; 32],
				amount: 100 * CURRENCY,
			};
			let encoded = msg.encode();

			let input = "0500000000000000010000000000000000000000000000000101000101010101010101010101010101010101010101010101010101010101010101000010632d5ec76b0500000000000000";

			let expected = <[u8; 75]>::from_hex(input).expect("Decoding failed");
			assert_eq!(encoded, expected);
		}
	}
}
