use support::{decl_module, decl_storage, StorageMap, dispatch::Result, ensure};

use system::{ensure_signed};
use runtime_primitives::traits::{As, Hash};
use parity_codec::{Encode, Decode};

// expiration duration in blocks of a pre commit
const EXPIRATION_DURATION_BLOCKS: u64 = 240;

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
pub trait Trait: system::Trait {}

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

		pub fn pre_commit(origin, anchor_id: T::Hash, signing_root: T::Hash) -> Result {
			let who = ensure_signed(origin)?;
			ensure!(!<Anchors<T>>::exists(anchor_id), "Anchor already exists");
			ensure!(!Self::has_valid_pre_commit(anchor_id), "A valid pre anchor already exists");

			let expiration_block = <system::Module<T>>::block_number()  + As::sa(Self::expiration_duration_blocks());
			<PreAnchors<T>>::insert(anchor_id, PreAnchorData {
				signing_root: signing_root,
				identity: who.clone(),
				expiration_block: expiration_block,
			});

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

			Ok(())
		}
	}
}

impl<T: Trait> Module<T> {

	fn has_valid_pre_commit(anchor_id: T::Hash) -> bool {
		if !<PreAnchors<T>>::exists(&anchor_id) {
			return false
		}

		<PreAnchors<T>>::get(anchor_id).expiration_block > <system::Module<T>>::block_number()
	}

	fn expiration_duration_blocks() -> u64 {
		// TODO this needs to come from governance
		EXPIRATION_DURATION_BLOCKS
	}
}


/// tests for anchor module
#[cfg(test)]
mod tests {
	use super::*;

	use runtime_io::with_externalities;
	use primitives::{H256, Blake2Hasher};
	use support::{impl_outer_origin, assert_ok, assert_err};
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
	impl Trait for Test {}

	type Anchor = Module<Test>;
	type System = system::Module<Test>;

	// This function basically just builds a genesis storage key/value store according to
	// our desired mockup.
	fn new_test_ext() -> runtime_io::TestExternalities<Blake2Hasher> {
		system::GenesisConfig::<Test>::default().build_storage().unwrap().0.into()
	}

