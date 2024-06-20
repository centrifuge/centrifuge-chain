use cfg_primitives::{
	currency_decimals, parachains, AccountId, Balance, OrderId, PoolId, TrancheId,
};
use cfg_traits::{
	investments::{Investment, OrderManager, TrancheCurrency},
	liquidity_pools::{InboundQueue, OutboundQueue},
	IdentityCurrencyConversion, Permissions, PoolInspect, PoolMutate, Seconds,
};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	fixed_point::{Quantity, Ratio},
	investments::{InvestCollection, InvestmentAccount, RedeemCollection},
	orders::FulfillmentWithPrice,
	permissions::{PermissionScope, PoolRole, Role},
	pools::TrancheMetadata,
	tokens::{AssetMetadata, CrossChainTransferability, CurrencyId, CustomMetadata},
};
use frame_support::{
	assert_noop, assert_ok,
	dispatch::RawOrigin,
	traits::{
		fungibles::{Inspect, Mutate as FungiblesMutate},
		OriginTrait, PalletInfo,
	},
};
use liquidity_pools_gateway_routers::{
	AxelarEVMRouter, AxelarXCMRouter, DomainRouter, EVMDomain, EVMRouter, EthereumXCMRouter,
	FeeValues, XCMRouter, XcmDomain, DEFAULT_PROOF_SIZE, MAX_AXELAR_EVM_CHAIN_SIZE,
};
use orml_traits::MultiCurrency;
use pallet_investments::CollectOutcome;
use pallet_liquidity_pools::Message;
use pallet_liquidity_pools_gateway::Call as LiquidityPoolsGatewayCall;
use pallet_pool_system::tranches::{TrancheInput, TrancheLoc, TrancheType};
use polkadot_core_primitives::BlakeTwo256;
use runtime_common::{
	account_conversion::AccountConverter, foreign_investments::IdentityPoolCurrencyConverter,
	xcm::general_key,
};
use sp_core::{Get, H160, U256};
use sp_runtime::{
	traits::{AccountIdConversion, BadOrigin, ConstU32, Convert, EnsureAdd, Hash, One, Zero},
	BoundedVec, DispatchError, FixedPointNumber, Perquintill, SaturatedConversion,
};
use staging_xcm::{
	v4::{Junction, Junction::*, Location, NetworkId},
	VersionedLocation,
};

use crate::{
	generic::{
		config::Runtime,
		env::{Blocks, Env},
		envs::fudge_env::{
			handle::{FudgeHandle, SIBLING_ID},
			FudgeEnv, FudgeSupport,
		},
		utils::{
			democracy::execute_via_democracy, genesis, genesis::Genesis,
			xcm::enable_para_to_sibling_communication,
		},
	},
	utils::{accounts::Keyring, orml_asset_registry},
};

/// The AUSD asset id
pub const AUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(3);
/// The USDT asset id
pub const USDT_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(1);

pub const AUSD_ED: Balance = 1_000_000_000;
pub const USDT_ED: Balance = 10_000;

pub const GLMR_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(4);
pub const GLMR_ED: Balance = 1_000_000;
pub const DEFAULT_BALANCE_GLMR: Balance = 10_000_000_000_000_000_000;
pub const POOL_ADMIN: Keyring = Keyring::Bob;
pub const POOL_ID: PoolId = 42;
pub const MOONBEAM_EVM_CHAIN_ID: u64 = 1284;
pub const DEFAULT_EVM_ADDRESS_MOONBEAM: [u8; 20] = [99; 20];
pub const DEFAULT_VALIDITY: Seconds = 2555583502;
pub const DOMAIN_MOONBEAM: Domain = Domain::EVM(MOONBEAM_EVM_CHAIN_ID);
pub const DEFAULT_DOMAIN_ADDRESS_MOONBEAM: DomainAddress =
	DomainAddress::EVM(MOONBEAM_EVM_CHAIN_ID, DEFAULT_EVM_ADDRESS_MOONBEAM);
pub const DEFAULT_OTHER_DOMAIN_ADDRESS: DomainAddress =
	DomainAddress::EVM(MOONBEAM_EVM_CHAIN_ID, [0; 20]);

pub type LiquidityPoolMessage = Message<Domain, PoolId, TrancheId, Balance, Quantity>;

mod utils {
	use cfg_types::oracles::OracleKey;
	use frame_support::weights::Weight;
	use runtime_common::oracle::Feeder;

	use super::*;

	/// Creates a new pool for the given id with
	///  * BOB as admin and depositor
	///  * Two tranches
	///  * AUSD as pool currency with max reserve 10k.
	pub fn create_ausd_pool<T: Runtime + FudgeSupport>(pool_id: u64) {
		create_currency_pool::<T>(pool_id, AUSD_CURRENCY_ID, decimals(currency_decimals::AUSD))
	}

	pub fn register_ausd<T: Runtime + FudgeSupport>() {
		let meta: AssetMetadata = AssetMetadata {
			decimals: 12,
			name: BoundedVec::default(),
			symbol: BoundedVec::default(),
			existential_deposit: 1_000_000_000,
			location: Some(VersionedLocation::V4(Location::new(
				1,
				[
					Parachain(SIBLING_ID),
					general_key(parachains::kusama::karura::AUSD_KEY),
				],
			))),
			additional: CustomMetadata {
				transferability: CrossChainTransferability::Xcm(Default::default()),
				pool_currency: true,
				..CustomMetadata::default()
			},
		};

		assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
			<T as frame_system::Config>::RuntimeOrigin::root(),
			meta,
			Some(AUSD_CURRENCY_ID)
		));
	}

	pub fn cfg(amount: Balance) -> Balance {
		amount * decimals(currency_decimals::NATIVE)
	}

	pub fn decimals(decimals: u32) -> Balance {
		10u128.saturating_pow(decimals)
	}

	pub fn set_domain_router_call<T: Runtime>(
		domain: Domain,
		router: DomainRouter<T>,
	) -> T::RuntimeCallExt {
		LiquidityPoolsGatewayCall::set_domain_router { domain, router }.into()
	}

	pub fn add_instance_call<T: Runtime>(instance: DomainAddress) -> T::RuntimeCallExt {
		LiquidityPoolsGatewayCall::add_instance { instance }.into()
	}

	pub fn remove_instance_call<T: Runtime>(instance: DomainAddress) -> T::RuntimeCallExt {
		LiquidityPoolsGatewayCall::remove_instance { instance }.into()
	}

	/// Creates a new pool for for the given id with the provided currency.
	///  * BOB as admin and depositor
	///  * Two tranches
	///  * The given `currency` as pool currency with of `currency_decimals`.
	pub fn create_currency_pool<T: Runtime + FudgeSupport>(
		pool_id: u64,
		currency_id: CurrencyId,
		currency_decimals: Balance,
	) {
		assert_ok!(pallet_pool_system::Pallet::<T>::create(
			POOL_ADMIN.into(),
			POOL_ADMIN.into(),
			pool_id,
			vec![
				TrancheInput {
					tranche_type: TrancheType::Residual,
					seniority: None,
					metadata:
						TrancheMetadata {
							// NOTE: For now, we have to set these metadata fields of the first
							// tranche to be convertible to the 32-byte size expected by the
							// liquidity pools AddTranche message.
							token_name: BoundedVec::<
								u8,
								<T as pallet_pool_system::Config>::StringLimit,
							>::try_from(
								"A highly advanced tranche".as_bytes().to_vec()
							)
							.expect("Can create BoundedVec for token name"),
							token_symbol: BoundedVec::<
								u8,
								<T as pallet_pool_system::Config>::StringLimit,
							>::try_from("TrNcH".as_bytes().to_vec())
							.expect("Can create BoundedVec for token symbol"),
						}
				},
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec: One::one(),
						min_risk_buffer: Perquintill::from_percent(10),
					},
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				}
			],
			currency_id,
			currency_decimals,
			// No pool fees per default
			vec![]
		));
	}

	pub fn register_glmr<T: Runtime + FudgeSupport>() {
		let meta: AssetMetadata = AssetMetadata {
			decimals: 18,
			name: BoundedVec::default(),
			symbol: BoundedVec::default(),
			existential_deposit: GLMR_ED,
			location: Some(VersionedLocation::V4(Location::new(
				1,
				[Parachain(SIBLING_ID), general_key(&[0, 1])],
			))),
			additional: CustomMetadata {
				transferability: CrossChainTransferability::Xcm(Default::default()),
				..CustomMetadata::default()
			},
		};

		assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
			<T as frame_system::Config>::RuntimeOrigin::root(),
			meta,
			Some(GLMR_CURRENCY_ID)
		));
	}

	pub fn set_test_domain_router<T: Runtime + FudgeSupport>(
		evm_chain_id: u64,
		xcm_domain_location: VersionedLocation,
		currency_id: CurrencyId,
	) {
		let ethereum_xcm_router = EthereumXCMRouter::<T> {
			router: XCMRouter {
				xcm_domain: XcmDomain {
					location: Box::new(xcm_domain_location),
					ethereum_xcm_transact_call_index: BoundedVec::truncate_from(vec![38, 0]),
					contract_address: H160::from(DEFAULT_EVM_ADDRESS_MOONBEAM),
					max_gas_limit: 500_000,
					transact_required_weight_at_most: Weight::from_parts(
						12530000000,
						DEFAULT_PROOF_SIZE.saturating_div(2),
					),
					overall_weight: Weight::from_parts(15530000000, DEFAULT_PROOF_SIZE),
					fee_currency: currency_id,
					// 0.2 token
					fee_amount: 200000000000000000,
				},
				_marker: Default::default(),
			},
			_marker: Default::default(),
		};

		let domain_router = DomainRouter::EthereumXCM(ethereum_xcm_router);
		let domain = Domain::EVM(evm_chain_id);

		assert_ok!(
			pallet_liquidity_pools_gateway::Pallet::<T>::set_domain_router(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				domain,
				domain_router,
			)
		);
	}

	pub fn default_tranche_id<T: Runtime + FudgeSupport>(pool_id: u64) -> TrancheId {
		let pool_details =
			pallet_pool_system::pallet::Pool::<T>::get(pool_id).expect("Pool should exist");
		pool_details
			.tranches
			.tranche_id(TrancheLoc::Index(0))
			.expect("Tranche at index 0 exists")
	}

	/// Returns a `VersionedLocation` that can be converted into
	/// `LiquidityPoolsWrappedToken` which is required for cross chain asset
	/// registration and transfer.
	pub fn liquidity_pools_transferable_multilocation<T: Runtime + FudgeSupport>(
		chain_id: u64,
		address: [u8; 20],
	) -> VersionedLocation {
		VersionedLocation::V4(Location::new(
			0,
			[
				PalletInstance(
					<T as frame_system::Config>::PalletInfo::index::<
						pallet_liquidity_pools::Pallet<T>,
					>()
					.expect("LiquidityPools should have pallet index")
					.saturated_into(),
				),
				GlobalConsensus(NetworkId::Ethereum { chain_id }),
				AccountKey20 {
					network: None,
					key: address,
				},
			],
		))
	}

	/// Enables `LiquidityPoolsTransferable` in the custom asset metadata
	/// for the given currency_id.
	///
	/// NOTE: Sets the location to the `MOONBEAM_EVM_CHAIN_ID` with dummy
	/// address as the location is required for LiquidityPoolsWrappedToken
	/// conversions.
	pub fn enable_liquidity_pool_transferability<T: Runtime + FudgeSupport>(
		currency_id: CurrencyId,
	) {
		let metadata = orml_asset_registry::Metadata::<T>::get(currency_id)
			.expect("Currency should be registered");
		let location = Some(Some(liquidity_pools_transferable_multilocation::<T>(
			MOONBEAM_EVM_CHAIN_ID,
			// Value of evm_address is irrelevant here
			[1u8; 20],
		)));

		assert_ok!(orml_asset_registry::Pallet::<T>::update_asset(
			<T as frame_system::Config>::RuntimeOrigin::root(),
			currency_id,
			None,
			None,
			None,
			None,
			location,
			Some(CustomMetadata {
				// Changed: Allow liquidity_pools transferability
				transferability: CrossChainTransferability::LiquidityPools,
				..metadata.additional
			})
		));
	}

	pub fn setup_test<T: Runtime + FudgeSupport>(env: &mut FudgeEnv<T>) {
		env.parachain_state_mut(|| {
			register_ausd::<T>();
			register_glmr::<T>();

			assert_ok!(orml_tokens::Pallet::<T>::set_balance(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				<T as pallet_liquidity_pools_gateway::Config>::Sender::get().into(),
				GLMR_CURRENCY_ID,
				DEFAULT_BALANCE_GLMR,
				0,
			));

			set_test_domain_router::<T>(
				MOONBEAM_EVM_CHAIN_ID,
				Location::new(1, Junction::Parachain(SIBLING_ID)).into(),
				GLMR_CURRENCY_ID,
			);
		});
	}

	/// Returns the derived general currency index.
	///
	/// Throws if the provided currency_id is not
	/// `CurrencyId::ForeignAsset(id)`.
	pub fn general_currency_index<T: Runtime + FudgeSupport>(currency_id: CurrencyId) -> u128 {
		pallet_liquidity_pools::Pallet::<T>::try_get_general_index(currency_id)
			.expect("ForeignAsset should convert into u128")
	}

	/// Returns the investment_id of the given pool and tranche ids.
	pub fn investment_id<T: Runtime + FudgeSupport>(
		pool_id: u64,
		tranche_id: TrancheId,
	) -> cfg_types::tokens::TrancheCurrency {
		<T as pallet_liquidity_pools::Config>::TrancheCurrency::generate(pool_id, tranche_id)
	}

	pub fn default_investment_id<T: Runtime + FudgeSupport>() -> cfg_types::tokens::TrancheCurrency
	{
		<T as pallet_liquidity_pools::Config>::TrancheCurrency::generate(
			POOL_ID,
			default_tranche_id::<T>(POOL_ID),
		)
	}

	pub fn default_order_id<T: Runtime + FudgeSupport>(investor: &AccountId) -> OrderId {
		let default_swap_id = (
			default_investment_id::<T>(),
			pallet_foreign_investments::Action::Investment,
		);
		pallet_swaps::Pallet::<T>::order_id(&investor, default_swap_id)
			.expect("Swap order exists; qed")
	}

	/// Returns the default investment account derived from the
	/// `DEFAULT_POOL_ID` and its default tranche.
	pub fn default_investment_account<T: Runtime + FudgeSupport>() -> AccountId {
		InvestmentAccount {
			investment_id: default_investment_id::<T>(),
		}
		.into_account_truncating()
	}

	pub fn fulfill_swap_into_pool<T: Runtime>(
		pool_id: u64,
		swap_order_id: u64,
		amount_pool: Balance,
		amount_foreign: Balance,
		trader: AccountId,
	) {
		let pool_currency: CurrencyId = pallet_pool_system::Pallet::<T>::currency_for(pool_id)
			.expect("Pool existence checked already");
		assert_ok!(orml_tokens::Pallet::<T>::mint_into(
			pool_currency,
			&trader,
			amount_pool
		));
		assert_ok!(pallet_order_book::Pallet::<T>::fill_order(
			RawOrigin::Signed(trader.clone()).into(),
			swap_order_id,
			amount_foreign
		));
	}

	/// Sets up required permissions for the investor and executes an
	/// initial investment via LiquidityPools by executing
	/// `IncreaseInvestOrder`.
	///
	/// Assumes `setup_pre_requirements` and
	/// `investments::create_currency_pool` to have been called
	/// beforehand
	pub fn do_initial_increase_investment<T: Runtime + FudgeSupport>(
		pool_id: u64,
		amount: Balance,
		investor: AccountId,
		currency_id: CurrencyId,
	) {
		let pool_currency: CurrencyId = pallet_pool_system::Pallet::<T>::currency_for(pool_id)
			.expect("Pool existence checked already");

		// Mock incoming increase invest message
		let msg = LiquidityPoolMessage::IncreaseInvestOrder {
			pool_id,
			tranche_id: default_tranche_id::<T>(pool_id),
			investor: investor.clone().into(),
			currency: general_currency_index::<T>(currency_id),
			amount,
		};

		// Should fail if investor does not have investor role yet
		// However, failure is async for foreign currencies as part of updating the
		// investment after the swap was fulfilled
		if currency_id == pool_currency {
			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::submit(
					DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
					msg.clone()
				),
				DispatchError::Other("Account does not have the TrancheInvestor permission.")
			);
		}

		// Make investor the MembersListAdmin of this Pool
		if !pallet_permissions::Pallet::<T>::has(
			PermissionScope::Pool(pool_id),
			investor.clone(),
			Role::PoolRole(PoolRole::TrancheInvestor(
				default_tranche_id::<T>(pool_id),
				DEFAULT_VALIDITY,
			)),
		) {
			crate::generic::utils::pool::give_role::<T>(
				investor.clone(),
				pool_id,
				PoolRole::TrancheInvestor(default_tranche_id::<T>(pool_id), DEFAULT_VALIDITY),
			);
		}

		let amount_before =
			orml_tokens::Pallet::<T>::balance(currency_id, &default_investment_account::<T>());
		let final_amount = amount_before
			.ensure_add(amount)
			.expect("Should not overflow when incrementing amount");

		// Execute byte message
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::submit(
			DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
			msg
		));

		if currency_id == pool_currency {
			// Verify investment was transferred into investment account
			assert_eq!(
				orml_tokens::Pallet::<T>::balance(currency_id, &default_investment_account::<T>()),
				final_amount
			);
			assert!(frame_system::Pallet::<T>::events().iter().any(|e| {
				e.event
					== pallet_investments::Event::<T>::InvestOrderUpdated {
						investment_id: default_investment_id::<T>(),
						submitted_at: 0,
						who: investor.clone(),
						amount: final_amount,
					}
					.into()
			}));
		}
	}

	/// Sets up required permissions for the investor and executes an
	/// initial redemption via LiquidityPools by executing
	/// `IncreaseRedeemOrder`.
	///
	/// Assumes `setup_pre_requirements` and
	/// `investments::create_currency_pool` to have been called
	/// beforehand.
	///
	/// NOTE: Mints exactly the redeeming amount of tranche tokens.
	pub fn do_initial_increase_redemption<T: Runtime + FudgeSupport>(
		pool_id: u64,
		amount: Balance,
		investor: AccountId,
		currency_id: CurrencyId,
	) {
		// Fund `DomainLocator` account of origination domain as redeemed tranche tokens
		// are transferred from this account instead of minting
		assert_ok!(orml_tokens::Pallet::<T>::mint_into(
			default_investment_id::<T>().into(),
			&Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain()),
			amount
		));

		// Verify redemption has not been made yet
		assert_eq!(
			orml_tokens::Pallet::<T>::balance(
				default_investment_id::<T>().into(),
				&default_investment_account::<T>(),
			),
			0
		);
		assert_eq!(
			orml_tokens::Pallet::<T>::balance(default_investment_id::<T>().into(), &investor),
			0
		);

		// Mock incoming increase invest message
		let msg = LiquidityPoolMessage::IncreaseRedeemOrder {
			pool_id,
			tranche_id: default_tranche_id::<T>(pool_id),
			investor: investor.clone().into(),
			currency: general_currency_index::<T>(currency_id),
			amount,
		};

		// Should fail if investor does not have investor role yet
		assert_noop!(
			pallet_liquidity_pools::Pallet::<T>::submit(
				DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				msg.clone()
			),
			DispatchError::Other("Account does not have the TrancheInvestor permission.")
		);

		// Make investor the MembersListAdmin of this Pool
		crate::generic::utils::pool::give_role::<T>(
			investor.clone(),
			pool_id,
			PoolRole::TrancheInvestor(default_tranche_id::<T>(pool_id), DEFAULT_VALIDITY),
		);

		assert_ok!(pallet_liquidity_pools::Pallet::<T>::submit(
			DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
			msg
		));

		// Verify redemption was transferred into investment account
		assert_eq!(
			orml_tokens::Pallet::<T>::balance(
				default_investment_id::<T>().into(),
				&default_investment_account::<T>(),
			),
			amount
		);
		assert_eq!(
			orml_tokens::Pallet::<T>::balance(default_investment_id::<T>().into(), &investor),
			0
		);
		assert_eq!(
			orml_tokens::Pallet::<T>::balance(
				default_investment_id::<T>().into(),
				&AccountConverter::convert(DEFAULT_OTHER_DOMAIN_ADDRESS)
			),
			0
		);
		assert_eq!(
			frame_system::Pallet::<T>::events()
				.iter()
				.last()
				.unwrap()
				.event,
			pallet_investments::Event::<T>::RedeemOrderUpdated {
				investment_id: default_investment_id::<T>(),
				submitted_at: 0,
				who: investor,
				amount
			}
			.into()
		);

		// Verify order id is 0
		assert_eq!(
			pallet_investments::Pallet::<T>::redeem_order_id(investment_id::<T>(
				pool_id,
				default_tranche_id::<T>(pool_id)
			)),
			0
		);
	}

	/// Register USDT in the asset registry and enable LiquidityPools cross
	/// chain transferability.
	///
	/// NOTE: Assumes to be executed within an externalities environment.
	fn register_usdt<T: Runtime + FudgeSupport>() {
		let meta: AssetMetadata = AssetMetadata {
			decimals: 6,
			name: BoundedVec::default(),
			symbol: BoundedVec::default(),
			existential_deposit: USDT_ED,
			location: Some(VersionedLocation::V4(Location::new(
				1,
				[Parachain(1000), PalletInstance(50), GeneralIndex(1984)],
			))),
			additional: CustomMetadata {
				transferability: CrossChainTransferability::LiquidityPools,
				pool_currency: true,
				..CustomMetadata::default()
			},
		};

		assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
			<T as frame_system::Config>::RuntimeOrigin::root(),
			meta,
			Some(USDT_CURRENCY_ID)
		));
	}

	/// Registers USDT currency, adds bidirectional trading pairs with
	/// conversion ratio one and returns the amount in foreign denomination.
	pub fn enable_usdt_trading<T: Runtime + FudgeSupport>(
		pool_currency: CurrencyId,
		amount_pool_denominated: Balance,
		enable_lp_transferability: bool,
		enable_foreign_to_pool_pair: bool,
		enable_pool_to_foreign_pair: bool,
	) -> Balance {
		register_usdt::<T>();
		let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
		let amount_foreign_denominated: u128 =
			IdentityPoolCurrencyConverter::<orml_asset_registry::Pallet<T>>::stable_to_stable(
				foreign_currency,
				pool_currency,
				amount_pool_denominated,
			)
			.unwrap();

		if enable_lp_transferability {
			enable_liquidity_pool_transferability::<T>(foreign_currency);
		}

		assert_ok!(pallet_order_book::Pallet::<T>::set_market_feeder(
			<T as frame_system::Config>::RuntimeOrigin::root(),
			Feeder::root(),
		));
		crate::generic::utils::oracle::update_feeders::<T>(
			POOL_ADMIN.id(),
			POOL_ID,
			[Feeder::root()],
		);

		if enable_foreign_to_pool_pair {
			crate::generic::utils::oracle::feed_from_root::<T>(
				OracleKey::ConversionRatio(foreign_currency, pool_currency),
				Ratio::one(),
			);
		}
		if enable_pool_to_foreign_pair {
			crate::generic::utils::oracle::feed_from_root::<T>(
				OracleKey::ConversionRatio(pool_currency, foreign_currency),
				Ratio::one(),
			);
		}

		amount_foreign_denominated
	}

	pub fn get_council_members() -> Vec<Keyring> {
		vec![Keyring::Alice, Keyring::Bob, Keyring::Charlie]
	}
}

