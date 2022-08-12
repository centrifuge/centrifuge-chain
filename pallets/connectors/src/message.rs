use common_traits::Moment;
use frame_support::BoundedVec;
use sp_std::vec;
use sp_std::vec::Vec;

use crate::*;

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
		token_name: [u8; 32],
		token_symbol: [u8; 32],
	},
	UpdateTokenPrice {
		pool_id: PoolId,
		tranche_id: TrancheId,
		price: Rate,
	},
	UpdateMember {
		pool_id: PoolId,
		tranche_id: TrancheId,
		address: [u8; 32],
		valid_until: Moment,
	},
	Transfer {
		pool_id: PoolId,
		tranche_id: TrancheId,
		domain: Domain,
		destination: [u8; 32],
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
	use crate::Message;
	use codec::{Decode, Encode};
	use hex::FromHex;
	use sp_runtime::traits::One;

	type PoolId = u64;
	type TrancheId = [u8; 16];
	type Balance = runtime_common::Balance;
	type Rate = runtime_common::Rate;

	const CURRENCY: Balance = 1_000_000_000_000_000_000;

	pub mod encode {
		use crate::Domain;

		use super::*;

		use serde::{Deserialize, Serialize};

		#[test_fuzz::test_fuzz]
		fn target(msg: Message::<PoolId, TrancheId>) {
			let encoded = msg.encode();
			let expected = <[u8; 9]>::from_hex(msg.encode()).expect("Decoding failed");
			println!("{:?}: {:?}",encoded, expected);
			assert_eq!(encoded, expected);
		}

		#[test]
		fn invalid() {
			let msg = Message::<Domain, PoolId, TrancheId, Balance, Rate>::Invalid;
			assert_eq!(msg.encode(), vec![msg.call_type()]);
			assert_eq!(msg.encode(), vec![0]);
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
				pool_id: 1,
				tranche_id: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
				token_name: [0; 32],
				token_symbol: [0; 32],
			};
			let encoded = msg.encode();

			let expected_hex = "0200000000000000010000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
			let expected = <[u8; 89]>::from_hex(expected_hex).expect("Decoding failed");
			assert_eq!(encoded, expected);
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
				domain: Domain::Avalanche,
				destination: [1; 32],
				amount: 100 * CURRENCY,
			};
			let encoded = msg.encode();
			println!("{}", hex::encode(encoded.clone()));

			let input = "0400000000000000010000000000000000000000000000000101010101010101010101010101010101010101010101010101010101010101016400000000000000";
			let expected = <[u8; 65]>::from_hex(input).expect("Decoding failed");
			assert_eq!(encoded, expected);
		}
	}
}
