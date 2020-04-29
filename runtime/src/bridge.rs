use frame_support::traits::{Currency, ExistenceRequirement::AllowDeath, Get};
use frame_support::{decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult, ensure};
use frame_system::{self as system, ensure_signed};
use sp_runtime::traits::EnsureOrigin;
use sp_std::prelude::*;
use sp_core::U256;
use sp_runtime::traits::SaturatedConversion;

type ResourceId = chainbridge::ResourceId;
type BalanceOf<T> =
    <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;

pub trait Trait: system::Trait + chainbridge::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    /// Specifies the origin check provided by the chainbridge for calls that can only be called by the chainbridge pallet
    type BridgeOrigin: EnsureOrigin<Self::Origin, Success = Self::AccountId>;
 	type Currency: Currency<Self::AccountId>;
    /// Ids can be defined by the runtime and passed in, perhaps from blake2b_128 hashes.
    type HashId: Get<ResourceId>;
    type NativeTokenId: Get<ResourceId>;
}

decl_storage! {
	trait Store for Module<T: Trait> as Bridge {}

	add_extra_genesis {
        config(chains): Vec<u8>;
        config(relayers): Vec<T::AccountId>;
        config(resources): Vec<ResourceId>;

        build(|config| Module::<T>::initialize(&config.chains, &config.relayers, &config.resources))
    }
}

decl_event! {
    pub enum Event<T> where
        <T as frame_system::Trait>::Hash,
    {
        Remark(Hash),
    }
}

decl_error! {
    pub enum Error for Module<T: Trait>{
        InvalidTransfer,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        const HashId: ResourceId = T::HashId::get();
        const NativeTokenId: ResourceId = T::NativeTokenId::get();

        fn deposit_event() = default;

        /// Transfers some amount of the native token to some recipient on a (whitelisted) destination chain.
        pub fn transfer_native(origin, amount: BalanceOf<T>, recipient: Vec<u8>, dest_id: chainbridge::ChainId) -> DispatchResult {
            let source = ensure_signed(origin)?;
            ensure!(<chainbridge::Module<T>>::chain_whitelisted(dest_id), Error::<T>::InvalidTransfer);
            let bridge_id = <chainbridge::Module<T>>::account_id();
            T::Currency::transfer(&source, &bridge_id, amount.into(), AllowDeath)?;

            let resource_id = T::NativeTokenId::get();
            <chainbridge::Module<T>>::transfer_fungible(dest_id, resource_id, recipient, U256::from(amount.saturated_into()))?;
			Ok(())
        }


        //
        // Executable calls. These can be triggered by a chainbridge transfer initiated on another chain
        //

        /// Executes a simple currency transfer using the chainbridge account as the source
        pub fn transfer(origin, to: T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
            let source = T::BridgeOrigin::ensure_origin(origin)?;
            T::Currency::transfer(&source, &to, amount.into(), AllowDeath)?;
            Ok(())
        }

        /// This can be called by the chainbridge to demonstrate an arbitrary call from a proposal.
        pub fn remark(origin, hash: T::Hash) -> DispatchResult {
            T::BridgeOrigin::ensure_origin(origin)?;
            Self::deposit_event(RawEvent::Remark(hash));
            Ok(())
        }

    }
}

impl<T: Trait> Module<T> {
	/// Its called as part of genesis step to initialize some dev parameters
	fn initialize(chains: &[u8], relayers: &[T::AccountId], resources: &[ResourceId]) {
		chains.into_iter().for_each(|c| {
			<chainbridge::Module<T>>::whitelist(*c).unwrap_or_default();
		});
		relayers.into_iter().for_each(|rs| {
			<chainbridge::Module<T>>::register_relayer(rs.clone()).unwrap_or_default();
		});
		if !relayers.is_empty() {
			<chainbridge::Module<T>>::set_relayer_threshold(relayers.len() as u32).unwrap_or_default();
		}
		resources.into_iter().for_each(|re| {
			<chainbridge::Module<T>>::register_resource(*re, vec![0]).unwrap_or_default();
		});
	}
}

