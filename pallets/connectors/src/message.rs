use frame_support::BoundedVec;

use crate::*;

#[derive(Decode, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Message<PoolId, TrancheId, Rate>
where
	PoolId: Encode + Decode,
	TrancheId: Encode + Decode,
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
	}, // More to come...
}

impl<PoolId: Encode + Decode, TrancheId: Encode + Decode, Rate: Encode + Decode>
	Message<PoolId, TrancheId, Rate>
{
	fn call_type(&self) -> u8 {
		match self {
			Self::Invalid => 0,
			Self::AddPool { .. } => 1,
			Self::AddTranche { .. } => 2,
			Self::UpdateTokenPrice { .. } => 3,
		}
	}
}

impl<PoolId: Encode + Decode, TrancheId: Encode + Decode, Rate: Encode + Decode> Encode
	for Message<PoolId, TrancheId, Rate>
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
		}
	}
}

#[cfg(test)]
mod tests {
	use crate::Message;
	use codec::Encode;
	use hex::FromHex;
	use sp_runtime::traits::One;

	type PoolId = u64;
	type TrancheId = [u8; 16];
	type Rate = runtime_common::Rate;

	pub mod encode {
		use super::*;

		#[test]
		fn invalid() {
			let msg = Message::<PoolId, TrancheId, Rate>::Invalid;
			assert_eq!(msg.encode(), vec![msg.call_type()]);
			assert_eq!(msg.encode(), vec![0]);
		}

		#[test]
		fn add_pool_0() {
			let msg = Message::<PoolId, TrancheId, Rate>::AddPool { pool_id: 0 };
			let encoded = msg.encode();

			let input = "010000000000000000";
			let expected = <[u8; 9]>::from_hex(input).expect("Decoding failed");
			assert_eq!(encoded, expected);
		}

		#[test]
		fn add_pool_long() {
			let msg = Message::<PoolId, TrancheId, Rate>::AddPool { pool_id: 12378532 };
			let encoded = msg.encode();

			let input = "010000000000bce1a4";
			let expected = <[u8; 9]>::from_hex(input).expect("Decoding failed");
			assert_eq!(encoded, expected);
		}

		#[test]
		fn add_tranche() {
			let msg = Message::<PoolId, TrancheId, Rate>::AddTranche {
				pool_id: 1,
				tranche_id: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
				token_name: [0; 32],
				token_symbol: [0; 32],
			};
			let encoded = msg.encode();

			let input = "0200000000000000010000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
			let expected = <[u8; 89]>::from_hex(input).expect("Decoding failed");
			assert_eq!(encoded, expected);
		}

		#[test]
		fn update_token_price() {
			let msg = Message::<PoolId, TrancheId, Rate>::UpdateTokenPrice {
				pool_id: 1,
				tranche_id: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
				price: Rate::one(),
			};
			let encoded = msg.encode();

			let input = "0200000000000000010000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
			let expected = <[u8; 89]>::from_hex(input).expect("Decoding failed");
			assert_eq!(encoded, expected);
		}
	}
}