use utils::*;

mod add_allow_upgrade {
	use cfg_types::tokens::LiquidityPoolsWrappedToken;

	use super::*;

	#[test_runtimes([development])]
	fn add_tranche<T: Runtime + FudgeSupport>() {
		let mut env = FudgeEnv::<T>::from_parachain_storage(
			Genesis::default()
				.add(genesis::balances::<T>(cfg(1_000)))
				.add(genesis::tokens::<T>(vec![(
					GLMR_CURRENCY_ID,
					DEFAULT_BALANCE_GLMR,
				)]))
				.storage(),
		);

		setup_test(&mut env);

		env.parachain_state_mut(|| {
			// Now create the pool
			let pool_id = POOL_ID;
			create_ausd_pool::<T>(pool_id);

			// Verify we can't call pallet_liquidity_pools::Pallet::<T>::add_tranche with a
			// non-existing tranche_id
			let nonexistent_tranche = [71u8; 16];

			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::add_tranche(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					pool_id,
					nonexistent_tranche,
					Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
				),
				pallet_liquidity_pools::Error::<T>::TrancheNotFound
			);
			let tranche_id = default_tranche_id::<T>(pool_id);

			// Verify ALICE can't call `add_tranche` given she is not the `PoolAdmin`
			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::add_tranche(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					pool_id,
					tranche_id,
					Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
				),
				pallet_liquidity_pools::Error::<T>::NotPoolAdmin
			);

			// Finally, verify we can call pallet_liquidity_pools::Pallet::<T>::add_tranche
			// successfully when called by the PoolAdmin with the right pool + tranche id
			// pair.
			assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_tranche(
				RawOrigin::Signed(POOL_ADMIN.into()).into(),
				pool_id,
				tranche_id,
				Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
			));

			// Edge case: Should throw if tranche exists but metadata does not exist
			let tranche_currency_id = CurrencyId::Tranche(pool_id, tranche_id);