#[cfg(test)]
mod tests{
	use super::*;
	use frame_support::dispatch::DispatchError;
	use frame_support::{assert_noop, assert_ok};
	use codec::Encode;
	use sp_core::{blake2_256, H256};
	use frame_support::{ord_parameter_types, parameter_types, weights::Weight};
	use frame_system::{self as system, EnsureSignedBy};
	use sp_core::hashing::blake2_128;
	use sp_runtime::{
		testing::Header,
		traits::{AccountIdConversion, BlakeTwo256, Block as BlockT, IdentityLookup},
		BuildStorage, ModuleId, Perbill,
	};
	use crate::bridge as pallet_bridge;

	pub use pallet_balances as balances;

	const TEST_THRESHOLD: u32 = 2;



	parameter_types! {
		pub const BlockHashCount: u64 = 250;
		pub const MaximumBlockWeight: Weight = 1024;
		pub const MaximumBlockLength: u32 = 2 * 1024;
		pub const AvailableBlockRatio: Perbill = Perbill::one();
	}

	impl frame_system::Trait for Test {
		type Origin = Origin;
		type Call = ();
		type Index = u64;
		type BlockNumber = u64;
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type AccountId = u64;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Header = Header;
		type Event = Event;
		type BlockHashCount = BlockHashCount;
		type MaximumBlockWeight = MaximumBlockWeight;
		type MaximumBlockLength = MaximumBlockLength;
		type AvailableBlockRatio = AvailableBlockRatio;
		type Version = ();
		type ModuleToIndex = ();
		type AccountData = balances::AccountData<u64>;
		type OnNewAccount = ();
		type OnKilledAccount = ();
	}

	parameter_types! {
		pub const ExistentialDeposit: u64 = 1;
	}

	ord_parameter_types! {
		pub const One: u64 = 1;
	}

	impl pallet_balances::Trait for Test {
		type Balance = u64;
		type DustRemoval = ();
		type Event = Event;
		type ExistentialDeposit = ExistentialDeposit;
		type AccountStore = System;
	}

	parameter_types! {
		pub const TestChainId: u8 = 5;
	}

	impl chainbridge::Trait for Test {
		type Event = Event;
		type Proposal = Call;
		type ChainId = TestChainId;
		type AdminOrigin = EnsureSignedBy<One, u64>;
	}

	parameter_types! {
		pub const HashId: chainbridge::ResourceId = chainbridge::derive_resource_id(1, &blake2_128(b"hash"));
		pub const NativeTokenId: chainbridge::ResourceId = chainbridge::derive_resource_id(1, &blake2_128(b"xRAD"));
	}

	impl Trait for Test {
		type Event = Event;
		type BridgeOrigin = chainbridge::EnsureBridge<Test>;
		type Currency = Balances;
		type HashId = HashId;
		type NativeTokenId = NativeTokenId;
	}

	pub type Block = sp_runtime::generic::Block<Header, UncheckedExtrinsic>;
	pub type UncheckedExtrinsic = sp_runtime::generic::UncheckedExtrinsic<u32, u64, Call, ()>;

	frame_support::construct_runtime!(
		pub enum Test where
			Block = Block,
			NodeBlock = Block,
			UncheckedExtrinsic = UncheckedExtrinsic
		{
			System: system::{Module, Call, Event<T>},
			Balances: balances::{Module, Call, Storage, Config<T>, Event<T>},
			ChainBridge: chainbridge::{Module, Call, Storage, Event<T>},
			PalletBridge: pallet_bridge::{Module, Call, Event<T>}
		}
	);

	pub const RELAYER_A: u64 = 0x2;
	pub const RELAYER_B: u64 = 0x3;
	pub const RELAYER_C: u64 = 0x4;
	pub const ENDOWED_BALANCE: u64 = 100_000_000;

	pub fn new_test_ext() -> sp_io::TestExternalities {
		let bridge_id = ModuleId(*b"cb/bridg").into_account();
		GenesisConfig {
			balances: Some(balances::GenesisConfig {
				balances: vec![(bridge_id, ENDOWED_BALANCE), (RELAYER_A, ENDOWED_BALANCE)],
			}),
		}
		.build_storage()
		.unwrap()
		.into()
	}

