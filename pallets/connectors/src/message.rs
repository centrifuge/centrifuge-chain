use crate::*;

#[derive(Decode, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Message<PoolId> {
	AddPool { pool_id: PoolId }, // More to come...
}

impl<PoolId> Encode for Message<PoolId> {
	fn encode(&self) -> Vec<u8> {
		match self {
			Message::AddPool { pool_id } => {
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
	use frame_support::storage::child::ChildInfo;

	#[test]
	fn test_message_encode() {
		let encoded = Message::<u64>::AddPool { pool_id: 42 }.encode();
		assert_eq!(encoded, vec![0, 1, 2, 3])
	}
}