			orml_asset_registry::Metadata::<T>::remove(tranche_currency_id);

			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::update_tranche_token_metadata(
					RawOrigin::Signed(POOL_ADMIN.into()).into(),
					pool_id,
					tranche_id,
					Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
				),
				pallet_liquidity_pools::Error::<T>::TrancheMetadataNotFound
			);
		});
	}

	#[test_runtimes([development])]
	fn update_member<T: Runtime + FudgeSupport>() {
		let mut env = FudgeEnv::<T>::from_parachain_storage(
			Genesis::default()
				.add(genesis::balances::<T>(cfg(1_000)))
				.add(genesis::tokens::<T>(vec![(
					GLMR_CURRENCY_ID,
					DEFAULT_BALANCE_GLMR,
				)]))
				.storage(),
		);

		setup_test(&mut env);

		env.parachain_state_mut(|| {
			// Now create the pool
			let pool_id = POOL_ID;

			create_ausd_pool::<T>(pool_id);

			let tranche_id = default_tranche_id::<T>(pool_id);

			// Finally, verify we can call pallet_liquidity_pools::Pallet::<T>::add_tranche
			// successfully when given a valid pool + tranche id pair.
			let new_member = DomainAddress::EVM(MOONBEAM_EVM_CHAIN_ID, [3; 20]);

			// Make ALICE the MembersListAdmin of this Pool
			assert_ok!(pallet_permissions::Pallet::<T>::add(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				Role::PoolRole(PoolRole::PoolAdmin),
				Keyring::Alice.into(),
				PermissionScope::Pool(pool_id),
				Role::PoolRole(PoolRole::InvestorAdmin),
			));

			// Verify it fails if the destination is not whitelisted yet
			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::update_member(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					pool_id,
					tranche_id,
					new_member.clone(),
					DEFAULT_VALIDITY,
				),
				pallet_liquidity_pools::Error::<T>::InvestorDomainAddressNotAMember,
			);

			// Whitelist destination as TrancheInvestor of this Pool
			crate::generic::utils::pool::give_role::<T>(
				AccountConverter::convert(new_member.clone()),
				pool_id,
				PoolRole::TrancheInvestor(default_tranche_id::<T>(pool_id), DEFAULT_VALIDITY),
			);

			// Verify the Investor role was set as expected in Permissions
			assert!(pallet_permissions::Pallet::<T>::has(
				PermissionScope::Pool(pool_id),
				AccountConverter::convert(new_member.clone()),
				Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, DEFAULT_VALIDITY)),
			));

			// Verify it now works
			assert_ok!(pallet_liquidity_pools::Pallet::<T>::update_member(
				RawOrigin::Signed(Keyring::Alice.into()).into(),
				pool_id,
				tranche_id,
				new_member,
				DEFAULT_VALIDITY,
			));

			// Verify it cannot be called for another member without whitelisting the domain
			// beforehand
			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::update_member(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					pool_id,
					tranche_id,
					DomainAddress::EVM(MOONBEAM_EVM_CHAIN_ID, [9; 20]),
					DEFAULT_VALIDITY,
				),
				pallet_liquidity_pools::Error::<T>::InvestorDomainAddressNotAMember,
			);
		});
	}

	#[test_runtimes([development])]
	fn update_token_price<T: Runtime + FudgeSupport>() {
		let mut env = FudgeEnv::<T>::from_parachain_storage(
			Genesis::default()
				.add(genesis::balances::<T>(cfg(1_000)))
				.add(genesis::tokens::<T>(vec![(
					GLMR_CURRENCY_ID,
					DEFAULT_BALANCE_GLMR,
				)]))
				.storage(),
		);

		setup_test(&mut env);

		env.parachain_state_mut(|| {
			let currency_id = AUSD_CURRENCY_ID;
			let pool_id = POOL_ID;

			enable_liquidity_pool_transferability::<T>(currency_id);

			create_ausd_pool::<T>(pool_id);

			assert_ok!(pallet_liquidity_pools::Pallet::<T>::update_token_price(
				RawOrigin::Signed(Keyring::Bob.into()).into(),
				pool_id,
				default_tranche_id::<T>(pool_id),
				currency_id,
				Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
			));
		});
	}

	#[test_runtimes([development])]
	fn add_currency<T: Runtime + FudgeSupport>() {
		let mut env = FudgeEnv::<T>::from_parachain_storage(
			Genesis::default()
				.add(genesis::balances::<T>(cfg(1_000)))
				.add(genesis::tokens::<T>(vec![(
					GLMR_CURRENCY_ID,
					DEFAULT_BALANCE_GLMR,
				)]))
				.storage(),
		);

		setup_test(&mut env);

		env.parachain_state_mut(|| {
			let gateway_sender = <T as pallet_liquidity_pools_gateway::Config>::Sender::get();

			let currency_id = AUSD_CURRENCY_ID;

			enable_liquidity_pool_transferability::<T>(currency_id);

			assert_eq!(
				orml_tokens::Pallet::<T>::free_balance(GLMR_CURRENCY_ID, &gateway_sender),
				DEFAULT_BALANCE_GLMR
			);

			assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_currency(
				RawOrigin::Signed(Keyring::Bob.into()).into(),
				currency_id,
			));

			let currency_index =
				pallet_liquidity_pools::Pallet::<T>::try_get_general_index(currency_id)
					.expect("can get general index for currency");

			let LiquidityPoolsWrappedToken::EVM {
				address: evm_address,
				..
			} = pallet_liquidity_pools::Pallet::<T>::try_get_wrapped_token(&currency_id)
				.expect("can get wrapped token");

			let outbound_message = pallet_liquidity_pools_gateway::OutboundMessageQueue::<T>::get(
				T::OutboundMessageNonce::one(),
			)
			.expect("expected outbound queue message");

			assert_eq!(
				outbound_message.2,
				Message::AddCurrency {
					currency: currency_index,
					evm_address,
				},
			);
		});
	}

	#[test_runtimes([development])]
	fn add_currency_should_fail<T: Runtime + FudgeSupport>() {
		let mut env = FudgeEnv::<T>::from_parachain_storage(
			Genesis::default()
				.add(genesis::balances::<T>(cfg(1_000)))
				.add(genesis::tokens::<T>(vec![(
					GLMR_CURRENCY_ID,
					DEFAULT_BALANCE_GLMR,
				)]))
				.storage(),
		);

		setup_test(&mut env);

		env.parachain_state_mut(|| {
			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::add_currency(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					CurrencyId::ForeignAsset(42)
				),
				pallet_liquidity_pools::Error::<T>::AssetNotFound
			);
			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::add_currency(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					CurrencyId::Native
				),
				pallet_liquidity_pools::Error::<T>::AssetNotFound
			);
			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::add_currency(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					CurrencyId::Staking(cfg_types::tokens::StakingCurrency::BlockRewards)
				),
				pallet_liquidity_pools::Error::<T>::AssetNotFound
			);
			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::add_currency(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					CurrencyId::Staking(cfg_types::tokens::StakingCurrency::BlockRewards)
				),
				pallet_liquidity_pools::Error::<T>::AssetNotFound
			);

			// Should fail to add currency_id which is missing a registered
			// Location
			let currency_id = CurrencyId::ForeignAsset(100);

			assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				AssetMetadata {
					name: BoundedVec::default(),
					symbol: BoundedVec::default(),
					decimals: 12,
					location: None,
					existential_deposit: 1_000_000,
					additional: CustomMetadata {
						transferability: CrossChainTransferability::LiquidityPools,
						mintable: false,
						permissioned: false,
						pool_currency: false,
						local_representation: None,
					},
				},
				Some(currency_id)
			));

			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::add_currency(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					currency_id
				),
				pallet_liquidity_pools::Error::<T>::AssetNotLiquidityPoolsWrappedToken
			);

			// Add convertable Location to metadata but remove transferability
			assert_ok!(orml_asset_registry::Pallet::<T>::update_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				currency_id,
				None,
				None,
				None,
				None,
				// Changed: Add multilocation to metadata for some random EVM chain id for
				// which no instance is registered
				Some(Some(liquidity_pools_transferable_multilocation::<T>(
					u64::MAX,
					[1u8; 20],
				))),
				Some(CustomMetadata {
					// Changed: Disallow liquidityPools transferability
					transferability: CrossChainTransferability::Xcm(Default::default()),
					..Default::default()
				}),
			));

			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::add_currency(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					currency_id
				),
				pallet_liquidity_pools::Error::<T>::AssetNotLiquidityPoolsTransferable
			);

			// Switch transferability from XCM to None
			assert_ok!(orml_asset_registry::Pallet::<T>::update_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				currency_id,
				None,
				None,
				None,
				None,
				None,
				Some(CustomMetadata {
					// Changed: Disallow cross chain transferability entirely
					transferability: CrossChainTransferability::None,
					..Default::default()
				})
			));

			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::add_currency(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					currency_id
				),
				pallet_liquidity_pools::Error::<T>::AssetNotLiquidityPoolsTransferable
			);
		});
	}

	#[test_runtimes([development])]
	fn allow_investment_currency<T: Runtime + FudgeSupport>() {
		let mut env = FudgeEnv::<T>::from_parachain_storage(
			Genesis::default()
				.add(genesis::balances::<T>(cfg(1_000)))
				.add(genesis::tokens::<T>(vec![(
					GLMR_CURRENCY_ID,
					DEFAULT_BALANCE_GLMR,
				)]))
				.storage(),
		);

		setup_test(&mut env);

		env.parachain_state_mut(|| {
			let currency_id = AUSD_CURRENCY_ID;
			let pool_id = POOL_ID;
			let evm_chain_id: u64 = MOONBEAM_EVM_CHAIN_ID;
			let evm_address = [1u8; 20];

			// Create an AUSD pool
			create_ausd_pool::<T>(pool_id);

			enable_liquidity_pool_transferability::<T>(currency_id);

			// Enable LiquidityPools transferability
			assert_ok!(orml_asset_registry::Pallet::<T>::update_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				currency_id,
				None,
				None,
				None,
				None,
				// Changed: Add location which can be converted to LiquidityPoolsWrappedToken
				Some(Some(liquidity_pools_transferable_multilocation::<T>(
					evm_chain_id,
					evm_address,
				))),
				Some(CustomMetadata {
					// Changed: Allow liquidity_pools transferability
					transferability: CrossChainTransferability::LiquidityPools,
					pool_currency: true,
					..Default::default()
				})
			));

			assert_ok!(
				pallet_liquidity_pools::Pallet::<T>::allow_investment_currency(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					pool_id,
					currency_id,
				)
			);

			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::allow_investment_currency(
					RawOrigin::Signed(Keyring::Charlie.into()).into(),
					pool_id,
					currency_id,
				),
				pallet_liquidity_pools::Error::<T>::NotPoolAdmin
			);
		});
	}

	#[test_runtimes([development])]
	fn allow_investment_currency_should_fail<T: Runtime + FudgeSupport>() {
		let mut env = FudgeEnv::<T>::from_parachain_storage(
			Genesis::default()
				.add(genesis::balances::<T>(cfg(1_000)))
				.add(genesis::tokens::<T>(vec![(
					GLMR_CURRENCY_ID,
					DEFAULT_BALANCE_GLMR,
				)]))
				.storage(),
		);

		setup_test(&mut env);

		env.parachain_state_mut(|| {
			let pool_id = POOL_ID;
			let currency_id = CurrencyId::ForeignAsset(42);
			let ausd_currency_id = AUSD_CURRENCY_ID;

			// Should fail if pool does not exist
			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::allow_investment_currency(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					pool_id,
					currency_id,
				),
				pallet_liquidity_pools::Error::<T>::NotPoolAdmin
			);

			// Register currency_id with pool_currency set to true
			assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				AssetMetadata {
					name: BoundedVec::default(),
					symbol: BoundedVec::default(),
					decimals: 12,
					location: None,
					existential_deposit: 1_000_000,
					additional: CustomMetadata {
						pool_currency: true,
						..Default::default()
					},
				},
				Some(currency_id)
			));

			// Create pool
			create_currency_pool::<T>(pool_id, currency_id, 10_000 * decimals(12));

			enable_liquidity_pool_transferability::<T>(ausd_currency_id);

			// Should fail if currency is not liquidityPools transferable
			assert_ok!(orml_asset_registry::Pallet::<T>::update_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				currency_id,
				None,
				None,
				None,
				None,
				None,
				Some(CustomMetadata {
					// Disallow any cross chain transferability
					transferability: CrossChainTransferability::None,
					// Changed: Allow to be usable as pool currency
					pool_currency: true,
					..Default::default()
				}),
			));
			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::allow_investment_currency(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					pool_id,
					currency_id,
				),
				pallet_liquidity_pools::Error::<T>::AssetNotLiquidityPoolsTransferable
			);

			// Should fail if currency does not have any Location in metadata
			assert_ok!(orml_asset_registry::Pallet::<T>::update_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				currency_id,
				None,
				None,
				None,
				None,
				None,
				Some(CustomMetadata {
					// Changed: Allow liquidityPools transferability
					transferability: CrossChainTransferability::LiquidityPools,
					// Still allow to be pool currency
					pool_currency: true,
					..Default::default()
				}),
			));
			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::allow_investment_currency(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					pool_id,
					currency_id,
				),
				pallet_liquidity_pools::Error::<T>::AssetNotLiquidityPoolsWrappedToken
			);

			// Should fail if currency does not have LiquidityPoolsWrappedToken location in
			// metadata
			assert_ok!(orml_asset_registry::Pallet::<T>::update_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				currency_id,
				None,
				None,
				None,
				None,
				// Changed: Add some location which cannot be converted to
				// LiquidityPoolsWrappedToken
				Some(Some(VersionedLocation::V4(Default::default()))),
				// No change for transferability required as it is already allowed for
				// LiquidityPools
				None,
			));
			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::allow_investment_currency(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					pool_id,
					currency_id,
				),
				pallet_liquidity_pools::Error::<T>::AssetNotLiquidityPoolsWrappedToken
			);
		});
	}

	#[test_runtimes([development])]
	fn disallow_investment_currency<T: Runtime + FudgeSupport>() {
		let mut env = FudgeEnv::<T>::from_parachain_storage(
			Genesis::default()
				.add(genesis::balances::<T>(cfg(1_000)))
				.add(genesis::tokens::<T>(vec![(
					GLMR_CURRENCY_ID,
					DEFAULT_BALANCE_GLMR,
				)]))
				.storage(),
		);

		setup_test(&mut env);

		env.parachain_state_mut(|| {
			let currency_id = AUSD_CURRENCY_ID;
			let pool_id = POOL_ID;
			let evm_chain_id: u64 = MOONBEAM_EVM_CHAIN_ID;
			let evm_address = [1u8; 20];

			// Create an AUSD pool
			create_ausd_pool::<T>(pool_id);

			enable_liquidity_pool_transferability::<T>(currency_id);

			// Enable LiquidityPools transferability
			assert_ok!(orml_asset_registry::Pallet::<T>::update_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				currency_id,
				None,
				None,
				None,
				None,
				// Changed: Add location which can be converted to LiquidityPoolsWrappedToken
				Some(Some(liquidity_pools_transferable_multilocation::<T>(
					evm_chain_id,
					evm_address,
				))),
				Some(CustomMetadata {
					// Changed: Allow liquidity_pools transferability
					transferability: CrossChainTransferability::LiquidityPools,
					pool_currency: true,
					..Default::default()
				})
			));

			assert_ok!(
				pallet_liquidity_pools::Pallet::<T>::disallow_investment_currency(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					pool_id,
					currency_id,
				)
			);

			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::disallow_investment_currency(
					RawOrigin::Signed(Keyring::Charlie.into()).into(),
					pool_id,
					currency_id,
				),
				pallet_liquidity_pools::Error::<T>::NotPoolAdmin
			);
		});
	}

	#[test_runtimes([development])]
	fn disallow_investment_currency_should_fail<T: Runtime + FudgeSupport>() {
		let mut env = FudgeEnv::<T>::from_parachain_storage(
			Genesis::default()
				.add(genesis::balances::<T>(cfg(1_000)))
				.add(genesis::tokens::<T>(vec![(
					GLMR_CURRENCY_ID,
					DEFAULT_BALANCE_GLMR,
				)]))
				.storage(),
		);

		setup_test(&mut env);

		env.parachain_state_mut(|| {
			let pool_id = POOL_ID;
			let currency_id = CurrencyId::ForeignAsset(42);
			let ausd_currency_id = AUSD_CURRENCY_ID;

			// Should fail if pool does not exist
			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::disallow_investment_currency(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					pool_id,
					currency_id,
				),
				pallet_liquidity_pools::Error::<T>::NotPoolAdmin
			);

			// Register currency_id with pool_currency set to true
			assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				AssetMetadata {
					name: BoundedVec::default(),
					symbol: BoundedVec::default(),
					decimals: 12,
					location: None,
					existential_deposit: 1_000_000,
					additional: CustomMetadata {
						pool_currency: true,
						..Default::default()
					},
				},
				Some(currency_id)
			));

			// Create pool
			create_currency_pool::<T>(pool_id, currency_id, 10_000 * decimals(12));

			enable_liquidity_pool_transferability::<T>(ausd_currency_id);

			// Should fail if currency is not liquidityPools transferable
			assert_ok!(orml_asset_registry::Pallet::<T>::update_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				currency_id,
				None,
				None,
				None,
				None,
				None,
				Some(CustomMetadata {
					// Disallow any cross chain transferability
					transferability: CrossChainTransferability::None,
					// Changed: Allow to be usable as pool currency
					pool_currency: true,
					..Default::default()
				}),
			));
			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::disallow_investment_currency(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					pool_id,
					currency_id,
				),
				pallet_liquidity_pools::Error::<T>::AssetNotLiquidityPoolsTransferable
			);

			// Should fail if currency does not have any Location in metadata
			assert_ok!(orml_asset_registry::Pallet::<T>::update_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				currency_id,
				None,
				None,
				None,
				None,
				None,
				Some(CustomMetadata {
					// Changed: Allow liquidityPools transferability
					transferability: CrossChainTransferability::LiquidityPools,
					// Still allow to be pool currency
					pool_currency: true,
					..Default::default()
				}),
			));
			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::disallow_investment_currency(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					pool_id,
					currency_id,
				),
				pallet_liquidity_pools::Error::<T>::AssetNotLiquidityPoolsWrappedToken
			);

			// Should fail if currency does not have LiquidityPoolsWrappedToken location in
			// metadata
			assert_ok!(orml_asset_registry::Pallet::<T>::update_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				currency_id,
				None,
				None,
				None,
				None,
				// Changed: Add some location which cannot be converted to
				// LiquidityPoolsWrappedToken
				Some(Some(VersionedLocation::V4(Default::default()))),
				// No change for transferability required as it is already allowed for
				// LiquidityPools
				None,
			));
			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::disallow_investment_currency(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					pool_id,
					currency_id,
				),
				pallet_liquidity_pools::Error::<T>::AssetNotLiquidityPoolsWrappedToken
			);
		});
	}

	#[test_runtimes([development])]
	fn schedule_upgrade<T: Runtime + FudgeSupport>() {
		let mut env = FudgeEnv::<T>::from_parachain_storage(
			Genesis::default()
				.add(genesis::balances::<T>(cfg(1_000)))
				.add(genesis::tokens::<T>(vec![(
					GLMR_CURRENCY_ID,
					DEFAULT_BALANCE_GLMR,
				)]))
				.storage(),
		);

		setup_test(&mut env);

		env.parachain_state_mut(|| {
			// Only Root can call `schedule_upgrade`
			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::schedule_upgrade(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					MOONBEAM_EVM_CHAIN_ID,
					[7; 20]
				),
				BadOrigin
			);

			// Now it finally works
			assert_ok!(pallet_liquidity_pools::Pallet::<T>::schedule_upgrade(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				MOONBEAM_EVM_CHAIN_ID,
				[7; 20]
			));
		});
	}

	#[test_runtimes([development])]
	fn cancel_upgrade<T: Runtime + FudgeSupport>() {
		let mut env = FudgeEnv::<T>::from_parachain_storage(
			Genesis::default()
				.add(genesis::balances::<T>(cfg(1_000)))
				.add(genesis::tokens::<T>(vec![(
					GLMR_CURRENCY_ID,
					DEFAULT_BALANCE_GLMR,
				)]))
				.storage(),
		);

		setup_test(&mut env);

		env.parachain_state_mut(|| {
			// Only Root can call `cancel_upgrade`
			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::cancel_upgrade(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					MOONBEAM_EVM_CHAIN_ID,
					[7; 20]
				),
				BadOrigin
			);

			// Now it finally works
			assert_ok!(pallet_liquidity_pools::Pallet::<T>::cancel_upgrade(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				MOONBEAM_EVM_CHAIN_ID,
				[7; 20]
			));
		});
	}

	#[test_runtimes([development])]
	fn update_tranche_token_metadata<T: Runtime + FudgeSupport>() {
		let mut env = FudgeEnv::<T>::from_parachain_storage(
			Genesis::default()
				.add(genesis::balances::<T>(cfg(1_000)))
				.add(genesis::tokens::<T>(vec![(
					GLMR_CURRENCY_ID,
					DEFAULT_BALANCE_GLMR,
				)]))
				.storage(),
		);

		setup_test(&mut env);

		env.parachain_state_mut(|| {
			let pool_id = POOL_ID;
			// NOTE: Default pool admin is BOB
			create_ausd_pool::<T>(pool_id);

			// Missing tranche token should throw
			let nonexistent_tranche = [71u8; 16];

			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::update_tranche_token_metadata(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					pool_id,
					nonexistent_tranche,
					Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
				),
				pallet_liquidity_pools::Error::<T>::TrancheNotFound
			);

			let tranche_id = default_tranche_id::<T>(pool_id);

			// Moving the update to another domain can be called by anyone
			assert_ok!(
				pallet_liquidity_pools::Pallet::<T>::update_tranche_token_metadata(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					pool_id,
					tranche_id,
					Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
				)
			);

			// Edge case: Should throw if tranche exists but metadata does not exist
			let tranche_currency_id = CurrencyId::Tranche(pool_id, tranche_id);

			orml_asset_registry::Metadata::<T>::remove(tranche_currency_id);

			assert_noop!(
				pallet_liquidity_pools::Pallet::<T>::update_tranche_token_metadata(
					RawOrigin::Signed(POOL_ADMIN.into()).into(),
					pool_id,
					tranche_id,
					Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
				),
				pallet_liquidity_pools::Error::<T>::TrancheMetadataNotFound
			);
		});
	}
}