	fn last_event() -> Event {
		system::Module::<Test>::events()
			.pop()
			.map(|e| e.event)
			.expect("Event expected")
	}

	pub fn expect_event<E: Into<Event>>(e: E) {
		assert_eq!(last_event(), e.into());
	}

	// Asserts that the event was emitted at some point.
	pub fn event_exists<E: Into<Event>>(e: E) {
		let actual: Vec<Event> = system::Module::<Test>::events()
			.iter()
			.map(|e| e.event.clone())
			.collect();
		let e: Event = e.into();
		let mut exists = false;
		for evt in actual {
			if evt == e {
				exists = true;
				break;
			}
		}
		assert!(exists);
	}

	// Checks events against the latest. A contiguous set of events must be provided. They must
	// include the most recent event, but do not have to include every past event.
	pub fn assert_events(mut expected: Vec<Event>) {
		let mut actual: Vec<Event> = system::Module::<Test>::events()
			.iter()
			.map(|e| e.event.clone())
			.collect();

		expected.reverse();

		for evt in expected {
			let next = actual.pop().expect("event expected");
			assert_eq!(next, evt.into(), "Events don't match");
		}
	}

	fn make_remark_proposal(hash: H256) -> Call {
		Call::PalletBridge(crate::bridge::Call::remark(hash))
	}

	fn make_transfer_proposal(to: u64, amount: u64) -> Call {
		Call::PalletBridge(crate::bridge::Call::transfer(to, amount))
	}


	#[test]
	fn transfer_native() {
		new_test_ext().execute_with(|| {
			let dest_chain = 0;
			let resource_id = NativeTokenId::get();
			let amount: u64 = 100;
			let recipient = vec![99];

			assert_ok!(ChainBridge::whitelist_chain(Origin::ROOT, dest_chain.clone()));
			assert_ok!(PalletBridge::transfer_native(
				Origin::signed(RELAYER_A),
				amount.clone(),
				recipient.clone(),
				dest_chain,
			));

			expect_event(chainbridge::RawEvent::FungibleTransfer(
				dest_chain,
				1,
				resource_id,
				amount.into(),
				recipient,
			));
		})
	}


	#[test]
	fn execute_remark() {
		new_test_ext().execute_with(|| {
			let hash: H256 = "ABC".using_encoded(blake2_256).into();
			let proposal = make_remark_proposal(hash.clone());
			let prop_id = 1;
			let src_id = 1;
			let r_id = chainbridge::derive_resource_id(src_id, b"hash");
			let resource = b"PalletBridge.remark".to_vec();

			assert_ok!(ChainBridge::set_threshold(Origin::ROOT, TEST_THRESHOLD,));
			assert_ok!(ChainBridge::add_relayer(Origin::ROOT, RELAYER_A));
			assert_ok!(ChainBridge::add_relayer(Origin::ROOT, RELAYER_B));
			assert_ok!(ChainBridge::whitelist_chain(Origin::ROOT, src_id));
			assert_ok!(ChainBridge::set_resource(Origin::ROOT, r_id, resource));

			assert_ok!(ChainBridge::acknowledge_proposal(
				Origin::signed(RELAYER_A),
				prop_id,
				src_id,
				r_id,
				Box::new(proposal.clone())
			));
			assert_ok!(ChainBridge::acknowledge_proposal(
				Origin::signed(RELAYER_B),
				prop_id,
				src_id,
				r_id,
				Box::new(proposal.clone())
			));

			event_exists(RawEvent::Remark(hash));
		})
	}

	#[test]
	fn execute_remark_bad_origin() {
		new_test_ext().execute_with(|| {
			let hash: H256 = "ABC".using_encoded(blake2_256).into();

			assert_ok!(PalletBridge::remark(Origin::signed(ChainBridge::account_id()), hash));
			// Don't allow any signed origin except from chainbridge addr
			assert_noop!(
				PalletBridge::remark(Origin::signed(RELAYER_A), hash),
				DispatchError::BadOrigin
			);
			// Don't allow root calls
			assert_noop!(
				PalletBridge::remark(Origin::ROOT, hash),
				DispatchError::BadOrigin
			);
		})
	}

