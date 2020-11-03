use crate::nft;
use bridge_names;
use core::convert::TryInto;
use codec::{Decode, Encode};
use unique_assets::traits::Unique;
use crate::registry::types::{RegistryId, AssetId, TokenId};
use crate::{fees, constants::currency};
use frame_support::traits::{Currency, ExistenceRequirement::AllowDeath, Get};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult, ensure,
    traits::EnsureOrigin,
};
use frame_system::{self as system, ensure_signed};
use sp_core::{U256, Bytes};
use sp_runtime::traits::SaturatedConversion;
use sp_std::prelude::*;

/// Abstract identifer of an asset, for a common vocabulary across chains.
pub type ResourceId = chainbridge::ResourceId;

const ADDR_LEN: usize = 32;
type Bytes32 = [u8; ADDR_LEN];
/// A generic representation of a local address. A resource id points to this. It may be a
/// registry id (20 bytes) or a fungible asset type (in the future). Constrained to 32 bytes just
/// as an upper bound to store efficiently.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, Debug)]
pub struct Address(pub Bytes32);

type BalanceOf<T> =
    <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;

/// Additional Fee charged when moving native tokens to target chains (RAD)
const TOKEN_FEE: u128 = 20 * currency::RAD;
/// Additional Fee charged when move an NFT to target chain
const NFT_FEE: u128 = 10 * currency::RAD;

impl From<RegistryId> for Address {
    fn from(r: RegistryId) -> Self {
        // Pad 12 bytes to the registry id - total 32 bytes
        let padded = r.to_fixed_bytes().iter().copied()
            .chain([0; 12].iter().copied()).collect::<Vec<u8>>()[..ADDR_LEN]
            .try_into().expect("RegistryId is 20 bytes. 12 are padded. Converting to a 32 byte array should never fail");

        Address( padded )
    }
}

// In order to be generic into T::Address
impl From<Bytes32> for Address {
    fn from(v: Bytes32) -> Self {
        Address( v[..ADDR_LEN].try_into().expect("Address wraps a 32 byte array") )
    }
}
impl From<Address> for Bytes32 {
    fn from(a: Address) -> Self {
        a.0
    }
}

pub trait Trait: system::Trait
               + fees::Trait
               + pallet_balances::Trait
               + chainbridge::Trait
               + nft::Trait
               + bridge_names::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    /// Specifies the origin check provided by the chainbridge for calls that can only be called by the chainbridge pallet
    type BridgeOrigin: EnsureOrigin<Self::Origin, Success = Self::AccountId>;
    type Currency: Currency<Self::AccountId>;
    /// Ids can be defined by the runtime and passed in, perhaps from blake2b_128 hashes.
    type HashId: Get<ResourceId>;
    type NativeTokenId: Get<ResourceId>;
}

