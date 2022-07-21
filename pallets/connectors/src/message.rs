use crate::*;

#[derive(Decode, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Message<PoolId> {
	Invalid,
	AddPool { pool_id: PoolId }, // More to come...
}

impl<PoolId> Encode for Message<PoolId> {
	fn encode(&self) -> Vec<u8> {
		match self {
			Message::Invalid => vec![0u8],
			Message::AddPool { pool_id: _ } => {
				let mut message: Vec<u8> = vec![0u8];
				message.append(&mut vec![1, 2, 3]); //todo(nuno): &mut pool_id.as_bytes().to_vec());
				message
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use crate::Message;
	use codec::Encode;

	type PoolId = u64;

	pub mod encode {
		use super::*;

		#[test]
		fn invalid() {
			let encoded = Message::<PoolId>::Invalid.encode();
			assert_eq!(encoded, vec![0])
		}

		#[test]
		fn add_pool() {
			let encoded = Message::<PoolId>::AddPool { pool_id: 42 }.encode();
			assert_eq!(encoded, vec![0, 1, 2, 3])
		}
	}
}