mod foreign_investments {
	use super::*;

	mod same_currencies {
		use super::*;

		#[test_runtimes([development])]
		fn increase_invest_order<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.add(genesis::tokens::<T>(vec![(
						GLMR_CURRENCY_ID,
						DEFAULT_BALANCE_GLMR,
					)]))
					.storage(),
			);

			setup_test(&mut env);

			env.parachain_state_mut(|| {
				let pool_id = POOL_ID;
				let amount = 10 * decimals(12);
				let investor =
					AccountConverter::domain_account_to_account(DOMAIN_MOONBEAM, Keyring::Bob.id());
				let currency_id = AUSD_CURRENCY_ID;
				let currency_decimals = currency_decimals::AUSD;

				// Create new pool
				create_currency_pool::<T>(pool_id, currency_id, currency_decimals.into());

				// Set permissions and execute initial investment
				do_initial_increase_investment::<T>(pool_id, amount, investor.clone(), currency_id);

				// Verify the order was updated to the amount
				assert_eq!(
					pallet_investments::Pallet::<T>::acc_active_invest_order(
						default_investment_id::<T>(),
					)
					.amount,
					amount
				);

				// Increasing again should just bump invest_amount
				let msg = LiquidityPoolMessage::IncreaseInvestOrder {
					pool_id,
					tranche_id: default_tranche_id::<T>(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index::<T>(currency_id),
					amount,
				};
				assert_ok!(pallet_liquidity_pools::Pallet::<T>::submit(
					DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
					msg
				));
			});
		}