	#[test]
	fn transfer() {
		new_test_ext().execute_with(|| {
			// Check inital state
			let bridge_id: u64 = ChainBridge::account_id();
			assert_eq!(Balances::free_balance(&bridge_id), ENDOWED_BALANCE);
			// Transfer and check result
			assert_ok!(PalletBridge::transfer(
				Origin::signed(ChainBridge::account_id()),
				RELAYER_A,
				10
			));
			assert_eq!(Balances::free_balance(&bridge_id), ENDOWED_BALANCE - 10);
			assert_eq!(Balances::free_balance(RELAYER_A), ENDOWED_BALANCE + 10);

			assert_events(vec![Event::balances(balances::RawEvent::Transfer(
				ChainBridge::account_id(),
				RELAYER_A,
				10,
			))]);
		})
	}

	#[test]
	fn create_sucessful_transfer_proposal() {
		new_test_ext().execute_with(|| {
			let prop_id = 1;
			let src_id = 1;
			let r_id = chainbridge::derive_resource_id(src_id, b"transfer");
			let resource = b"PalletBridge.transfer".to_vec();
			let proposal = make_transfer_proposal(RELAYER_A, 10);

			assert_ok!(ChainBridge::set_threshold(Origin::ROOT, TEST_THRESHOLD,));
			assert_ok!(ChainBridge::add_relayer(Origin::ROOT, RELAYER_A));
			assert_ok!(ChainBridge::add_relayer(Origin::ROOT, RELAYER_B));
			assert_ok!(ChainBridge::add_relayer(Origin::ROOT, RELAYER_C));
			assert_ok!(ChainBridge::whitelist_chain(Origin::ROOT, src_id));
			assert_ok!(ChainBridge::set_resource(Origin::ROOT, r_id, resource));

			// Create proposal (& vote)
			assert_ok!(ChainBridge::acknowledge_proposal(
				Origin::signed(RELAYER_A),
				prop_id,
				src_id,
				r_id,
				Box::new(proposal.clone())
			));
			let prop = ChainBridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
			let expected = chainbridge::ProposalVotes {
				votes_for: vec![RELAYER_A],
				votes_against: vec![],
				status: chainbridge::ProposalStatus::Active,
			};
			assert_eq!(prop, expected);

			// Second relayer votes against
			assert_ok!(ChainBridge::reject(
				Origin::signed(RELAYER_B),
				prop_id,
				src_id,
				r_id,
				Box::new(proposal.clone())
			));
			let prop = ChainBridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
			let expected = chainbridge::ProposalVotes {
				votes_for: vec![RELAYER_A],
				votes_against: vec![RELAYER_B],
				status: chainbridge::ProposalStatus::Active,
			};
			assert_eq!(prop, expected);

			// Third relayer votes in favour
			assert_ok!(ChainBridge::acknowledge_proposal(
				Origin::signed(RELAYER_C),
				prop_id,
				src_id,
				r_id,
				Box::new(proposal.clone())
			));
			let prop = ChainBridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
			let expected = chainbridge::ProposalVotes {
				votes_for: vec![RELAYER_A, RELAYER_C],
				votes_against: vec![RELAYER_B],
				status: chainbridge::ProposalStatus::Approved,
			};
			assert_eq!(prop, expected);

			assert_eq!(Balances::free_balance(RELAYER_A), ENDOWED_BALANCE + 10);
			assert_eq!(
				Balances::free_balance(ChainBridge::account_id()),
				ENDOWED_BALANCE - 10
			);

			assert_events(vec![
				Event::chainbridge(chainbridge::RawEvent::VoteFor(src_id, prop_id, RELAYER_A)),
				Event::chainbridge(chainbridge::RawEvent::VoteAgainst(src_id, prop_id, RELAYER_B)),
				Event::chainbridge(chainbridge::RawEvent::VoteFor(src_id, prop_id, RELAYER_C)),
				Event::chainbridge(chainbridge::RawEvent::ProposalApproved(src_id, prop_id)),
				Event::balances(balances::RawEvent::Transfer(
					ChainBridge::account_id(),
					RELAYER_A,
					10,
				)),
				Event::chainbridge(chainbridge::RawEvent::ProposalSucceeded(src_id, prop_id)),
			]);
		})
	}
}