decl_storage! {
    trait Store for Module<T: Trait> as PalletBridge {}

    add_extra_genesis {
        config(chains): Vec<u8>;
        config(relayers): Vec<T::AccountId>;
        config(resources): Vec<(ResourceId, Vec<u8>)>;
        config(threshold): u32;

        build(|config| Module::<T>::initialize(&config.chains, &config.relayers, &config.resources, &config.threshold))
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
        /// Resource id provided on initiating a transfer is not a key in bridges-names mapping.
        ResourceIdDoesNotExist,
        /// Registry id provided on recieving a transfer is not a key in bridges-names mapping.
        RegistryIdDoesNotExist,
        InvalidTransfer,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        const HashId: ResourceId = T::HashId::get();
        const NativeTokenId: ResourceId = T::NativeTokenId::get();

        fn deposit_event() = default;

        /// Transfers some amount of the native token to some recipient on a (whitelisted) destination chain.
        #[weight = 195_000_000]
        pub fn transfer_native(origin, amount: BalanceOf<T>, recipient: Vec<u8>, dest_id: chainbridge::ChainId) -> DispatchResult {
            let source = ensure_signed(origin)?;

            let token_fee: T::Balance = TOKEN_FEE.saturated_into();
			let total_amount = U256::from(amount.saturated_into()).saturating_add(U256::from(token_fee.saturated_into()));

            // Ensure account has enough balance for both fee and transfer
            // Check to avoid balance errors down the line that leave balance storage in an inconsistent state
            let current_balance = T::Currency::free_balance(&source);
            ensure!(U256::from(current_balance.saturated_into()) >= total_amount, "Insufficient Balance");

            ensure!(<chainbridge::Module<T>>::chain_whitelisted(dest_id), Error::<T>::InvalidTransfer);

            // Burn additional fees
            <fees::Module<T>>::burn_fee(&source, token_fee)?;

            let bridge_id = <chainbridge::Module<T>>::account_id();
            T::Currency::transfer(&source, &bridge_id, amount.into(), AllowDeath)?;

            let resource_id = T::NativeTokenId::get();
            <chainbridge::Module<T>>::transfer_fungible(dest_id, resource_id, recipient, U256::from(amount.saturated_into()))?;
            Ok(())
        }

        /// Transfer an nft to a whitelisted destination chain. Source nft is locked in bridge account
        /// rather than being burned.
        #[weight = 195_000_000]
        pub fn transfer_asset(origin,
                              recipient: Vec<u8>,
                              from_registry: RegistryId,
                              token_id: TokenId,
                              dest_id: chainbridge::ChainId,
        ) -> DispatchResult {
            let source = ensure_signed(origin)?;

            /// Get resource id from registry
            let reg: Address = from_registry.into();
            let reg: Bytes32 = reg.into();
            let reg: <T as bridge_names::Trait>::Address = reg.into();
            let resource_id = <bridge_names::Module<T>>::name_of(reg)
                .ok_or(Error::<T>::ResourceIdDoesNotExist)?;

            // Burn additional fees
            let nft_fee: T::Balance = NFT_FEE.saturated_into();
            <fees::Module<T>>::burn_fee(&source, nft_fee)?;

            // Lock asset by transfering to bridge account
            let bridge_id = <chainbridge::Module<T>>::account_id();
            let asset_id = AssetId(from_registry, token_id);
            <nft::Module<T> as Unique>::transfer(&source, &bridge_id, &asset_id)?;

            // Transfer instructions for relayer
            let tid: &mut [u8] = &mut[0; 32];
            // Ethereum is big-endian
            token_id.to_big_endian(tid);
            <chainbridge::Module<T>>::transfer_nonfungible(dest_id,
                                                           resource_id.into(),
                                                           tid.to_vec(),
                                                           recipient,
                                                           vec![]/*assetinfo.metadata*/)
        }

        //
        // Executable calls. These can be triggered by a chainbridge transfer initiated on another chain
        //

        /// Executes a simple currency transfer using the chainbridge account as the source
        #[weight = 195_000_000]
        pub fn transfer(origin, to: T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
            let source = T::BridgeOrigin::ensure_origin(origin)?;
            T::Currency::transfer(&source, &to, amount.into(), AllowDeath)?;
            Ok(())
        }

        #[weight = 195_000_000]
        pub fn recieve_nonfungible(origin,
                                   to: T::AccountId,
                                   token_id: TokenId,
                                   resource_id: ResourceId
        ) -> DispatchResult {
            let source = T::BridgeOrigin::ensure_origin(origin)?;

            /// Get registry from resource id
            let rid: <T as bridge_names::Trait>::ResourceId = resource_id.into();
            let registry_id = <bridge_names::Module<T>>::addr_of(rid)
                .ok_or(Error::<T>::RegistryIdDoesNotExist)?;
            let registry_id: Address = registry_id.into().into();

            // Transfer from bridge account to destination account
            let asset_id = AssetId(registry_id.into(), token_id);
            <nft::Module<T> as Unique>::transfer(&source, &to, &asset_id)
        }

        /// This can be called by the chainbridge to demonstrate an arbitrary call from a proposal.
        #[weight = 195_000_000]
        pub fn remark(origin, hash: T::Hash) -> DispatchResult {
            T::BridgeOrigin::ensure_origin(origin)?;
            Self::deposit_event(RawEvent::Remark(hash));
            Ok(())
        }

    }
}

impl<T: Trait> Module<T> {
    /// Its called as part of genesis step to initialize some dev parameters
    fn initialize(
        chains: &[u8],
        relayers: &[T::AccountId],
        resources: &Vec<(ResourceId, Vec<u8>)>,
        threshold: &u32,
    ) {
        chains.into_iter().for_each(|c| {
            <chainbridge::Module<T>>::whitelist(*c).unwrap_or_default();
        });
        relayers.into_iter().for_each(|rs| {
            <chainbridge::Module<T>>::register_relayer(rs.clone()).unwrap_or_default();
        });
        <chainbridge::Module<T>>::set_relayer_threshold(*threshold).unwrap_or_default();
        for &(ref re, ref m) in resources.iter() {
            <chainbridge::Module<T>>::register_resource(*re, m.clone()).unwrap_or_default();
        }
    }
}