		#[test_runtimes([development])]
		fn decrease_invest_order<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.storage(),
			);

			setup_test(&mut env);

			env.parachain_state_mut(|| {
				let pool_id = POOL_ID;
				let invest_amount: u128 = 10 * decimals(12);
				let decrease_amount = invest_amount / 3;
				let final_amount = invest_amount - decrease_amount;
				let investor =
					AccountConverter::domain_account_to_account(DOMAIN_MOONBEAM, Keyring::Bob.id());
				let currency_id: CurrencyId = AUSD_CURRENCY_ID;
				let currency_decimals = currency_decimals::AUSD;

				// Create new pool
				create_currency_pool::<T>(pool_id, currency_id, currency_decimals.into());

				// Set permissions and execute initial investment
				do_initial_increase_investment::<T>(
					pool_id,
					invest_amount,
					investor.clone(),
					currency_id,
				);

				// Mock incoming decrease message
				let msg = LiquidityPoolMessage::DecreaseInvestOrder {
					pool_id,
					tranche_id: default_tranche_id::<T>(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index::<T>(currency_id),
					amount: decrease_amount,
				};

				// Expect failure if transferability is disabled since this is required for
				// preparing the `ExecutedDecreaseInvest` message.
				assert_noop!(
					pallet_liquidity_pools::Pallet::<T>::submit(
						DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
						msg.clone()
					),
					pallet_liquidity_pools::Error::<T>::AssetNotLiquidityPoolsTransferable
				);
				enable_liquidity_pool_transferability::<T>(currency_id);

				// Execute byte message
				assert_ok!(pallet_liquidity_pools::Pallet::<T>::submit(
					DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
					msg
				));

				// Verify investment was decreased into investment account
				assert_eq!(
					orml_tokens::Pallet::<T>::balance(
						currency_id,
						&default_investment_account::<T>()
					),
					final_amount
				);
				// Since the investment was done in the pool currency, the decrement happens
				// synchronously and thus it must be burned from investor's holdings
				assert_eq!(orml_tokens::Pallet::<T>::balance(currency_id, &investor), 0);
				assert!(frame_system::Pallet::<T>::events().iter().any(|e| e.event
					== pallet_investments::Event::<T>::InvestOrderUpdated {
						investment_id: default_investment_id::<T>(),
						submitted_at: 0,
						who: investor.clone(),
						amount: final_amount
					}
					.into()));
				assert!(frame_system::Pallet::<T>::events().iter().any(|e| e.event
					== orml_tokens::Event::<T>::Withdrawn {
						currency_id,
						who: investor.clone(),
						amount: decrease_amount
					}
					.into()));
				assert_eq!(
					pallet_investments::Pallet::<T>::acc_active_invest_order(
						default_investment_id::<T>(),
					)
					.amount,
					final_amount
				);
			});
		}

		#[test_runtimes([development])]
		fn cancel_invest_order<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.storage(),
			);

			setup_test(&mut env);

			env.parachain_state_mut(|| {
				let pool_id = POOL_ID;
				let invest_amount = 10 * decimals(12);
				let investor =
					AccountConverter::domain_account_to_account(DOMAIN_MOONBEAM, Keyring::Bob.id());
				let currency_id = AUSD_CURRENCY_ID;
				let currency_decimals = currency_decimals::AUSD;

				// Create new pool
				create_currency_pool::<T>(pool_id, currency_id, currency_decimals.into());

				// Set permissions and execute initial investment
				do_initial_increase_investment::<T>(
					pool_id,
					invest_amount,
					investor.clone(),
					currency_id,
				);

				// Verify investment account holds funds before cancelling
				assert_eq!(
					orml_tokens::Pallet::<T>::balance(
						currency_id,
						&default_investment_account::<T>()
					),
					invest_amount
				);

				// Mock incoming cancel message
				let msg = LiquidityPoolMessage::CancelInvestOrder {
					pool_id,
					tranche_id: default_tranche_id::<T>(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index::<T>(currency_id),
				};

				// Expect failure if transferability is disabled since this is required for
				// preparing the `ExecutedDecreaseInvest` message.
				assert_noop!(
					pallet_liquidity_pools::Pallet::<T>::submit(
						DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
						msg.clone()
					),
					pallet_liquidity_pools::Error::<T>::AssetNotLiquidityPoolsTransferable
				);

				enable_liquidity_pool_transferability::<T>(currency_id);

				// Execute byte message
				assert_ok!(pallet_liquidity_pools::Pallet::<T>::submit(
					DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
					msg
				));

				// Verify investment was entirely drained from investment account
				assert_eq!(
					orml_tokens::Pallet::<T>::balance(
						currency_id,
						&default_investment_account::<T>()
					),
					0
				);
				// Since the investment was done in the pool currency, the decrement happens
				// synchronously and thus it must be burned from investor's holdings
				assert_eq!(orml_tokens::Pallet::<T>::balance(currency_id, &investor), 0);
				assert!(frame_system::Pallet::<T>::events().iter().any(|e| e.event
					== pallet_investments::Event::<T>::InvestOrderUpdated {
						investment_id: default_investment_id::<T>(),
						submitted_at: 0,
						who: investor.clone(),
						amount: 0
					}
					.into()));
				assert!(frame_system::Pallet::<T>::events().iter().any(|e| e.event
					== orml_tokens::Event::<T>::Withdrawn {
						currency_id,
						who: investor.clone(),
						amount: invest_amount
					}
					.into()));
				assert_eq!(
					pallet_investments::Pallet::<T>::acc_active_invest_order(
						default_investment_id::<T>(),
					)
					.amount,
					0
				);
			});
		}

		#[test_runtimes([development])]
		fn collect_invest_order<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.storage(),
			);

			setup_test(&mut env);

			env.parachain_state_mut(|| {
				let pool_id = POOL_ID;
				let amount = 10 * decimals(12);
				let investor =
					AccountConverter::domain_account_to_account(DOMAIN_MOONBEAM, Keyring::Bob.id());
				let currency_id = AUSD_CURRENCY_ID;
				let currency_decimals = currency_decimals::AUSD;
				let sending_domain_locator =
					Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain());
				enable_liquidity_pool_transferability::<T>(currency_id);

				// Create new pool
				create_currency_pool::<T>(pool_id, currency_id, currency_decimals.into());
				let investment_currency_id: CurrencyId = default_investment_id::<T>().into();
				// Set permissions and execute initial investment
				do_initial_increase_investment::<T>(pool_id, amount, investor.clone(), currency_id);
				let events_before_collect = frame_system::Pallet::<T>::events();

				// Process and fulfill order
				// NOTE: Without this step, the order id is not cleared and
				// `Event::InvestCollectedForNonClearedOrderId` be dispatched
				assert_ok!(pallet_investments::Pallet::<T>::process_invest_orders(
					default_investment_id::<T>()
				));

				// Tranche tokens will be minted upon fulfillment
				assert_eq!(
					orml_tokens::Pallet::<T>::total_issuance(investment_currency_id),
					0
				);
				assert_ok!(pallet_investments::Pallet::<T>::invest_fulfillment(
					default_investment_id::<T>(),
					FulfillmentWithPrice {
						of_amount: Perquintill::one(),
						price: Ratio::one(),
					}
				));
				assert_eq!(
					orml_tokens::Pallet::<T>::total_issuance(investment_currency_id),
					amount
				);

				// Mock collection message msg
				let msg = LiquidityPoolMessage::CollectInvest {
					pool_id,
					tranche_id: default_tranche_id::<T>(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index::<T>(currency_id),
				};
				assert_ok!(pallet_liquidity_pools::Pallet::<T>::submit(
					DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
					msg
				));

				// Remove events before collect execution
				let events_since_collect: Vec<_> = frame_system::Pallet::<T>::events()
					.into_iter()
					.filter(|e| !events_before_collect.contains(e))
					.collect();

				// Verify investment was transferred to the domain locator
				assert_eq!(
					orml_tokens::Pallet::<T>::balance(
						default_investment_id::<T>().into(),
						&sending_domain_locator
					),
					amount
				);

				// Order should have been cleared by fulfilling investment
				assert_eq!(
					pallet_investments::Pallet::<T>::acc_active_invest_order(
						default_investment_id::<T>(),
					)
					.amount,
					0
				);
				assert!(!events_since_collect.iter().any(|e| {
					e.event
						== pallet_investments::Event::<T>::InvestCollectedForNonClearedOrderId {
							investment_id: default_investment_id::<T>(),
							who: investor.clone(),
						}
						.into()
				}));

				// Order should not have been updated since everything is collected
				assert!(!events_since_collect.iter().any(|e| {
					e.event
						== pallet_investments::Event::<T>::InvestOrderUpdated {
							investment_id: default_investment_id::<T>(),
							submitted_at: 0,
							who: investor.clone(),
							amount: 0,
						}
						.into()
				}));

				// Order should have been fully collected
				assert!(events_since_collect.iter().any(|e| {
					e.event
						== pallet_investments::Event::<T>::InvestOrdersCollected {
							investment_id: default_investment_id::<T>(),
							processed_orders: vec![0],
							who: investor.clone(),
							collection: InvestCollection::<Balance> {
								payout_investment_invest: amount,
								remaining_investment_invest: 0,
							},
							outcome: CollectOutcome::FullyCollected,
						}
						.into()
				}));

				let sender = <T as pallet_liquidity_pools_gateway::Config>::Sender::get();

				// Clearing of foreign InvestState should be dispatched
				assert!(frame_system::Pallet::<T>::events().iter().any(|e| {
					e.event
						== pallet_liquidity_pools_gateway::Event::<T>::OutboundMessageSubmitted {
							sender: sender.clone(),
							domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
							message: LiquidityPoolMessage::ExecutedCollectInvest {
								pool_id,
								tranche_id: default_tranche_id::<T>(pool_id),
								investor: investor.clone().into(),
								currency: general_currency_index::<T>(currency_id),
								currency_payout: amount,
								tranche_tokens_payout: amount,
								remaining_invest_amount: 0,
							},
						}
						.into()
				}));
			});
		}

		#[test_runtimes([development])]
		fn partially_collect_investment_for_through_investments<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.storage(),
			);

			setup_test(&mut env);

			env.parachain_state_mut(|| {
				let pool_id = POOL_ID;
				let invest_amount = 10 * decimals(12);
				let investor =
					AccountConverter::domain_account_to_account(DOMAIN_MOONBEAM, Keyring::Bob.id());
				let currency_id = AUSD_CURRENCY_ID;
				let currency_decimals = currency_decimals::AUSD;
				let sending_domain_locator =
					Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain());
				create_currency_pool::<T>(pool_id, currency_id, currency_decimals.into());
				do_initial_increase_investment::<T>(
					pool_id,
					invest_amount,
					investor.clone(),
					currency_id,
				);
				enable_liquidity_pool_transferability::<T>(currency_id);
				let investment_currency_id: CurrencyId = default_investment_id::<T>().into();

				assert!(
					!pallet_investments::Pallet::<T>::investment_requires_collect(
						&investor,
						default_investment_id::<T>()
					)
				);

				// Process 50% of investment at 25% rate, i.e. 1 pool currency = 4 tranche
				// tokens
				assert_ok!(pallet_investments::Pallet::<T>::process_invest_orders(
					default_investment_id::<T>()
				));
				assert_ok!(pallet_investments::Pallet::<T>::invest_fulfillment(
					default_investment_id::<T>(),
					FulfillmentWithPrice {
						of_amount: Perquintill::from_percent(50),
						price: Ratio::checked_from_rational(1, 4).unwrap(),
					}
				));

				// Pre collect assertions
				assert!(
					pallet_investments::Pallet::<T>::investment_requires_collect(
						&investor,
						default_investment_id::<T>()
					)
				);

				// Collecting through Investments should denote amounts and transition
				// state
				assert_ok!(pallet_investments::Pallet::<T>::collect_investments_for(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					investor.clone(),
					default_investment_id::<T>()
				));
				assert!(
					!pallet_investments::Pallet::<T>::investment_requires_collect(
						&investor,
						default_investment_id::<T>()
					)
				);

				// Tranche Tokens should still be transferred to collected to
				// domain locator account already
				assert_eq!(
					orml_tokens::Pallet::<T>::balance(investment_currency_id, &investor),
					0
				);
				assert_eq!(
					orml_tokens::Pallet::<T>::balance(
						investment_currency_id,
						&sending_domain_locator
					),
					invest_amount * 2
				);
				assert!(frame_system::Pallet::<T>::events().iter().any(|e| {
					e.event
						== pallet_investments::Event::<T>::InvestOrdersCollected {
							investment_id: default_investment_id::<T>(),
							processed_orders: vec![0],
							who: investor.clone(),
							collection: InvestCollection::<Balance> {
								payout_investment_invest: invest_amount * 2,
								remaining_investment_invest: invest_amount / 2,
							},
							outcome: CollectOutcome::FullyCollected,
						}
						.into()
				}));

				let sender = <T as pallet_liquidity_pools_gateway::Config>::Sender::get();

				assert!(frame_system::Pallet::<T>::events().iter().any(|e| {
					e.event
						== pallet_liquidity_pools_gateway::Event::<T>::OutboundMessageSubmitted {
							sender: sender.clone(),
							domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
							message: pallet_liquidity_pools::Message::ExecutedCollectInvest {
								pool_id,
								tranche_id: default_tranche_id::<T>(pool_id),
								investor: investor.clone().into(),
								currency: general_currency_index::<T>(currency_id),
								currency_payout: invest_amount / 2,
								tranche_tokens_payout: invest_amount * 2,
								remaining_invest_amount: invest_amount / 2,
							},
						}
						.into()
				}));

				// Process rest of investment at 50% rate (1 pool currency = 2 tranche tokens)
				assert_ok!(pallet_investments::Pallet::<T>::process_invest_orders(
					default_investment_id::<T>()
				));
				assert_ok!(pallet_investments::Pallet::<T>::invest_fulfillment(
					default_investment_id::<T>(),
					FulfillmentWithPrice {
						of_amount: Perquintill::one(),
						price: Ratio::checked_from_rational(1, 2).unwrap(),
					}
				));
				// Order should have been cleared by fulfilling investment
				assert_eq!(
					pallet_investments::Pallet::<T>::acc_active_invest_order(
						default_investment_id::<T>(),
					)
					.amount,
					0
				);
				assert_eq!(
					orml_tokens::Pallet::<T>::total_issuance(investment_currency_id),
					invest_amount * 3
				);

				// Collect remainder through Investments
				assert_ok!(pallet_investments::Pallet::<T>::collect_investments_for(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					investor.clone(),
					default_investment_id::<T>()
				));
				assert!(
					!pallet_investments::Pallet::<T>::investment_requires_collect(
						&investor,
						default_investment_id::<T>()
					)
				);

				// Tranche Tokens should be transferred to collected to
				// domain locator account already
				let amount_tranche_tokens = invest_amount * 3;
				assert_eq!(
					orml_tokens::Pallet::<T>::total_issuance(investment_currency_id),
					amount_tranche_tokens
				);
				assert!(
					orml_tokens::Pallet::<T>::balance(investment_currency_id, &investor).is_zero()
				);
				assert_eq!(
					orml_tokens::Pallet::<T>::balance(
						investment_currency_id,
						&sending_domain_locator
					),
					amount_tranche_tokens
				);
				assert!(!frame_system::Pallet::<T>::events().iter().any(|e| {
					e.event
						== pallet_investments::Event::<T>::InvestCollectedForNonClearedOrderId {
							investment_id: default_investment_id::<T>(),
							who: investor.clone(),
						}
						.into()
				}));
				assert!(frame_system::Pallet::<T>::events().iter().any(|e| {
					e.event
						== pallet_investments::Event::<T>::InvestOrdersCollected {
							investment_id: default_investment_id::<T>(),
							processed_orders: vec![1],
							who: investor.clone(),
							collection: InvestCollection::<Balance> {
								payout_investment_invest: invest_amount,
								remaining_investment_invest: 0,
							},
							outcome: CollectOutcome::FullyCollected,
						}
						.into()
				}));

				assert!(frame_system::Pallet::<T>::events().iter().any(|e| {
					e.event
						== pallet_liquidity_pools_gateway::Event::<T>::OutboundMessageSubmitted {
							sender: sender.clone(),
							domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
							message: LiquidityPoolMessage::ExecutedCollectInvest {
								pool_id,
								tranche_id: default_tranche_id::<T>(pool_id),
								investor: investor.clone().into(),
								currency: general_currency_index::<T>(currency_id),
								currency_payout: invest_amount / 2,
								tranche_tokens_payout: invest_amount,
								remaining_invest_amount: 0,
							},
						}
						.into()
				}));

				// Should fail to collect if `InvestmentState` does not
				// exist
				let msg = LiquidityPoolMessage::CollectInvest {
					pool_id,
					tranche_id: default_tranche_id::<T>(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index::<T>(currency_id),
				};
				assert_noop!(
					pallet_liquidity_pools::Pallet::<T>::submit(
						DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
						msg
					),
					pallet_foreign_investments::Error::<T>::InfoNotFound
				);
			});
		}

		#[test_runtimes([development])]
		fn increase_redeem_order<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.storage(),
			);

			setup_test(&mut env);

			env.parachain_state_mut(|| {
				let pool_id = POOL_ID;
				let amount = 10 * decimals(12);
				let investor =
					AccountConverter::domain_account_to_account(DOMAIN_MOONBEAM, Keyring::Bob.id());
				let currency_id = AUSD_CURRENCY_ID;
				let currency_decimals = currency_decimals::AUSD;

				// Create new pool
				create_currency_pool::<T>(pool_id, currency_id, currency_decimals.into());

				// Set permissions and execute initial redemption
				do_initial_increase_redemption::<T>(pool_id, amount, investor.clone(), currency_id);

				// Verify amount was noted in the corresponding order
				assert_eq!(
					pallet_investments::Pallet::<T>::acc_active_redeem_order(
						default_investment_id::<T>(),
					)
					.amount,
					amount
				);

				// Increasing again should just bump redeeming amount
				assert_ok!(orml_tokens::Pallet::<T>::mint_into(
					default_investment_id::<T>().into(),
					&Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain()),
					amount
				));
				let msg = LiquidityPoolMessage::IncreaseRedeemOrder {
					pool_id,
					tranche_id: default_tranche_id::<T>(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index::<T>(currency_id),
					amount,
				};
				assert_ok!(pallet_liquidity_pools::Pallet::<T>::submit(
					DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
					msg
				));
			});
		}

		#[test_runtimes([development])]
		fn decrease_redeem_order<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.storage(),
			);

			setup_test(&mut env);

			env.parachain_state_mut(|| {
				let pool_id = POOL_ID;
				let redeem_amount = 10 * decimals(12);
				let decrease_amount = redeem_amount / 3;
				let final_amount = redeem_amount - decrease_amount;
				let investor =
					AccountConverter::domain_account_to_account(DOMAIN_MOONBEAM, Keyring::Bob.id());
				let currency_id = AUSD_CURRENCY_ID;
				let currency_decimals = currency_decimals::AUSD;
				let sending_domain_locator =
					Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain());

				// Create new pool
				create_currency_pool::<T>(pool_id, currency_id, currency_decimals.into());

				// Set permissions and execute initial redemption
				do_initial_increase_redemption::<T>(
					pool_id,
					redeem_amount,
					investor.clone(),
					currency_id,
				);

				// Verify the corresponding redemption order id is 0
				assert_eq!(
					pallet_investments::Pallet::<T>::invest_order_id(investment_id::<T>(
						pool_id,
						default_tranche_id::<T>(pool_id)
					)),
					0
				);

				// Mock incoming decrease message
				let msg = LiquidityPoolMessage::DecreaseRedeemOrder {
					pool_id,
					tranche_id: default_tranche_id::<T>(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index::<T>(currency_id),
					amount: decrease_amount,
				};

				// Execute byte message
				assert_ok!(pallet_liquidity_pools::Pallet::<T>::submit(
					DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
					msg
				));

				// Verify investment was decreased into investment account
				assert_eq!(
					orml_tokens::Pallet::<T>::balance(
						default_investment_id::<T>().into(),
						&default_investment_account::<T>(),
					),
					final_amount
				);
				// Tokens should have been transferred from investor's wallet to domain's
				// sovereign account
				assert_eq!(
					orml_tokens::Pallet::<T>::balance(
						default_investment_id::<T>().into(),
						&investor
					),
					0
				);
				assert_eq!(
					orml_tokens::Pallet::<T>::balance(
						default_investment_id::<T>().into(),
						&sending_domain_locator
					),
					decrease_amount
				);

				// Order should have been updated
				assert!(frame_system::Pallet::<T>::events().iter().any(|e| e.event
					== pallet_investments::Event::<T>::RedeemOrderUpdated {
						investment_id: default_investment_id::<T>(),
						submitted_at: 0,
						who: investor.clone(),
						amount: final_amount
					}
					.into()));
				assert_eq!(
					pallet_investments::Pallet::<T>::acc_active_redeem_order(
						default_investment_id::<T>(),
					)
					.amount,
					final_amount
				);

				let sender = <T as pallet_liquidity_pools_gateway::Config>::Sender::get();

				assert!(frame_system::Pallet::<T>::events().iter().any(|e| {
					e.event
						== pallet_liquidity_pools_gateway::Event::<T>::OutboundMessageSubmitted {
							sender: sender.clone(),
							domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
							message: LiquidityPoolMessage::ExecutedDecreaseRedeemOrder {
								pool_id,
								tranche_id: default_tranche_id::<T>(pool_id),
								investor: investor.clone().into(),
								currency: general_currency_index::<T>(currency_id),
								tranche_tokens_payout: decrease_amount,
								remaining_redeem_amount: final_amount,
							},
						}
						.into()
				}));
			});
		}

		#[test_runtimes([development])]
		fn cancel_redeem_order<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.storage(),
			);

			setup_test(&mut env);

			env.parachain_state_mut(|| {
				let pool_id = POOL_ID;
				let redeem_amount = 10 * decimals(12);
				let investor =
					AccountConverter::domain_account_to_account(DOMAIN_MOONBEAM, Keyring::Bob.id());
				let currency_id = AUSD_CURRENCY_ID;
				let currency_decimals = currency_decimals::AUSD;
				let sending_domain_locator =
					Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain());

				// Create new pool
				create_currency_pool::<T>(pool_id, currency_id, currency_decimals.into());

				// Set permissions and execute initial redemption
				do_initial_increase_redemption::<T>(
					pool_id,
					redeem_amount,
					investor.clone(),
					currency_id,
				);

				// Verify the corresponding redemption order id is 0
				assert_eq!(
					pallet_investments::Pallet::<T>::invest_order_id(investment_id::<T>(
						pool_id,
						default_tranche_id::<T>(pool_id)
					)),
					0
				);

				// Mock incoming decrease message
				let msg = LiquidityPoolMessage::CancelRedeemOrder {
					pool_id,
					tranche_id: default_tranche_id::<T>(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index::<T>(currency_id),
				};

				// Execute byte message
				assert_ok!(pallet_liquidity_pools::Pallet::<T>::submit(
					DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
					msg
				));

				// Verify investment was decreased into investment account
				assert_eq!(
					orml_tokens::Pallet::<T>::balance(
						default_investment_id::<T>().into(),
						&default_investment_account::<T>(),
					),
					0
				);
				// Tokens should have been transferred from investor's wallet to domain's
				// sovereign account
				assert_eq!(
					orml_tokens::Pallet::<T>::balance(
						default_investment_id::<T>().into(),
						&investor
					),
					0
				);
				assert_eq!(
					orml_tokens::Pallet::<T>::balance(
						default_investment_id::<T>().into(),
						&sending_domain_locator
					),
					redeem_amount
				);

				// Order should have been updated
				assert!(frame_system::Pallet::<T>::events().iter().any(|e| e.event
					== pallet_investments::Event::<T>::RedeemOrderUpdated {
						investment_id: default_investment_id::<T>(),
						submitted_at: 0,
						who: investor.clone(),
						amount: 0
					}
					.into()));
				assert_eq!(
					pallet_investments::Pallet::<T>::acc_active_redeem_order(
						default_investment_id::<T>(),
					)
					.amount,
					0
				);
			});
		}

		#[test_runtimes([development])]
		fn fully_collect_redeem_order<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.storage(),
			);

			setup_test(&mut env);

			env.parachain_state_mut(|| {
				let pool_id = POOL_ID;
				let amount = 10 * decimals(12);
				let investor =
					AccountConverter::domain_account_to_account(DOMAIN_MOONBEAM, Keyring::Bob.id());
				let currency_id = AUSD_CURRENCY_ID;
				let currency_decimals = currency_decimals::AUSD;
				let pool_account = pallet_pool_system::pool_types::PoolLocator { pool_id }
					.into_account_truncating();

				// Create new pool
				create_currency_pool::<T>(pool_id, currency_id, currency_decimals.into());

				// Set permissions and execute initial investment
				do_initial_increase_redemption::<T>(pool_id, amount, investor.clone(), currency_id);
				let events_before_collect = frame_system::Pallet::<T>::events();

				// Fund the pool account with sufficient pool currency, else redemption cannot
				// swap tranche tokens against pool currency
				assert_ok!(orml_tokens::Pallet::<T>::mint_into(
					currency_id,
					&pool_account,
					amount
				));

				// Process and fulfill order
				// NOTE: Without this step, the order id is not cleared and
				// `Event::RedeemCollectedForNonClearedOrderId` be dispatched
				assert_ok!(pallet_investments::Pallet::<T>::process_redeem_orders(
					default_investment_id::<T>()
				));
				assert_ok!(pallet_investments::Pallet::<T>::redeem_fulfillment(
					default_investment_id::<T>(),
					FulfillmentWithPrice {
						of_amount: Perquintill::one(),
						price: Ratio::one(),
					}
				));

				// Enable liquidity pool transferability
				enable_liquidity_pool_transferability::<T>(currency_id);

				// Mock collection message msg
				let msg = LiquidityPoolMessage::CollectRedeem {
					pool_id,
					tranche_id: default_tranche_id::<T>(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index::<T>(currency_id),
				};

				// Execute byte message
				assert_ok!(pallet_liquidity_pools::Pallet::<T>::submit(
					DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
					msg
				));

				// Remove events before collect execution
				let events_since_collect: Vec<_> = frame_system::Pallet::<T>::events()
					.into_iter()
					.filter(|e| !events_before_collect.contains(e))
					.collect();

				// Verify collected redemption was burned from investor
				assert_eq!(orml_tokens::Pallet::<T>::balance(currency_id, &investor), 0);
				assert!(frame_system::Pallet::<T>::events().iter().any(|e| e.event
					== orml_tokens::Event::<T>::Withdrawn {
						currency_id,
						who: investor.clone(),
						amount
					}
					.into()));

				// Order should have been cleared by fulfilling redemption
				assert_eq!(
					pallet_investments::Pallet::<T>::acc_active_redeem_order(
						default_investment_id::<T>(),
					)
					.amount,
					0
				);
				assert!(!events_since_collect.iter().any(|e| {
					e.event
						== pallet_investments::Event::<T>::RedeemCollectedForNonClearedOrderId {
							investment_id: default_investment_id::<T>(),
							who: investor.clone(),
						}
						.into()
				}));

				// Order should not have been updated since everything is collected
				assert!(!events_since_collect.iter().any(|e| {
					e.event
						== pallet_investments::Event::<T>::RedeemOrderUpdated {
							investment_id: default_investment_id::<T>(),
							submitted_at: 0,
							who: investor.clone(),
							amount: 0,
						}
						.into()
				}));

				// Order should have been fully collected
				assert!(events_since_collect.iter().any(|e| {
					e.event
						== pallet_investments::Event::<T>::RedeemOrdersCollected {
							investment_id: default_investment_id::<T>(),
							processed_orders: vec![0],
							who: investor.clone(),
							collection: RedeemCollection::<Balance> {
								payout_investment_redeem: amount,
								remaining_investment_redeem: 0,
							},
							outcome: CollectOutcome::FullyCollected,
						}
						.into()
				}));

				let sender = <T as pallet_liquidity_pools_gateway::Config>::Sender::get();

				// Clearing of foreign RedeemState should be dispatched
				assert!(frame_system::Pallet::<T>::events().iter().any(|e| {
					e.event
						== pallet_liquidity_pools_gateway::Event::<T>::OutboundMessageSubmitted {
							sender: sender.clone(),
							domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
							message: LiquidityPoolMessage::ExecutedCollectRedeem {
								pool_id,
								tranche_id: default_tranche_id::<T>(pool_id),
								investor: investor.clone().into(),
								currency: general_currency_index::<T>(currency_id),
								currency_payout: amount,
								tranche_tokens_payout: amount,
								remaining_redeem_amount: 0,
							},
						}
						.into()
				}));
			});
		}

		#[test_runtimes([development])]
		fn partially_collect_redemption_for_through_investments<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.storage(),
			);

			setup_test(&mut env);

			env.parachain_state_mut(|| {
				let pool_id = POOL_ID;
				let redeem_amount = 10 * decimals(12);
				let investor =
					AccountConverter::domain_account_to_account(DOMAIN_MOONBEAM, Keyring::Bob.id());
				let currency_id = AUSD_CURRENCY_ID;
				let currency_decimals = currency_decimals::AUSD;
				let pool_account = pallet_pool_system::pool_types::PoolLocator { pool_id }
					.into_account_truncating();
				create_currency_pool::<T>(pool_id, currency_id, currency_decimals.into());
				do_initial_increase_redemption::<T>(
					pool_id,
					redeem_amount,
					investor.clone(),
					currency_id,
				);
				enable_liquidity_pool_transferability::<T>(currency_id);

				// Fund the pool account with sufficient pool currency, else redemption cannot
				// swap tranche tokens against pool currency
				assert_ok!(orml_tokens::Pallet::<T>::mint_into(
					currency_id,
					&pool_account,
					redeem_amount
				));
				assert!(
					!pallet_investments::Pallet::<T>::redemption_requires_collect(
						&investor,
						default_investment_id::<T>()
					)
				);

				// Process 50% of redemption at 25% rate, i.e. 1 pool currency = 4 tranche
				// tokens
				assert_ok!(pallet_investments::Pallet::<T>::process_redeem_orders(
					default_investment_id::<T>()
				));
				assert_ok!(pallet_investments::Pallet::<T>::redeem_fulfillment(
					default_investment_id::<T>(),
					FulfillmentWithPrice {
						of_amount: Perquintill::from_percent(50),
						price: Ratio::checked_from_rational(1, 4).unwrap(),
					}
				));

				// Pre collect assertions
				assert!(
					pallet_investments::Pallet::<T>::redemption_requires_collect(
						&investor,
						default_investment_id::<T>()
					)
				);

				// Collecting through investments should denote amounts and transition
				// state
				assert_ok!(pallet_investments::Pallet::<T>::collect_redemptions_for(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					investor.clone(),
					default_investment_id::<T>()
				));
				assert!(frame_system::Pallet::<T>::events().iter().any(|e| {
					e.event
						== pallet_investments::Event::<T>::RedeemOrdersCollected {
							investment_id: default_investment_id::<T>(),
							processed_orders: vec![0],
							who: investor.clone(),
							collection: RedeemCollection::<Balance> {
								payout_investment_redeem: redeem_amount / 8,
								remaining_investment_redeem: redeem_amount / 2,
							},
							outcome: CollectOutcome::FullyCollected,
						}
						.into()
				}));

				let sender = <T as pallet_liquidity_pools_gateway::Config>::Sender::get();

				assert!(frame_system::Pallet::<T>::events().iter().any(|e| {
					e.event
						== pallet_liquidity_pools_gateway::Event::<T>::OutboundMessageSubmitted {
							sender: sender.clone(),
							domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
							message: LiquidityPoolMessage::ExecutedCollectRedeem {
								pool_id,
								tranche_id: default_tranche_id::<T>(pool_id),
								investor: investor.clone().into(),
								currency: general_currency_index::<T>(currency_id),
								currency_payout: redeem_amount / 8,
								tranche_tokens_payout: redeem_amount / 2,
								remaining_redeem_amount: redeem_amount / 2,
							},
						}
						.into()
				}));
				assert!(
					!pallet_investments::Pallet::<T>::redemption_requires_collect(
						&investor,
						default_investment_id::<T>()
					)
				);
				// Since foreign currency is pool currency, the swap is immediately fulfilled
				// and ExecutedCollectRedeem dispatched
				assert!(frame_system::Pallet::<T>::events().iter().any(|e| e.event
					== orml_tokens::Event::<T>::Withdrawn {
						currency_id,
						who: investor.clone(),
						amount: redeem_amount / 8
					}
					.into()));

				// Process rest of redemption at 50% rate
				assert_ok!(pallet_investments::Pallet::<T>::process_redeem_orders(
					default_investment_id::<T>()
				));
				assert_ok!(pallet_investments::Pallet::<T>::redeem_fulfillment(
					default_investment_id::<T>(),
					FulfillmentWithPrice {
						of_amount: Perquintill::one(),
						price: Ratio::checked_from_rational(1, 2).unwrap(),
					}
				));
				// Order should have been cleared by fulfilling redemption
				assert_eq!(
					pallet_investments::Pallet::<T>::acc_active_redeem_order(
						default_investment_id::<T>(),
					)
					.amount,
					0
				);

				// Collect remainder through Investments
				assert_ok!(pallet_investments::Pallet::<T>::collect_redemptions_for(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					investor.clone(),
					default_investment_id::<T>()
				));
				assert!(
					!pallet_investments::Pallet::<T>::redemption_requires_collect(
						&investor,
						default_investment_id::<T>()
					)
				);
				assert!(!frame_system::Pallet::<T>::events().iter().any(|e| {
					e.event
						== pallet_investments::Event::<T>::RedeemCollectedForNonClearedOrderId {
							investment_id: default_investment_id::<T>(),
							who: investor.clone(),
						}
						.into()
				}));
				assert!(frame_system::Pallet::<T>::events().iter().any(|e| {
					e.event
						== pallet_investments::Event::<T>::RedeemOrdersCollected {
							investment_id: default_investment_id::<T>(),
							processed_orders: vec![1],
							who: investor.clone(),
							collection: RedeemCollection::<Balance> {
								payout_investment_redeem: redeem_amount / 4,
								remaining_investment_redeem: 0,
							},
							outcome: CollectOutcome::FullyCollected,
						}
						.into()
				}));
				// Verify collected redemption was burned from investor
				assert_eq!(orml_tokens::Pallet::<T>::balance(currency_id, &investor), 0);
				assert!(frame_system::Pallet::<T>::events().iter().any(|e| e.event
					== orml_tokens::Event::<T>::Withdrawn {
						currency_id,
						who: investor.clone(),
						amount: redeem_amount / 4
					}
					.into()));
				// Clearing of foreign RedeemState should have been dispatched exactly once
				assert!(frame_system::Pallet::<T>::events().iter().any(|e| {
					e.event
						== pallet_liquidity_pools_gateway::Event::<T>::OutboundMessageSubmitted {
							sender: sender.clone(),
							domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
							message: LiquidityPoolMessage::ExecutedCollectRedeem {
								pool_id,
								tranche_id: default_tranche_id::<T>(pool_id),
								investor: investor.clone().into(),
								currency: general_currency_index::<T>(currency_id),
								currency_payout: redeem_amount / 4,
								tranche_tokens_payout: redeem_amount / 2,
								remaining_redeem_amount: 0,
							},
						}
						.into()
				}));
			});
		}

		mod should_fail {
			use super::*;

			mod decrease_should_underflow {
				use super::*;

				#[test_runtimes([development])]
				fn invest_decrease_underflow<T: Runtime + FudgeSupport>() {
					let mut env = FudgeEnv::<T>::from_parachain_storage(
						Genesis::default()
							.add(genesis::balances::<T>(cfg(1_000)))
							.storage(),
					);

					setup_test(&mut env);

					env.parachain_state_mut(|| {
						let pool_id = POOL_ID;
						let invest_amount: u128 = 10 * decimals(12);
						let decrease_amount = invest_amount + 1;
						let investor = AccountConverter::domain_account_to_account(
							DOMAIN_MOONBEAM,
							Keyring::Bob.id(),
						);
						let currency_id: CurrencyId = AUSD_CURRENCY_ID;
						let currency_decimals = currency_decimals::AUSD;
						create_currency_pool::<T>(pool_id, currency_id, currency_decimals.into());
						do_initial_increase_investment::<T>(
							pool_id,
							invest_amount,
							investor.clone(),
							currency_id,
						);
						enable_liquidity_pool_transferability::<T>(currency_id);

						// Mock incoming decrease message
						let msg = LiquidityPoolMessage::DecreaseInvestOrder {
							pool_id,
							tranche_id: default_tranche_id::<T>(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index::<T>(currency_id),
							amount: decrease_amount,
						};

						assert_noop!(
							pallet_liquidity_pools::Pallet::<T>::submit(
								DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
								msg
							),
							pallet_foreign_investments::Error::<T>::TooMuchDecrease
						);
					});
				}

				#[test_runtimes([development])]
				fn redeem_decrease_underflow<T: Runtime + FudgeSupport>() {
					let mut env = FudgeEnv::<T>::from_parachain_storage(
						Genesis::default()
							.add(genesis::balances::<T>(cfg(1_000)))
							.storage(),
					);

					setup_test(&mut env);

					env.parachain_state_mut(|| {
						let pool_id = POOL_ID;
						let redeem_amount: u128 = 10 * decimals(12);
						let decrease_amount = redeem_amount + 1;
						let investor = AccountConverter::domain_account_to_account(
							DOMAIN_MOONBEAM,
							Keyring::Bob.id(),
						);
						let currency_id: CurrencyId = AUSD_CURRENCY_ID;
						let currency_decimals = currency_decimals::AUSD;
						create_currency_pool::<T>(pool_id, currency_id, currency_decimals.into());
						do_initial_increase_redemption::<T>(
							pool_id,
							redeem_amount,
							investor.clone(),
							currency_id,
						);

						// Mock incoming decrease message
						let msg = LiquidityPoolMessage::DecreaseRedeemOrder {
							pool_id,
							tranche_id: default_tranche_id::<T>(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index::<T>(currency_id),
							amount: decrease_amount,
						};

						assert_noop!(
							pallet_liquidity_pools::Pallet::<T>::submit(
								DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
								msg
							),
							DispatchError::Arithmetic(sp_runtime::ArithmeticError::Underflow)
						);
					});
				}
			}

			mod should_throw_requires_collect {
				use super::*;

				#[test_runtimes([development])]
				fn invest_requires_collect<T: Runtime + FudgeSupport>() {
					let mut env = FudgeEnv::<T>::from_parachain_storage(
						Genesis::default()
							.add(genesis::balances::<T>(cfg(1_000)))
							.storage(),
					);

					setup_test(&mut env);

					env.parachain_state_mut(|| {
						let pool_id = POOL_ID;
						let amount: u128 = 10 * decimals(12);
						let investor = AccountConverter::domain_account_to_account(
							DOMAIN_MOONBEAM,
							Keyring::Bob.id(),
						);
						let currency_id: CurrencyId = AUSD_CURRENCY_ID;
						let currency_decimals = currency_decimals::AUSD;
						create_currency_pool::<T>(pool_id, currency_id, currency_decimals.into());
						do_initial_increase_investment::<T>(
							pool_id,
							amount,
							investor.clone(),
							currency_id,
						);
						enable_liquidity_pool_transferability::<T>(currency_id);

						// Prepare collection
						let pool_account = pallet_pool_system::pool_types::PoolLocator { pool_id }
							.into_account_truncating();
						assert_ok!(orml_tokens::Pallet::<T>::mint_into(
							currency_id,
							&pool_account,
							amount
						));
						assert_ok!(pallet_investments::Pallet::<T>::process_invest_orders(
							default_investment_id::<T>()
						));
						assert_ok!(pallet_investments::Pallet::<T>::invest_fulfillment(
							default_investment_id::<T>(),
							FulfillmentWithPrice {
								of_amount: Perquintill::one(),
								price: Ratio::one(),
							}
						));

						// Should fail to increase
						let increase_msg = LiquidityPoolMessage::IncreaseInvestOrder {
							pool_id,
							tranche_id: default_tranche_id::<T>(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index::<T>(currency_id),
							amount: AUSD_ED,
						};
						assert_noop!(
							pallet_liquidity_pools::Pallet::<T>::submit(
								DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
								increase_msg
							),
							pallet_investments::Error::<T>::CollectRequired
						);

						// Should fail to decrease
						let decrease_msg = LiquidityPoolMessage::DecreaseInvestOrder {
							pool_id,
							tranche_id: default_tranche_id::<T>(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index::<T>(currency_id),
							amount: 1,
						};
						assert_noop!(
							pallet_liquidity_pools::Pallet::<T>::submit(
								DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
								decrease_msg
							),
							pallet_investments::Error::<T>::CollectRequired
						);
					});
				}

				#[test_runtimes([development])]
				fn redeem_requires_collect<T: Runtime + FudgeSupport>() {
					let mut env = FudgeEnv::<T>::from_parachain_storage(
						Genesis::default()
							.add(genesis::balances::<T>(cfg(1_000)))
							.storage(),
					);

					setup_test(&mut env);

					env.parachain_state_mut(|| {
						let pool_id = POOL_ID;
						let amount: u128 = 10 * decimals(12);
						let investor = AccountConverter::domain_account_to_account(
							DOMAIN_MOONBEAM,
							Keyring::Bob.id(),
						);
						let currency_id: CurrencyId = AUSD_CURRENCY_ID;
						let currency_decimals = currency_decimals::AUSD;
						create_currency_pool::<T>(pool_id, currency_id, currency_decimals.into());
						do_initial_increase_redemption::<T>(
							pool_id,
							amount,
							investor.clone(),
							currency_id,
						);
						enable_liquidity_pool_transferability::<T>(currency_id);

						// Mint more into DomainLocator required for subsequent invest attempt
						assert_ok!(orml_tokens::Pallet::<T>::mint_into(
							default_investment_id::<T>().into(),
							&Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain()),
							1,
						));

						// Prepare collection
						let pool_account = pallet_pool_system::pool_types::PoolLocator { pool_id }
							.into_account_truncating();
						assert_ok!(orml_tokens::Pallet::<T>::mint_into(
							currency_id,
							&pool_account,
							amount
						));
						assert_ok!(pallet_investments::Pallet::<T>::process_redeem_orders(
							default_investment_id::<T>()
						));
						assert_ok!(pallet_investments::Pallet::<T>::redeem_fulfillment(
							default_investment_id::<T>(),
							FulfillmentWithPrice {
								of_amount: Perquintill::one(),
								price: Ratio::one(),
							}
						));

						// Should fail to increase
						let increase_msg = LiquidityPoolMessage::IncreaseRedeemOrder {
							pool_id,
							tranche_id: default_tranche_id::<T>(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index::<T>(currency_id),
							amount: 1,
						};
						assert_noop!(
							pallet_liquidity_pools::Pallet::<T>::submit(
								DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
								increase_msg
							),
							pallet_investments::Error::<T>::CollectRequired
						);

						// Should fail to decrease
						let decrease_msg = LiquidityPoolMessage::DecreaseRedeemOrder {
							pool_id,
							tranche_id: default_tranche_id::<T>(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index::<T>(currency_id),
							amount: 1,
						};
						assert_noop!(
							pallet_liquidity_pools::Pallet::<T>::submit(
								DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
								decrease_msg
							),
							pallet_investments::Error::<T>::CollectRequired
						);
					});
				}
			}

			mod payment_payout_currency {
				use super::*;

				#[test_runtimes([development])]
				fn invalid_invest_payment_currency<T: Runtime + FudgeSupport>() {
					let mut env = FudgeEnv::<T>::from_parachain_storage(
						Genesis::default()
							.add(genesis::balances::<T>(cfg(1_000)))
							.storage(),
					);

					setup_test(&mut env);

					env.parachain_state_mut(|| {
						let pool_id = POOL_ID;
						let investor = AccountConverter::domain_account_to_account(
							DOMAIN_MOONBEAM,
							Keyring::Bob.id(),
						);
						let pool_currency = AUSD_CURRENCY_ID;
						let currency_decimals = currency_decimals::AUSD;
						let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
						let amount = 6 * decimals(18);

						create_currency_pool::<T>(pool_id, pool_currency, currency_decimals.into());
						do_initial_increase_investment::<T>(
							pool_id,
							amount,
							investor.clone(),
							pool_currency,
						);

						enable_usdt_trading::<T>(pool_currency, amount, true, true, true);

						// Should fail to increase, decrease or collect for
						// another foreign currency as long as
						// `InvestmentState` exists
						let increase_msg = LiquidityPoolMessage::IncreaseInvestOrder {
							pool_id,
							tranche_id: default_tranche_id::<T>(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index::<T>(foreign_currency),
							amount: AUSD_ED,
						};
						assert_noop!(
							pallet_liquidity_pools::Pallet::<T>::submit(
								DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
								increase_msg
							),
							pallet_foreign_investments::Error::<T>::MismatchedForeignCurrency
						);
						let decrease_msg = LiquidityPoolMessage::DecreaseInvestOrder {
							pool_id,
							tranche_id: default_tranche_id::<T>(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index::<T>(foreign_currency),
							amount: 1,
						};
						assert_noop!(
							pallet_liquidity_pools::Pallet::<T>::submit(
								DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
								decrease_msg
							),
							pallet_foreign_investments::Error::<T>::MismatchedForeignCurrency
						);
						let collect_msg = LiquidityPoolMessage::CollectInvest {
							pool_id,
							tranche_id: default_tranche_id::<T>(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index::<T>(foreign_currency),
						};
						assert_noop!(
							pallet_liquidity_pools::Pallet::<T>::submit(
								DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
								collect_msg
							),
							pallet_foreign_investments::Error::<T>::MismatchedForeignCurrency
						);
					});
				}

				#[test_runtimes([development])]
				fn invalid_redeem_payout_currency<T: Runtime + FudgeSupport>() {
					let mut env = FudgeEnv::<T>::from_parachain_storage(
						Genesis::default()
							.add(genesis::balances::<T>(cfg(1_000)))
							.storage(),
					);

					setup_test(&mut env);

					env.parachain_state_mut(|| {
						let pool_id = POOL_ID;
						let investor = AccountConverter::domain_account_to_account(
							DOMAIN_MOONBEAM,
							Keyring::Bob.id(),
						);
						let pool_currency = AUSD_CURRENCY_ID;
						let currency_decimals = currency_decimals::AUSD;
						let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
						let amount = 6 * decimals(18);

						create_currency_pool::<T>(pool_id, pool_currency, currency_decimals.into());
						do_initial_increase_redemption::<T>(
							pool_id,
							amount,
							investor.clone(),
							pool_currency,
						);
						enable_usdt_trading::<T>(pool_currency, amount, true, true, true);
						assert_ok!(orml_tokens::Pallet::<T>::mint_into(
							default_investment_id::<T>().into(),
							&Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain()),
							amount,
						));

						// Should fail to increase, decrease or collect for
						// another foreign currency as long as
						// `RedemptionState` exists
						let increase_msg = LiquidityPoolMessage::IncreaseRedeemOrder {
							pool_id,
							tranche_id: default_tranche_id::<T>(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index::<T>(foreign_currency),
							amount: 1,
						};
						assert_noop!(
							pallet_liquidity_pools::Pallet::<T>::submit(
								DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
								increase_msg
							),
							pallet_foreign_investments::Error::<T>::MismatchedForeignCurrency
						);
						let decrease_msg = LiquidityPoolMessage::DecreaseRedeemOrder {
							pool_id,
							tranche_id: default_tranche_id::<T>(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index::<T>(foreign_currency),
							amount: 1,
						};
						assert_noop!(
							pallet_liquidity_pools::Pallet::<T>::submit(
								DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
								decrease_msg
							),
							pallet_foreign_investments::Error::<T>::MismatchedForeignCurrency
						);
						let collect_msg = LiquidityPoolMessage::CollectRedeem {
							pool_id,
							tranche_id: default_tranche_id::<T>(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index::<T>(foreign_currency),
						};
						assert_noop!(
							pallet_liquidity_pools::Pallet::<T>::submit(
								DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
								collect_msg
							),
							pallet_foreign_investments::Error::<T>::MismatchedForeignCurrency
						);
					});
				}

				#[test_runtimes([development])]
				fn redeem_payout_currency_not_found<T: Runtime + FudgeSupport>() {
					let mut env = FudgeEnv::<T>::from_parachain_storage(
						Genesis::default()
							.add(genesis::balances::<T>(cfg(1_000)))
							.storage(),
					);

					setup_test(&mut env);

					env.parachain_state_mut(|| {
						let pool_id = POOL_ID;
						let investor = AccountConverter::domain_account_to_account(
							DOMAIN_MOONBEAM,
							Keyring::Bob.id(),
						);
						let pool_currency = AUSD_CURRENCY_ID;
						let currency_decimals = currency_decimals::AUSD;
						let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
						let amount = 6 * decimals(18);

						create_currency_pool::<T>(pool_id, pool_currency, currency_decimals.into());
						do_initial_increase_redemption::<T>(
							pool_id,
							amount,
							investor.clone(),
							pool_currency,
						);
						enable_usdt_trading::<T>(pool_currency, amount, true, true, true);
						assert_ok!(orml_tokens::Pallet::<T>::mint_into(
							default_investment_id::<T>().into(),
							&Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain()),
							amount,
						));

						// Should fail to decrease or collect for another
						// foreign currency as long as `RedemptionState`
						// exists
						let decrease_msg = LiquidityPoolMessage::DecreaseRedeemOrder {
							pool_id,
							tranche_id: default_tranche_id::<T>(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index::<T>(foreign_currency),
							amount: 1,
						};
						assert_noop!(
							pallet_liquidity_pools::Pallet::<T>::submit(
								DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
								decrease_msg
							),
							pallet_foreign_investments::Error::<T>::MismatchedForeignCurrency
						);

						let collect_msg = LiquidityPoolMessage::CollectRedeem {
							pool_id,
							tranche_id: default_tranche_id::<T>(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index::<T>(foreign_currency),
						};
						assert_noop!(
							pallet_liquidity_pools::Pallet::<T>::submit(
								DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
								collect_msg
							),
							pallet_foreign_investments::Error::<T>::MismatchedForeignCurrency
						);
					});
				}
			}
		}
	}

	mod mismatching_currencies {
		use super::*;

		#[test_runtimes([development])]
		fn collect_foreign_investment_for<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.storage(),
			);

			setup_test(&mut env);

			env.parachain_state_mut(|| {
				let pool_id = POOL_ID;
				let investor =
					AccountConverter::domain_account_to_account(DOMAIN_MOONBEAM, Keyring::Bob.id());
				let pool_currency: CurrencyId = AUSD_CURRENCY_ID;
				let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
				let pool_currency_decimals = currency_decimals::AUSD;
				let invest_amount_pool_denominated: u128 = 6 * decimals(18);
				let sending_domain_locator =
					Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain());
				let trader: AccountId = Keyring::Alice.into();
				create_currency_pool::<T>(pool_id, pool_currency, pool_currency_decimals.into());

				// USDT investment preparations
				let invest_amount_foreign_denominated = enable_usdt_trading::<T>(
					pool_currency,
					invest_amount_pool_denominated,
					true,
					true,
					// not needed because we don't initialize a swap from pool to foreign here
					false,
				);

				// Do first investment and fulfill swap order
				do_initial_increase_investment::<T>(
					pool_id,
					invest_amount_foreign_denominated,
					investor.clone(),
					foreign_currency,
				);
				fulfill_swap_into_pool::<T>(
					pool_id,
					default_order_id::<T>(&investor),
					invest_amount_pool_denominated,
					invest_amount_foreign_denominated,
					trader,
				);

				// Increase invest order to initialize ForeignInvestmentInfo
				let msg = LiquidityPoolMessage::IncreaseInvestOrder {
					pool_id,
					tranche_id: default_tranche_id::<T>(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index::<T>(foreign_currency),
					amount: invest_amount_foreign_denominated,
				};
				assert_ok!(pallet_liquidity_pools::Pallet::<T>::submit(
					DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
					msg
				));

				// Process 100% of investment at 50% rate (1 pool currency = 2 tranche tokens)
				assert_ok!(pallet_investments::Pallet::<T>::process_invest_orders(
					default_investment_id::<T>()
				));
				assert_ok!(pallet_investments::Pallet::<T>::invest_fulfillment(
					default_investment_id::<T>(),
					FulfillmentWithPrice {
						of_amount: Perquintill::one(),
						price: Ratio::checked_from_rational(1, 2).unwrap(),
					}
				));
				assert_ok!(pallet_investments::Pallet::<T>::collect_investments_for(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					investor.clone(),
					default_investment_id::<T>()
				));
				assert!(orml_tokens::Pallet::<T>::balance(
					default_investment_id::<T>().into(),
					&investor
				)
				.is_zero());
				assert_eq!(
					orml_tokens::Pallet::<T>::balance(
						default_investment_id::<T>().into(),
						&sending_domain_locator
					),
					invest_amount_pool_denominated * 2
				);

				let sender = <T as pallet_liquidity_pools_gateway::Config>::Sender::get();

				assert!(frame_system::Pallet::<T>::events().iter().any(|e| {
					e.event
						== pallet_liquidity_pools_gateway::Event::<T>::OutboundMessageSubmitted {
							sender: sender.clone(),
							domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
							message: LiquidityPoolMessage::ExecutedCollectInvest {
								pool_id,
								tranche_id: default_tranche_id::<T>(pool_id),
								investor: investor.clone().into(),
								currency: general_currency_index::<T>(foreign_currency),
								currency_payout: invest_amount_foreign_denominated,
								tranche_tokens_payout: 2 * invest_amount_pool_denominated,
								remaining_invest_amount: invest_amount_foreign_denominated,
							},
						}
						.into()
				}));
			});
		}

		/// Invest in pool currency, then increase in allowed foreign
		/// currency, then decrease in same foreign currency multiple times.
		#[test_runtimes([development])]
		fn increase_fulfill_increase_decrease_decrease_partial<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.storage(),
			);

			setup_test(&mut env);

			env.parachain_state_mut(|| {
				let pool_id = POOL_ID;
				let investor =
					AccountConverter::domain_account_to_account(DOMAIN_MOONBEAM, Keyring::Bob.id());
				let pool_currency: CurrencyId = AUSD_CURRENCY_ID;
				let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
				let pool_currency_decimals = currency_decimals::AUSD;
				let invest_amount_pool_denominated: u128 = 6 * decimals(18);
				let trader: AccountId = Keyring::Alice.into();
				create_currency_pool::<T>(pool_id, pool_currency, pool_currency_decimals.into());

				// USDT investment preparations
				let invest_amount_foreign_denominated = enable_usdt_trading::<T>(
					pool_currency,
					invest_amount_pool_denominated,
					true,
					true,
					true,
				);

				// Do first investment and fulfill swap order
				do_initial_increase_investment::<T>(
					pool_id,
					invest_amount_foreign_denominated,
					investor.clone(),
					foreign_currency,
				);
				fulfill_swap_into_pool::<T>(
					pool_id,
					default_order_id::<T>(&investor),
					invest_amount_pool_denominated,
					invest_amount_foreign_denominated,
					trader.clone(),
				);

				// Do second investment and not fulfill swap order
				let increase_msg = LiquidityPoolMessage::IncreaseInvestOrder {
					pool_id,
					tranche_id: default_tranche_id::<T>(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index::<T>(foreign_currency),
					amount: invest_amount_foreign_denominated,
				};
				assert_ok!(pallet_liquidity_pools::Pallet::<T>::submit(
					DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
					increase_msg
				));

				// Decrease pending pool swap by same amount
				let decrease_msg_pool_swap_amount = LiquidityPoolMessage::DecreaseInvestOrder {
					pool_id,
					tranche_id: default_tranche_id::<T>(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index::<T>(foreign_currency),
					amount: invest_amount_foreign_denominated,
				};
				assert_ok!(pallet_liquidity_pools::Pallet::<T>::submit(
					DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
					decrease_msg_pool_swap_amount
				));
				assert!(frame_system::Pallet::<T>::events().iter().any(|e| {
					e.event
						== pallet_liquidity_pools_gateway::Event::<T>::OutboundMessageSubmitted {
							sender: <T as pallet_liquidity_pools_gateway::Config>::Sender::get(),
							domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
							message: LiquidityPoolMessage::ExecutedDecreaseInvestOrder {
								pool_id,
								tranche_id: default_tranche_id::<T>(pool_id),
								investor: investor.clone().into(),
								currency: general_currency_index::<T>(foreign_currency),
								currency_payout: invest_amount_foreign_denominated,
								remaining_invest_amount: invest_amount_foreign_denominated,
							},
						}
						.into()
				}));

				// Decrease partially investing amount
				let decrease_msg_partial_invest_amount =
					LiquidityPoolMessage::DecreaseInvestOrder {
						pool_id,
						tranche_id: default_tranche_id::<T>(pool_id),
						investor: investor.clone().into(),
						currency: general_currency_index::<T>(foreign_currency),
						amount: invest_amount_foreign_denominated / 2,
					};
				assert_ok!(pallet_liquidity_pools::Pallet::<T>::submit(
					DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
					decrease_msg_partial_invest_amount.clone()
				));

				// Consume entire investing amount by sending same message
				assert_ok!(pallet_liquidity_pools::Pallet::<T>::submit(
					DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
					decrease_msg_partial_invest_amount.clone()
				));

				// Swap decreased amount
				assert_ok!(pallet_order_book::Pallet::<T>::fill_order(
					RawOrigin::Signed(trader.clone()).into(),
					default_order_id::<T>(&investor),
					invest_amount_pool_denominated
				));
				assert!(frame_system::Pallet::<T>::events().iter().any(|e| {
					e.event
						== pallet_liquidity_pools_gateway::Event::<T>::OutboundMessageSubmitted {
							sender: <T as pallet_liquidity_pools_gateway::Config>::Sender::get(),
							domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
							message: LiquidityPoolMessage::ExecutedDecreaseInvestOrder {
								pool_id,
								tranche_id: default_tranche_id::<T>(pool_id),
								investor: investor.clone().into(),
								currency: general_currency_index::<T>(foreign_currency),
								currency_payout: invest_amount_foreign_denominated,
								remaining_invest_amount: 0,
							},
						}
						.into()
				}));
			});
		}

		/// Propagate swaps only via OrderBook fulfillments.
		///
		/// Flow: Increase, fulfill, decrease, fulfill
		#[test_runtimes([development])]
		fn invest_swaps_happy_path<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.add(genesis::tokens::<T>(vec![
						(AUSD_CURRENCY_ID, AUSD_ED),
						(USDT_CURRENCY_ID, USDT_ED),
					]))
					.storage(),
			);

			setup_test(&mut env);

			env.parachain_state_mut(|| {
				let pool_id = POOL_ID;
				let investor =
					AccountConverter::domain_account_to_account(DOMAIN_MOONBEAM, Keyring::Bob.id());
				let trader: AccountId = Keyring::Alice.into();
				let pool_currency: CurrencyId = AUSD_CURRENCY_ID;
				let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
				let pool_currency_decimals = currency_decimals::AUSD;
				let invest_amount_pool_denominated: u128 = 10 * decimals(18);
				create_currency_pool::<T>(pool_id, pool_currency, pool_currency_decimals.into());
				let invest_amount_foreign_denominated: u128 = enable_usdt_trading::<T>(
					pool_currency,
					invest_amount_pool_denominated,
					true,
					true,
					true,
				);

				// Increase such that active swap into USDT is initialized
				do_initial_increase_investment::<T>(
					pool_id,
					invest_amount_foreign_denominated,
					investor.clone(),
					foreign_currency,
				);

				// Fulfilling order should propagate it from swapping to investing
				let swap_order_id = default_order_id::<T>(&investor);
				fulfill_swap_into_pool::<T>(
					pool_id,
					swap_order_id,
					invest_amount_pool_denominated,
					invest_amount_foreign_denominated,
					trader.clone(),
				);
				assert!(frame_system::Pallet::<T>::events().iter().any(|e| {
					e.event
						== pallet_order_book::Event::<T>::OrderFulfillment {
							order_id: swap_order_id,
							placing_account: investor.clone(),
							fulfilling_account: trader.clone(),
							partial_fulfillment: false,
							fulfillment_amount: invest_amount_foreign_denominated,
							currency_in: pool_currency,
							currency_out: foreign_currency,
							ratio: Ratio::one(),
						}
						.into()
				}));

				// Decrease by half the investment amount
				let msg = LiquidityPoolMessage::DecreaseInvestOrder {
					pool_id,
					tranche_id: default_tranche_id::<T>(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index::<T>(foreign_currency),
					amount: invest_amount_foreign_denominated / 2,
				};
				assert_ok!(pallet_liquidity_pools::Pallet::<T>::submit(
					DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
					msg.clone()
				));

				let swap_order_id = default_order_id::<T>(&investor);
				assert_ok!(pallet_order_book::Pallet::<T>::fill_order(
					RawOrigin::Signed(trader.clone()).into(),
					swap_order_id,
					invest_amount_pool_denominated / 2
				));
				assert!(frame_system::Pallet::<T>::events().iter().any(|e| {
					e.event
						== pallet_order_book::Event::<T>::OrderFulfillment {
							order_id: swap_order_id,
							placing_account: investor.clone(),
							fulfilling_account: trader.clone(),
							partial_fulfillment: false,
							fulfillment_amount: invest_amount_pool_denominated / 2,
							currency_in: foreign_currency,
							currency_out: pool_currency,
							ratio: Ratio::one(),
						}
						.into()
				}));

				let sender = <T as pallet_liquidity_pools_gateway::Config>::Sender::get();

				assert!(frame_system::Pallet::<T>::events().iter().any(|e| {
					e.event
						== pallet_liquidity_pools_gateway::Event::<T>::OutboundMessageSubmitted {
							sender: sender.clone(),
							domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
							message: LiquidityPoolMessage::ExecutedDecreaseInvestOrder {
								pool_id,
								tranche_id: default_tranche_id::<T>(pool_id),
								investor: investor.clone().into(),
								currency: general_currency_index::<T>(foreign_currency),
								currency_payout: invest_amount_foreign_denominated / 2,
								remaining_invest_amount: invest_amount_foreign_denominated / 2,
							},
						}
						.into()
				}));
			});
		}

		#[test_runtimes([development])]
		fn increase_fulfill_decrease_fulfill_partial_increase<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.storage(),
			);

			setup_test(&mut env);

			env.parachain_state_mut(|| {
				let pool_id = POOL_ID;
				let investor =
					AccountConverter::domain_account_to_account(DOMAIN_MOONBEAM, Keyring::Bob.id());
				let pool_currency: CurrencyId = AUSD_CURRENCY_ID;
				let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
				let pool_currency_decimals = currency_decimals::AUSD;
				let invest_amount_pool_denominated: u128 = 10 * decimals(18);
				let trader: AccountId = Keyring::Alice.into();
				create_currency_pool::<T>(pool_id, pool_currency, pool_currency_decimals.into());

				// USDT investment preparations
				let invest_amount_foreign_denominated = enable_usdt_trading::<T>(
					pool_currency,
					invest_amount_pool_denominated,
					true,
					true,
					true,
				);

				// Do first investment and fulfill swap order
				do_initial_increase_investment::<T>(
					pool_id,
					invest_amount_foreign_denominated,
					investor.clone(),
					foreign_currency,
				);
				fulfill_swap_into_pool::<T>(
					pool_id,
					default_order_id::<T>(&investor),
					invest_amount_pool_denominated,
					invest_amount_foreign_denominated,
					trader.clone(),
				);

				// Decrease pending pool swap by same amount
				let decrease_msg_pool_swap_amount = LiquidityPoolMessage::DecreaseInvestOrder {
					pool_id,
					tranche_id: default_tranche_id::<T>(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index::<T>(foreign_currency),
					amount: invest_amount_foreign_denominated,
				};
				assert_ok!(pallet_liquidity_pools::Pallet::<T>::submit(
					DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
					decrease_msg_pool_swap_amount
				));

				// Fulfill decrease swap partially
				assert_ok!(pallet_order_book::Pallet::<T>::fill_order(
					RawOrigin::Signed(trader.clone()).into(),
					default_order_id::<T>(&investor),
					3 * invest_amount_pool_denominated / 4
				));

				// Increase more than pending swap (pool -> foreign) amount from decrease
				let increase_msg = LiquidityPoolMessage::IncreaseInvestOrder {
					pool_id,
					tranche_id: default_tranche_id::<T>(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index::<T>(foreign_currency),
					amount: invest_amount_foreign_denominated / 2,
				};
				assert_ok!(pallet_liquidity_pools::Pallet::<T>::submit(
					DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
					increase_msg
				));

				assert!(frame_system::Pallet::<T>::events().iter().any(|e| {
					e.event
						== pallet_liquidity_pools_gateway::Event::<T>::OutboundMessageSubmitted {
							sender: <T as pallet_liquidity_pools_gateway::Config>::Sender::get(),
							domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
							message: LiquidityPoolMessage::ExecutedDecreaseInvestOrder {
								pool_id,
								tranche_id: default_tranche_id::<T>(pool_id),
								investor: investor.clone().into(),
								currency: general_currency_index::<T>(foreign_currency),
								currency_payout: invest_amount_foreign_denominated,
								remaining_invest_amount: invest_amount_foreign_denominated / 2,
							},
						}
						.into()
				}));
			});
		}
	}
}

mod routers {
	use super::*;

	mod axelar_evm {
		use std::ops::AddAssign;

		use super::*;

		#[test_runtimes([development])]
		fn test_via_outbound_queue<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.storage(),
			);

			let test_domain = Domain::EVM(1);

			let axelar_contract_address = H160::from_low_u64_be(1);
			let axelar_contract_code: Vec<u8> = vec![0, 0, 0];
			let axelar_contract_hash = BlakeTwo256::hash_of(&axelar_contract_code);
			let liquidity_pools_contract_address = H160::from_low_u64_be(2);

			env.parachain_state_mut(|| {
				pallet_evm::AccountCodes::<T>::insert(axelar_contract_address, axelar_contract_code)
			});

			let transaction_call_cost =
				env.parachain_state(|| <T as pallet_evm::Config>::config().gas_transaction_call);

			let evm_domain = EVMDomain {
				target_contract_address: axelar_contract_address,
				target_contract_hash: axelar_contract_hash,
				fee_values: FeeValues {
					value: U256::from(0),
					gas_limit: U256::from(transaction_call_cost + 1_000_000),
					gas_price: U256::from(10),
				},
			};

			let axelar_evm_router = AxelarEVMRouter::<T> {
				router: EVMRouter {
					evm_domain,
					_marker: Default::default(),
				},
				evm_chain: BoundedVec::<u8, ConstU32<MAX_AXELAR_EVM_CHAIN_SIZE>>::try_from(
					"ethereum".as_bytes().to_vec(),
				)
				.unwrap(),
				_marker: Default::default(),
				liquidity_pools_contract_address,
			};

			let test_router = DomainRouter::<T>::AxelarEVM(axelar_evm_router);

			env.parachain_state_mut(|| {
				assert_ok!(
					pallet_liquidity_pools_gateway::Pallet::<T>::set_domain_router(
						<T as frame_system::Config>::RuntimeOrigin::root(),
						test_domain.clone(),
						test_router,
					)
				);
			});

			let sender = Keyring::Alice.id();
			let gateway_sender = env
				.parachain_state(|| <T as pallet_liquidity_pools_gateway::Config>::Sender::get());

			let gateway_sender_h160: H160 = H160::from_slice(
				&<sp_core::crypto::AccountId32 as AsRef<[u8; 32]>>::as_ref(&gateway_sender)[0..20],
			);

			let msg = LiquidityPoolMessage::Transfer {
				currency: 0,
				sender: Keyring::Alice.id().into(),
				receiver: Keyring::Bob.id().into(),
				amount: 1_000u128,
			};

			// Failure - gateway sender account is not funded.
			assert_ok!(env.parachain_state_mut(|| {
				<pallet_liquidity_pools_gateway::Pallet<T> as OutboundQueue>::submit(
					sender.clone(),
					test_domain.clone(),
					msg.clone(),
				)
			}));

			let mut nonce = T::OutboundMessageNonce::one();

			let expected_event =
				pallet_liquidity_pools_gateway::Event::<T>::OutboundMessageExecutionFailure {
					sender: gateway_sender.clone(),
					domain: test_domain.clone(),
					message: msg.clone(),
					error: pallet_evm::Error::<T>::BalanceLow.into(),
					nonce,
				};

			env.pass(Blocks::UntilEvent {
				event: expected_event.clone().into(),
				limit: 3,
			});

			env.check_event(expected_event)
				.expect("expected RouterExecutionFailure event");

			nonce.add_assign(T::OutboundMessageNonce::one());

			assert_ok!(env.parachain_state_mut(|| {
				// Note how both the target address and the gateway sender need to have some
				// balance.
				crate::generic::utils::evm::mint_balance_into_derived_account::<T>(
					axelar_contract_address,
					cfg(1_000_000_000),
				);
				crate::generic::utils::evm::mint_balance_into_derived_account::<T>(
					gateway_sender_h160,
					cfg(1_000_000),
				);

				<pallet_liquidity_pools_gateway::Pallet<T> as OutboundQueue>::submit(
					sender.clone(),
					test_domain.clone(),
					msg.clone(),
				)
			}));

			let expected_event =
				pallet_liquidity_pools_gateway::Event::<T>::OutboundMessageExecutionSuccess {
					sender: gateway_sender.clone(),
					domain: test_domain.clone(),
					message: msg.clone(),
					nonce,
				};

			env.pass(Blocks::UntilEvent {
				event: expected_event.clone().into(),
				limit: 3,
			});

			env.check_event(expected_event)
				.expect("expected OutboundMessageExecutionSuccess event");

			// Router not found
			let unused_domain = Domain::EVM(1234);

			env.parachain_state_mut(|| {
				assert_noop!(
					<pallet_liquidity_pools_gateway::Pallet<T> as OutboundQueue>::submit(
						sender,
						unused_domain.clone(),
						msg,
					),
					pallet_liquidity_pools_gateway::Error::<T>::RouterNotFound
				);
			});
		}
	}

	mod ethereum_xcm {
		use super::*;

		mod utils {
			use super::*;

			pub fn submit_test_fn<T: Runtime + FudgeSupport>(
				router_creation_fn: RouterCreationFn<T>,
			) {
				let mut env = FudgeEnv::<T>::from_parachain_storage(
					Genesis::default()
						.add(genesis::balances::<T>(cfg(1_000)))
						.storage(),
				);

				setup_test(&mut env);

				enable_para_to_sibling_communication::<T>(&mut env);

				let msg = Message::<Domain, PoolId, TrancheId, Balance, Quantity>::Transfer {
					currency: 0,
					sender: Keyring::Alice.into(),
					receiver: Keyring::Bob.into(),
					amount: 1_000u128,
				};

				env.parachain_state_mut(|| {
					let domain_router = router_creation_fn(
						Location::new(1, Parachain(SIBLING_ID)).into(),
						GLMR_CURRENCY_ID,
					);

					assert_ok!(
						pallet_liquidity_pools_gateway::Pallet::<T>::set_domain_router(
							<T as frame_system::Config>::RuntimeOrigin::root(),
							TEST_DOMAIN,
							domain_router,
						)
					);

					assert_ok!(
						<pallet_liquidity_pools_gateway::Pallet::<T> as OutboundQueue>::submit(
							Keyring::Alice.into(),
							TEST_DOMAIN,
							msg.clone(),
						)
					);
				});

				let gateway_sender = env.parachain_state(|| {
					<T as pallet_liquidity_pools_gateway::Config>::Sender::get()
				});

				let expected_event =
					pallet_liquidity_pools_gateway::Event::<T>::OutboundMessageExecutionSuccess {
						sender: gateway_sender,
						domain: TEST_DOMAIN,
						message: msg,
						nonce: T::OutboundMessageNonce::one(),
					};

				env.pass(Blocks::UntilEvent {
					event: expected_event.clone().into(),
					limit: 3,
				});

				env.check_event(expected_event)
					.expect("expected OutboundMessageExecutionSuccess event");
			}

			type RouterCreationFn<T> =
				Box<dyn Fn(VersionedLocation, CurrencyId) -> DomainRouter<T>>;

			pub fn get_axelar_xcm_router_fn<T: Runtime + FudgeSupport>() -> RouterCreationFn<T> {
				Box::new(
					|location: VersionedLocation, currency_id: CurrencyId| -> DomainRouter<T> {
						let router = AxelarXCMRouter::<T> {
							router: XCMRouter {
								xcm_domain: XcmDomain {
									location: Box::new(
										location.try_into().expect("Bad xcm domain location"),
									),
									ethereum_xcm_transact_call_index: BoundedVec::truncate_from(
										vec![38, 0],
									),
									contract_address: H160::from_low_u64_be(11),
									max_gas_limit: 700_000,
									transact_required_weight_at_most: Default::default(),
									overall_weight: Default::default(),
									fee_currency: currency_id,
									fee_amount: decimals(18).saturating_div(5),
								},
								_marker: Default::default(),
							},
							axelar_target_chain: BoundedVec::<
								u8,
								ConstU32<MAX_AXELAR_EVM_CHAIN_SIZE>,
							>::try_from("ethereum".as_bytes().to_vec())
							.unwrap(),
							axelar_target_contract: H160::from_low_u64_be(111),
							_marker: Default::default(),
						};

						DomainRouter::AxelarXCM(router)
					},
				)
			}

			pub fn get_ethereum_xcm_router_fn<T: Runtime + FudgeSupport>() -> RouterCreationFn<T> {
				Box::new(
					|location: VersionedLocation, currency_id: CurrencyId| -> DomainRouter<T> {
						let router = EthereumXCMRouter::<T> {
							router: XCMRouter {
								xcm_domain: XcmDomain {
									location: Box::new(
										location.try_into().expect("Bad xcm domain location"),
									),
									ethereum_xcm_transact_call_index: BoundedVec::truncate_from(
										vec![38, 0],
									),
									contract_address: H160::from_low_u64_be(11),
									max_gas_limit: 700_000,
									transact_required_weight_at_most: Default::default(),
									overall_weight: Default::default(),
									fee_currency: currency_id,
									fee_amount: decimals(18).saturating_div(5),
								},
								_marker: Default::default(),
							},
							_marker: Default::default(),
						};

						DomainRouter::EthereumXCM(router)
					},
				)
			}
		}

		use utils::*;

		const TEST_DOMAIN: Domain = Domain::EVM(1);

		#[test_runtimes([development])]
		fn submit_ethereum_xcm<T: Runtime + FudgeSupport>() {
			submit_test_fn::<T>(get_ethereum_xcm_router_fn::<T>());
		}

		#[test_runtimes([development])]
		fn submit_axelar_xcm<T: Runtime + FudgeSupport>() {
			submit_test_fn::<T>(get_axelar_xcm_router_fn::<T>());
		}
	}
}
