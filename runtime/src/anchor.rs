use support::{decl_module, decl_storage, decl_event, StorageMap, dispatch::Result, ensure};

use system::{ensure_signed};
use runtime_primitives::traits::{As, Hash};
use parity_codec::{Encode, Decode};

// expiration duration in blocks of a pre commit
const EXPIRATION_DURATION_BLOCKS: u64 = 480;

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct PreAnchorData<Hash, AccountId, BlockNumber> {
	signing_root: Hash,
	identity: AccountId,
	expiration_block: BlockNumber,
}

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct AnchorData<Hash, BlockNumber> {
	id: Hash,
	doc_root: Hash,
	anchored_block: BlockNumber,
}

/// The module's configuration trait.
pub trait Trait: system::Trait {

	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_event!(
	pub enum Event<T> where <T as system::Trait>::Hash, <T as system::Trait>::AccountId, <T as system::Trait>::BlockNumber {
		// AnchorPreCommitted event with account, anchor_id and expiration block number info
		AnchorPreCommitted(AccountId, Hash, BlockNumber),

		// AnchorCommitted event with account, anchor_id, doc_root and block number info
		AnchorCommitted(AccountId, Hash, Hash, BlockNumber),
	}
);

decl_storage! {
	trait Store for Module<T: Trait> as Anchor {

		// Pre Anchors store the map of anchor Id to the pre anchor, which is a lock on an anchor id to be committed later
		PreAnchors get(get_pre_anchor): map T::Hash => PreAnchorData<T::Hash, T::AccountId, T::BlockNumber>;

		// Anchors store the map of anchor Id to the anchor
		Anchors get(get_anchor): map T::Hash => AnchorData<T::Hash, T::BlockNumber>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		// Initializing events
		fn deposit_event<T>() = default;

		pub fn pre_commit(origin, anchor_id: T::Hash, signing_root: T::Hash) -> Result {
			let who = ensure_signed(origin)?;
			ensure!(!<Anchors<T>>::exists(anchor_id), "Anchor already exists");

			ensure!(!Self::has_valid_pre_commit(anchor_id), "A valid pre anchor already exists");

			let expiration_block = <system::Module<T>>::block_number()  + As::sa(EXPIRATION_DURATION_BLOCKS);
			<PreAnchors<T>>::insert(anchor_id, PreAnchorData {
				signing_root: signing_root,
				identity: who.clone(),
				expiration_block: expiration_block,
			});

			Self::deposit_event(RawEvent::AnchorPreCommitted(who, anchor_id, expiration_block));
			Ok(())
		}
	
		pub fn commit(origin, anchor_id_preimage: T::Hash, doc_root: T::Hash, _proof: T::Hash) -> Result {
			let who = ensure_signed(origin)?;

			let anchor_id = (anchor_id_preimage)
                .using_encoded(<T as system::Trait>::Hashing::hash);
			ensure!(!<Anchors<T>>::exists(anchor_id), "Anchor already exists");

			if Self::has_valid_pre_commit(anchor_id) {
			    ensure!(<PreAnchors<T>>::get(anchor_id).identity == who, "Precommit owned by someone else")

			    // TODO research sha256 usage + merkle proof validation
			}


			let block_num = <system::Module<T>>::block_number();
			<Anchors<T>>::insert(anchor_id, AnchorData {
				id: anchor_id,
				doc_root: doc_root,
				anchored_block: block_num,
			});

			Self::deposit_event(RawEvent::AnchorCommitted(who, anchor_id, doc_root, block_num));
			Ok(())
		}
	}
}

impl<T: Trait> Module<T> {

	fn has_valid_pre_commit(anchor_id: T::Hash) -> bool {
		if !<PreAnchors<T>>::exists(&anchor_id) {
			return false
		}

		!(<PreAnchors<T>>::get(anchor_id).expiration_block > <system::Module<T>>::block_number())
	}
}


/// tests for anchor module
#[cfg(test)]
mod tests {
	use super::*;

	use runtime_io::with_externalities;
	use primitives::{H256, Blake2Hasher};
	use support::{impl_outer_origin, assert_ok};
	use runtime_primitives::{
		BuildStorage,
		traits::{BlakeTwo256, IdentityLookup},
		testing::{Digest, DigestItem, Header}
	};

	impl_outer_origin! {
		pub enum Origin for Test {}
	}

	// For testing the module, we construct most of a mock runtime. This means
	// first constructing a configuration type (`Test`) which `impl`s each of the
	// configuration traits of modules we want to use.
	#[derive(Clone, Eq, PartialEq)]
	pub struct Test;
	impl system::Trait for Test {
		type Origin = Origin;
		type Index = u64;
		type BlockNumber = u64;
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type Digest = Digest;
		type AccountId = u64;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Header = Header;
		type Event = ();
		type Log = DigestItem;
	}
	impl Trait for Test {
		type Event = ();
	}

	type Anchor = Module<Test>;

	// This function basically just builds a genesis storage key/value store according to
	// our desired mockup.
	fn new_test_ext() -> runtime_io::TestExternalities<Blake2Hasher> {
		system::GenesisConfig::<Test>::default().build_storage().unwrap().0.into()
	}

	#[test]
	fn basic_anchor() {
		with_externalities(&mut new_test_ext(), || {
			let pre_image = <Test as system::Trait>::Hashing::hash_of(&0);
			let anchor_id = (pre_image)
				.using_encoded(<Test as system::Trait>::Hashing::hash);
			let doc_root = <Test as system::Trait>::Hashing::hash_of(&0);
			assert_ok!(Anchor::commit(Origin::signed(1), pre_image,
				doc_root, <Test as system::Trait>::Hashing::hash_of(&0)));
			// asserting that the stored anchor id is what we sent the pre-image for
			let a = Anchor::get_anchor(anchor_id);
			assert_eq!(a.id, anchor_id);
			assert_eq!(a.doc_root, doc_root);
		});
	}
}