#[cfg(test)]
mod tests{
	use super::*;
	use frame_support::dispatch::DispatchError;
	use frame_support::{assert_err, assert_noop, assert_ok};
	use codec::Encode;
	use sp_core::{blake2_256, H256};
	use frame_support::{ord_parameter_types, parameter_types, weights::Weight};
	use frame_system::{self as system, EnsureSignedBy};
	use sp_core::hashing::blake2_128;
	use sp_runtime::{
		testing::Header,
		traits::{AccountIdConversion, BlakeTwo256, Block as BlockT, IdentityLookup}, ModuleId, Perbill,
	};
	use crate::bridge as pallet_bridge;
    use crate::nft;

	pub use pallet_balances as balances;

	const TEST_THRESHOLD: u32 = 2;

	parameter_types! {
		pub const BlockHashCount: u64 = 250;
		pub const MaximumBlockWeight: Weight = 1024;
		pub const MaximumBlockLength: u32 = 2 * 1024;
		pub const AvailableBlockRatio: Perbill = Perbill::one();
	}

	impl frame_system::Trait for Test {
		type BaseCallFilter = ();
		type Origin = Origin;
		type Call = Call;
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
		type DbWeight = ();
		type BlockExecutionWeight = ();
		type ExtrinsicBaseWeight = ();
		type MaximumExtrinsicWeight = ();
		type MaximumBlockLength = MaximumBlockLength;
		type AvailableBlockRatio = AvailableBlockRatio;
		type Version = ();
		type ModuleToIndex = ();
		type AccountData = balances::AccountData<u128>;
		type MigrateAccount = ();
		type OnNewAccount = ();
		type OnKilledAccount = ();
		type SystemWeightInfo = ();
	}

	parameter_types! {
		pub const ExistentialDeposit: u64 = 1;
	}

	ord_parameter_types! {
		pub const One: u64 = 1;
	}

	impl pallet_balances::Trait for Test {
		type Balance = u128;
		type DustRemoval = ();
		type Event = Event;
		type ExistentialDeposit = ExistentialDeposit;
		type AccountStore = System;
		type WeightInfo = ();
	}

	parameter_types! {
		pub const TestChainId: u8 = 5;
		pub const ProposalLifetime: u64 = 10;
	}

	impl chainbridge::Trait for Test {
		type Event = Event;
		type Proposal = Call;
		type ChainId = TestChainId;
		type AdminOrigin = EnsureSignedBy<One, u64>;
		type ProposalLifetime = ProposalLifetime;
	}

	impl fees::Trait for Test {
		type Event = Event;
		type FeeChangeOrigin = frame_system::EnsureRoot<u64>;
	}

	impl pallet_authorship::Trait for Test {
		type FindAuthor = ();
		type UncleGenerations = ();
		type FilterUncle = ();
		type EventHandler = ();
	}

    impl nft::Trait for Test {
        type Event = Event;
        type AssetInfo = crate::registry::types::AssetInfo;
    }

    impl bridge_names::Trait for Test {
        type ResourceId = ResourceId;
        type Address = Address;
        type Admin = frame_system::EnsureRoot<Self::AccountId>;
    }