	#[test]
	fn basic_pre_commit() {
		with_externalities(&mut new_test_ext(), || {
			let anchor_id = <Test as system::Trait>::Hashing::hash_of(&0);
			let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);

			// reject unsigned
			assert_err!(Anchor::pre_commit(Origin::INHERENT, anchor_id, signing_root), "bad origin: expected to be a signed origin");

			// happy
			assert_ok!(Anchor::pre_commit(Origin::signed(1), anchor_id, signing_root));
			// asserting that the stored pre anchor has the intended values set
			let a = Anchor::get_pre_anchor(anchor_id);
			assert_eq!(a.identity, 1);
			assert_eq!(a.signing_root, signing_root);
			assert_eq!(a.expiration_block, Anchor::expiration_duration_blocks() + 1);
		});
	}

	#[test]
	fn pre_commit_fail_anchor_exists() {
		with_externalities(&mut new_test_ext(), || {
			let pre_image = <Test as system::Trait>::Hashing::hash_of(&0);
			let anchor_id = (pre_image)
				.using_encoded(<Test as system::Trait>::Hashing::hash);
			let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);
			// anchor
			assert_ok!(Anchor::commit(Origin::signed(1), pre_image,
				<Test as system::Trait>::Hashing::hash_of(&0), <Test as system::Trait>::Hashing::hash_of(&0)));

			// fails because of existing anchor
			assert_err!(Anchor::pre_commit(Origin::signed(1), anchor_id, signing_root), "Anchor already exists");
		});
	}

	#[test]
	fn pre_commit_fail_anchor_exists_different_acc() {
		with_externalities(&mut new_test_ext(), || {
			let pre_image = <Test as system::Trait>::Hashing::hash_of(&0);
			let anchor_id = (pre_image)
				.using_encoded(<Test as system::Trait>::Hashing::hash);
			let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);
			// anchor
			assert_ok!(Anchor::commit(Origin::signed(2), pre_image,
				<Test as system::Trait>::Hashing::hash_of(&0), <Test as system::Trait>::Hashing::hash_of(&0)));

			// fails because of existing anchor
			assert_err!(Anchor::pre_commit(Origin::signed(1), anchor_id, signing_root), "Anchor already exists");
		});
	}

	#[test]
	fn pre_commit_fail_pre_anchor_exists() {
		with_externalities(&mut new_test_ext(), || {
			let anchor_id = <Test as system::Trait>::Hashing::hash_of(&0);
			let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);

			// first pre-anchor
			assert_ok!(Anchor::pre_commit(Origin::signed(1), anchor_id, signing_root));
			let a = Anchor::get_pre_anchor(anchor_id);
			assert_eq!(a.identity, 1);
			assert_eq!(a.signing_root, signing_root);
			assert_eq!(a.expiration_block, Anchor::expiration_duration_blocks() + 1);

			// fail, pre anchor exists
			assert_err!(Anchor::pre_commit(Origin::signed(1), anchor_id, signing_root), "A valid pre anchor already exists");

			// expire the pre commit
			System::set_block_number(Anchor::expiration_duration_blocks() + 2);
			assert_ok!(Anchor::pre_commit(Origin::signed(1), anchor_id, signing_root));
		});
	}

	#[test]
	fn pre_commit_fail_pre_anchor_exists_different_acc() {
		with_externalities(&mut new_test_ext(), || {
			let anchor_id = <Test as system::Trait>::Hashing::hash_of(&0);
			let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);

			// first pre-anchor
			assert_ok!(Anchor::pre_commit(Origin::signed(1), anchor_id, signing_root));
			let a = Anchor::get_pre_anchor(anchor_id);
			assert_eq!(a.identity, 1);
			assert_eq!(a.signing_root, signing_root);
			assert_eq!(a.expiration_block, Anchor::expiration_duration_blocks() + 1);

			// fail, pre anchor exists
			assert_err!(Anchor::pre_commit(Origin::signed(2), anchor_id, signing_root), "A valid pre anchor already exists");

			// expire the pre commit
			System::set_block_number(Anchor::expiration_duration_blocks() + 2);
			assert_ok!(Anchor::pre_commit(Origin::signed(2), anchor_id, signing_root));
		});
	}

	#[test]
	fn basic_commit() {
		with_externalities(&mut new_test_ext(), || {
			let pre_image = <Test as system::Trait>::Hashing::hash_of(&0);
			let anchor_id = (pre_image)
				.using_encoded(<Test as system::Trait>::Hashing::hash);
			let doc_root = <Test as system::Trait>::Hashing::hash_of(&0);
			// reject unsigned
			assert_err!(Anchor::commit(Origin::INHERENT, pre_image,
				doc_root, <Test as system::Trait>::Hashing::hash_of(&0)), "bad origin: expected to be a signed origin");

			// happy
			assert_ok!(Anchor::commit(Origin::signed(1), pre_image,
				doc_root, <Test as system::Trait>::Hashing::hash_of(&0)));
			// asserting that the stored anchor id is what we sent the pre-image for
			let a = Anchor::get_anchor(anchor_id);
			assert_eq!(a.id, anchor_id);
			assert_eq!(a.doc_root, doc_root);

		});
	}

	#[test]
	fn commit_fail_anchor_exists() {
		with_externalities(&mut new_test_ext(), || {
			let pre_image = <Test as system::Trait>::Hashing::hash_of(&0);
			let anchor_id = (pre_image)
				.using_encoded(<Test as system::Trait>::Hashing::hash);
			let doc_root = <Test as system::Trait>::Hashing::hash_of(&0);

			// happy
			assert_ok!(Anchor::commit(Origin::signed(1), pre_image,
				doc_root, <Test as system::Trait>::Hashing::hash_of(&0)));
			// asserting that the stored anchor id is what we sent the pre-image for
			let a = Anchor::get_anchor(anchor_id);
			assert_eq!(a.id, anchor_id);
			assert_eq!(a.doc_root, doc_root);

			assert_err!(Anchor::commit(Origin::signed(1), pre_image,
				doc_root, <Test as system::Trait>::Hashing::hash_of(&0)), "Anchor already exists");

			// different acc
			assert_err!(Anchor::commit(Origin::signed(2), pre_image,
            	doc_root, <Test as system::Trait>::Hashing::hash_of(&0)), "Anchor already exists");
		});
	}

	// TODO pre-commit + commit tests
}