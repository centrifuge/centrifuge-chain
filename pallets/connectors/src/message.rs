use crate::*;

#[derive(Decode, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Message<PoolId>
where
	PoolId: Encode + Decode,
{
	Invalid,
	AddPool { pool_id: PoolId }, // More to come...
}

impl<PoolId: Encode + Decode> Message<PoolId> {
	fn call_type(&self) -> u8 {
		match self {
			Self::Invalid => 0,
			Self::AddPool { .. } => 1,
		}
	}
}

impl<PoolId: Encode + Decode> Encode for Message<PoolId> {
	fn encode(&self) -> Vec<u8> {
		match self {
			Message::Invalid => vec![self.call_type()],
			Message::AddPool { pool_id } => {
				let mut message: Vec<u8> = vec![];
				message.push(self.call_type());
				//todo(nuno): &mut pool_id.as_bytes().to_vec());
				// to do this, we need to need a stricter PoolId bound to be able to convert it to byte array
				let mut encoded_pool_id = pool_id.encode();
				encoded_pool_id.reverse();
				message.append(&mut encoded_pool_id);
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

	type PoolId = u64;

	pub mod encode {
		use super::*;

		#[test]
		fn invalid() {
			let msg = Message::<PoolId>::Invalid;
			assert_eq!(msg.encode(), vec![msg.call_type()]);
			assert_eq!(msg.encode(), vec![0]);
		}

		#[test]
		fn add_pool_0() {
			let msg = Message::<PoolId>::AddPool { pool_id: 0 };
			let encoded = msg.encode();

			let input = "010000000000000000";
			let expected = <[u8; 9]>::from_hex(input).expect("Decoding failed");
			assert_eq!(encoded, expected);
		}

		#[test]
		fn add_pool_long() {
			let msg = Message::<PoolId>::AddPool { pool_id: 12378532 };
			let encoded = msg.encode();

			let input = "010000000000bce1a4";
			let expected = <[u8; 9]>::from_hex(input).expect("Decoding failed");
			assert_eq!(encoded, expected);
		}
	}
}