	parameter_types! {
		pub HashId: chainbridge::ResourceId = chainbridge::derive_resource_id(1, &blake2_128(b"hash"));
		pub NativeTokenId: chainbridge::ResourceId = chainbridge::derive_resource_id(1, &blake2_128(b"xRAD"));
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
			System: system::{Module, Call, Config, Storage, Event<T>},
			Balances: balances::{Module, Call, Storage, Config<T>, Event<T>},
			ChainBridge: chainbridge::{Module, Call, Storage, Event<T>},
			PalletBridge: pallet_bridge::{Module, Call, Event<T>},
			Fees: fees::{Module, Call, Event<T>},
            Nft: nft::{Module, Event<T>},
		}
	);

	pub const RELAYER_A: u64 = 0x2;
	pub const RELAYER_B: u64 = 0x3;
	pub const RELAYER_C: u64 = 0x4;
	pub const ENDOWED_BALANCE: u128 = 100 * currency::RAD;

    pub fn new_test_ext() -> sp_io::TestExternalities {
        let bridge_id = ModuleId(*b"cb/bridg").into_account();
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Test>()
            .unwrap();
        pallet_balances::GenesisConfig::<Test> {
            balances: vec![
                (bridge_id, ENDOWED_BALANCE),
                (RELAYER_A, ENDOWED_BALANCE),
                (RELAYER_B, 100),
            ],
        }
            .assimilate_storage(&mut t)
            .unwrap();
        let mut ext = sp_io::TestExternalities::new(t);
        ext.execute_with(|| System::set_block_number(1));
        ext
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

	fn make_transfer_proposal(to: u64, amount: u128) -> Call {
		Call::PalletBridge(crate::bridge::Call::transfer(to, amount))
	}


	#[test]
	fn transfer_native() {
		new_test_ext().execute_with(|| {
			let dest_chain = 0;
			let resource_id = NativeTokenId::get();
			let amount: u128 = 20 * currency::RAD;
			let recipient = vec![99];

			assert_ok!(ChainBridge::whitelist_chain(Origin::root(), dest_chain.clone()));

			// Using account with not enough balance for fee should fail when requesting transfer
			assert_err!(
				PalletBridge::transfer_native(
					Origin::signed(RELAYER_C),
					amount.clone(),
					recipient.clone(),
					dest_chain,
				),
				"Insufficient Balance"
			);

			let mut account_current_balance = <pallet_balances::Module<Test>>::free_balance(RELAYER_B);
			assert_eq!(account_current_balance, 100);

			// Using account with enough balance for fee but not for transfer amount
			assert_err!(
				PalletBridge::transfer_native(
					Origin::signed(RELAYER_B),
					amount.clone(),
					recipient.clone(),
					dest_chain,
				),
				"Insufficient Balance"
			);

			// Account balance should be reverted to original balance
			account_current_balance = <pallet_balances::Module<Test>>::free_balance(RELAYER_B);
			assert_eq!(account_current_balance, 100);

			// Success
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

			// Account balance should be reduced amount + fee
			account_current_balance = <pallet_balances::Module<Test>>::free_balance(RELAYER_A);
			assert_eq!(account_current_balance, 60 * currency::RAD);
		})
	}

    #[test]
    fn transfer_nonfungible_asset() {
        new_test_ext().execute_with(|| {
            let dest_chain = 0;
            let resource_id = NativeTokenId::get();
            let recipient = vec![1];
            let owner = RELAYER_B;
            let (registry_id, token_id) = crate::registry::tests::mint_nft::<Test>(owner).destruct();

            // Whitelist destination chain
            assert_ok!(ChainBridge::whitelist_chain(Origin::root(), dest_chain.clone()));

            let nft_owner = <crate::nft::Module<Test>>::account_for_asset(registry_id, token_id);
            assert!(nft_owner != owner);

            // Transfer nonfungible
            assert_ok!(
                PalletBridge::transfer_asset(
                    Origin::signed(owner),
                    recipient.clone(),
                    registry_id,
                    token_id,
                    dest_chain));

            let nft_owner = <crate::nft::Module<Test>>::account_for_asset(registry_id, token_id);
            assert_eq!(nft_owner, RELAYER_B);
            // Check that nft is locked in bridge account
            // Check that transfer event was emitted
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

			assert_ok!(ChainBridge::set_threshold(Origin::root(), TEST_THRESHOLD,));
			assert_ok!(ChainBridge::add_relayer(Origin::root(), RELAYER_A));
			assert_ok!(ChainBridge::add_relayer(Origin::root(), RELAYER_B));
			assert_ok!(ChainBridge::whitelist_chain(Origin::root(), src_id));
			assert_ok!(ChainBridge::set_resource(Origin::root(), r_id, resource));

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
				PalletBridge::remark(Origin::root(), hash),
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
	fn create_successful_transfer_proposal() {
		new_test_ext().execute_with(|| {
			let prop_id = 1;
			let src_id = 1;
			let r_id = chainbridge::derive_resource_id(src_id, b"transfer");
			let resource = b"PalletBridge.transfer".to_vec();
			let proposal = make_transfer_proposal(RELAYER_A, 10);

			assert_ok!(ChainBridge::set_threshold(Origin::root(), TEST_THRESHOLD,));
			assert_ok!(ChainBridge::add_relayer(Origin::root(), RELAYER_A));
			assert_ok!(ChainBridge::add_relayer(Origin::root(), RELAYER_B));
			assert_ok!(ChainBridge::add_relayer(Origin::root(), RELAYER_C));
			assert_ok!(ChainBridge::whitelist_chain(Origin::root(), src_id));
			assert_ok!(ChainBridge::set_resource(Origin::root(), r_id, resource));

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
				status: chainbridge::ProposalStatus::Initiated,
				expiry: ProposalLifetime::get() + 1,
			};
			assert_eq!(prop, expected);

			// Second relayer votes against
			assert_ok!(ChainBridge::reject_proposal(
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
				status: chainbridge::ProposalStatus::Initiated,
				expiry: ProposalLifetime::get() + 1,
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
				expiry: ProposalLifetime::get() + 1,
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
