use cfg_primitives::{
	currency_decimals, parachains, AccountId, Balance, CouncilCollective, OrderId, PoolId,
	TrancheId,
};
use cfg_traits::{
	investments::{Investment, OrderManager, TrancheCurrency},
	liquidity_pools::{Codec, InboundQueue, OutboundQueue},
	IdentityCurrencyConversion, Permissions, PoolInspect, PoolMutate, Seconds,
};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	fixed_point::{Quantity, Ratio},
	investments::{InvestCollection, InvestmentAccount, RedeemCollection},
	locations::Location,
	orders::FulfillmentWithPrice,
	permissions::{PermissionScope, PoolRole, Role},
	pools::TrancheMetadata,
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
	xcm::XcmMetadata,
};
use cfg_utils::vec_to_fixed_array;
use frame_support::{
	assert_noop, assert_ok,
	dispatch::{RawOrigin, Weight},
	traits::{
		fungible::Mutate as FungibleMutate,
		fungibles::{Inspect, Mutate as FungiblesMutate},
		OriginTrait, PalletInfo,
	},
};
use liquidity_pools_gateway_routers::{
	AxelarEVMRouter, AxelarXCMRouter, DomainRouter, EVMDomain, EVMRouter, EthereumXCMRouter,
	FeeValues, XCMRouter, XcmDomain, DEFAULT_PROOF_SIZE, MAX_AXELAR_EVM_CHAIN_SIZE,
};
use orml_traits::{asset_registry::AssetMetadata, MultiCurrency};
use pallet_investments::CollectOutcome;
use pallet_liquidity_pools::Message;
use pallet_liquidity_pools_gateway::{Call as LiquidityPoolsGatewayCall, GatewayOrigin};
use pallet_pool_system::tranches::{TrancheInput, TrancheLoc, TrancheType};
use parity_scale_codec::Encode;
use polkadot_core_primitives::BlakeTwo256;
use polkadot_parachain::primitives::{Id, ValidationCode};
use polkadot_runtime_parachains::{
	paras,
	paras::{ParaGenesisArgs, ParaKind},
};
use runtime_common::{
	account_conversion::AccountConverter,
	foreign_investments::IdentityPoolCurrencyConverter,
	xcm::general_key,
	xcm_fees::{default_per_second, ksm_per_second},
};
use sp_core::{Get, H160, U256};
use sp_runtime::{
	traits::{AccountIdConversion, BadOrigin, ConstU32, Convert as C2, EnsureAdd, Hash, One, Zero},
	BoundedVec, BuildStorage, DispatchError, FixedPointNumber, Perquintill, SaturatedConversion,
	WeakBoundedVec,
};
use staging_xcm::{
	latest::NetworkId,
	prelude::XCM_VERSION,
	v3::{
		AssetId, Fungibility, Junction, Junction::*, Junctions, Junctions::*, MultiAsset,
		MultiAssets, MultiLocation, WeightLimit,
	},
	VersionedMultiAsset, VersionedMultiAssets, VersionedMultiLocation,
};
use staging_xcm_executor::traits::Convert as C1;

use crate::{
	generic::{
		config::Runtime,
		env::{Blocks, Env},
		envs::fudge_env::{handle::FudgeHandle, FudgeEnv, FudgeSupport},
		utils::{democracy::execute_via_democracy, genesis, genesis::Genesis},
	},
	utils::{accounts::Keyring, AUSD_CURRENCY_ID, AUSD_ED, USDT_CURRENCY_ID, USDT_ED},
};

pub mod utils {
	use super::*;

	pub fn parachain_account(id: u32) -> AccountId {
		polkadot_parachain::primitives::Sibling::from(id).into_account_truncating()
	}

	pub fn xcm_metadata(transferability: CrossChainTransferability) -> Option<XcmMetadata> {
		match transferability {
			CrossChainTransferability::Xcm(x) => Some(x),
			_ => None,
		}
	}

	pub fn setup_xcm<T: Runtime + FudgeSupport>(env: &mut FudgeEnv<T>) {
		env.parachain_state_mut(|| {
			// Set the XCM version used when sending XCM messages to sibling.
			assert_ok!(pallet_xcm::Pallet::<T>::force_xcm_version(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				Box::new(MultiLocation::new(
					1,
					Junctions::X1(Junction::Parachain(T::FudgeHandle::SIBLING_ID)),
				)),
				XCM_VERSION,
			));
		});

		env.sibling_state_mut(|| {
			// Set the XCM version used when sending XCM messages to parachain.
			assert_ok!(pallet_xcm::Pallet::<T>::force_xcm_version(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				Box::new(MultiLocation::new(
					1,
					Junctions::X1(Junction::Parachain(T::FudgeHandle::PARA_ID)),
				)),
				XCM_VERSION,
			));
		});

		env.relay_state_mut(|| {
			assert_ok!(polkadot_runtime_parachains::hrmp::Pallet::<
				FudgeRelayRuntime<T>,
			>::force_open_hrmp_channel(
				<FudgeRelayRuntime<T> as frame_system::Config>::RuntimeOrigin::root(),
				Id::from(T::FudgeHandle::PARA_ID),
				Id::from(T::FudgeHandle::SIBLING_ID),
				10,
				1024,
			));

			assert_ok!(polkadot_runtime_parachains::hrmp::Pallet::<
				FudgeRelayRuntime<T>,
			>::force_open_hrmp_channel(
				<FudgeRelayRuntime<T> as frame_system::Config>::RuntimeOrigin::root(),
				Id::from(T::FudgeHandle::SIBLING_ID),
				Id::from(T::FudgeHandle::PARA_ID),
				10,
				1024,
			));

			assert_ok!(polkadot_runtime_parachains::hrmp::Pallet::<
				FudgeRelayRuntime<T>,
			>::force_process_hrmp_open(
				<FudgeRelayRuntime<T> as frame_system::Config>::RuntimeOrigin::root(),
				0,
			));
		});

		env.pass(Blocks::ByNumber(1));
	}

	pub fn setup_usdc_xcm<T: Runtime + FudgeSupport>(env: &mut FudgeEnv<T>) {
		env.parachain_state_mut(|| {
			// Set the XCM version used when sending XCM messages to USDC parachain.
			assert_ok!(pallet_xcm::Pallet::<T>::force_xcm_version(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				Box::new(MultiLocation::new(
					1,
					Junctions::X1(Junction::Parachain(1000)),
				)),
				XCM_VERSION,
			));
		});

		env.relay_state_mut(|| {
			assert_ok!(polkadot_runtime_parachains::hrmp::Pallet::<
				FudgeRelayRuntime<T>,
			>::force_open_hrmp_channel(
				<FudgeRelayRuntime<T> as frame_system::Config>::RuntimeOrigin::root(),
				Id::from(T::FudgeHandle::PARA_ID),
				Id::from(1000),
				10,
				1024,
			));

			assert_ok!(polkadot_runtime_parachains::hrmp::Pallet::<
				FudgeRelayRuntime<T>,
			>::force_process_hrmp_open(
				<FudgeRelayRuntime<T> as frame_system::Config>::RuntimeOrigin::root(),
				0,
			));
		});

		env.pass(Blocks::ByNumber(1));
	}

	pub fn register_ausd<T: Runtime + FudgeSupport>() {
		let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
			decimals: 12,
			name: "Acala Dollar".into(),
			symbol: "AUSD".into(),
			existential_deposit: 1_000_000_000,
			location: Some(VersionedMultiLocation::V3(MultiLocation::new(
				1,
				X2(
					Parachain(T::FudgeHandle::SIBLING_ID),
					general_key(parachains::kusama::karura::AUSD_KEY),
				),
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

	pub fn ausd(amount: Balance) -> Balance {
		amount * decimals(currency_decimals::AUSD)
	}

	pub fn ausd_fee() -> Balance {
		fee(currency_decimals::AUSD)
	}

	pub fn cfg(amount: Balance) -> Balance {
		amount * decimals(currency_decimals::NATIVE)
	}

	pub fn cfg_fee() -> Balance {
		fee(currency_decimals::NATIVE)
	}

	pub fn decimals(decimals: u32) -> Balance {
		10u128.saturating_pow(decimals)
	}

	pub fn fee(decimals: u32) -> Balance {
		calc_fee(default_per_second(decimals))
	}

	pub fn calc_fee(fee_per_second: Balance) -> Balance {
		// We divide the fee to align its unit and multiply by 4 as that seems to be the
		// unit of time the tests take.
		// NOTE: it is possible that in different machines this value may differ. We
		// shall see.
		fee_per_second.div_euclid(10_000) * 8
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
}

type FudgeRelayRuntime<T> = <<T as FudgeSupport>::FudgeHandle as FudgeHandle<T>>::RelayRuntime;

use utils::*;

mod development {
	use development_runtime::xcm::LocationToAccountId;

	use super::*;

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
		DomainAddress::EVM(crate::utils::MOONBEAM_EVM_CHAIN_ID, [0; 20]);

	pub type LiquidityPoolMessage = Message<Domain, PoolId, TrancheId, Balance, Quantity>;

	mod utils {
		use cfg_types::oracles::OracleKey;
		use runtime_common::oracle::Feeder;

		use super::*;

		/// Creates a new pool for the given id with
		///  * BOB as admin and depositor
		///  * Two tranches
		///  * AUSD as pool currency with max reserve 10k.
		pub fn create_ausd_pool<T: Runtime + FudgeSupport>(pool_id: u64) {
			create_currency_pool::<T>(pool_id, AUSD_CURRENCY_ID, decimals(currency_decimals::AUSD))
		}

		/// Creates a new pool for for the given id with the provided currency.
		///  * BOB as admin and depositor
		///  * Two tranches
		///  * The given `currency` as pool currency with of
		///    `currency_decimals`.
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
						metadata: TrancheMetadata {
							// NOTE: For now, we have to set these metadata fields of the first
							// tranche to be convertible to the 32-byte size expected by the
							// liquidity pools AddTranche message.
							token_name: BoundedVec::<
								u8,
								<T as pallet_pool_system::Config>::MaxTokenNameLength,
							>::try_from("A highly advanced tranche".as_bytes().to_vec())
							.expect(""),
							token_symbol: BoundedVec::<
								u8,
								<T as pallet_pool_system::Config>::MaxTokenSymbolLength,
							>::try_from("TrNcH".as_bytes().to_vec())
							.expect(""),
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
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 18,
				name: "Glimmer".into(),
				symbol: "GLMR".into(),
				existential_deposit: GLMR_ED,
				location: Some(VersionedMultiLocation::V3(MultiLocation::new(
					1,
					X2(Parachain(T::FudgeHandle::SIBLING_ID), general_key(&[0, 1])),
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
			xcm_domain_location: VersionedMultiLocation,
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

		/// Returns a `VersionedMultiLocation` that can be converted into
		/// `LiquidityPoolsWrappedToken` which is required for cross chain asset
		/// registration and transfer.
		pub fn liquidity_pools_transferable_multilocation<T: Runtime + FudgeSupport>(
			chain_id: u64,
			address: [u8; 20],
		) -> VersionedMultiLocation {
			VersionedMultiLocation::V3(MultiLocation {
				parents: 0,
				interior: X3(
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
				),
			})
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
			setup_xcm(env);

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
					MultiLocation::new(
						1,
						Junctions::X1(Junction::Parachain(T::FudgeHandle::SIBLING_ID)),
					)
					.into(),
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

		pub fn default_investment_id<T: Runtime + FudgeSupport>(
		) -> cfg_types::tokens::TrancheCurrency {
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
				crate::generic::utils::give_pool_role::<T>(
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
					orml_tokens::Pallet::<T>::balance(
						currency_id,
						&default_investment_account::<T>()
					),
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
			crate::generic::utils::give_pool_role::<T>(
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
					&AccountConverter::<T, LocationToAccountId>::convert(
						DEFAULT_OTHER_DOMAIN_ADDRESS
					)
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
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 6,
				name: "Tether USDT".into(),
				symbol: "USDT".into(),
				existential_deposit: USDT_ED,
				location: Some(VersionedMultiLocation::V3(MultiLocation::new(
					1,
					X3(Parachain(1000), PalletInstance(50), GeneralIndex(1984)),
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

		fn add_pool<T: Runtime + FudgeSupport>() {
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

				// Verify that the pool must exist before we can call
				// pallet_liquidity_pools::Pallet::<T>::add_pool
				assert_noop!(
					pallet_liquidity_pools::Pallet::<T>::add_pool(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						pool_id,
						Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
					),
					pallet_liquidity_pools::Error::<T>::PoolNotFound
				);

				// Now create the pool
				create_ausd_pool::<T>(pool_id);

				// Verify ALICE can't call `add_pool` given she is not the `PoolAdmin`
				assert_noop!(
					pallet_liquidity_pools::Pallet::<T>::add_pool(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						pool_id,
						Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
					),
					pallet_liquidity_pools::Error::<T>::NotPoolAdmin
				);

				// Verify that it works if it's BOB calling it (the pool admin)
				assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_pool(
					RawOrigin::Signed(POOL_ADMIN.into()).into(),
					pool_id,
					Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
				));
			});
		}

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
				let new_member = DomainAddress::EVM(crate::utils::MOONBEAM_EVM_CHAIN_ID, [3; 20]);

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
				crate::generic::utils::give_pool_role::<T>(
					AccountConverter::<T, LocationToAccountId>::convert(new_member.clone()),
					pool_id,
					PoolRole::TrancheInvestor(default_tranche_id::<T>(pool_id), DEFAULT_VALIDITY),
				);

				// Verify the Investor role was set as expected in Permissions
				assert!(pallet_permissions::Pallet::<T>::has(
					PermissionScope::Pool(pool_id),
					AccountConverter::<T, LocationToAccountId>::convert(new_member.clone()),
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

			let (domain, sender, message) = env.parachain_state_mut(|| {
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

				let outbound_message =
					pallet_liquidity_pools_gateway::OutboundMessageQueue::<T>::get(
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

				outbound_message
			});

			let expected_event =
				pallet_liquidity_pools_gateway::Event::<T>::OutboundMessageExecutionSuccess {
					sender,
					domain,
					message,
					nonce: T::OutboundMessageNonce::one(),
				};

			env.pass(Blocks::UntilEvent {
				event: expected_event.clone().into(),
				limit: 3,
			});

			env.check_event(expected_event)
				.expect("expected RouterExecutionSuccess event");
		}

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
				// MultiLocation
				let currency_id = CurrencyId::ForeignAsset(100);

				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					AssetMetadata {
						name: "Test".into(),
						symbol: "TEST".into(),
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

				// Add convertable MultiLocation to metadata but remove transferability
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
						name: "Test".into(),
						symbol: "TEST".into(),
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

				// Should fail if currency does not have any MultiLocation in metadata
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
					Some(Some(VersionedMultiLocation::V3(Default::default()))),
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
						name: "Test".into(),
						symbol: "TEST".into(),
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

				// Should fail if currency does not have any MultiLocation in metadata
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
					Some(Some(VersionedMultiLocation::V3(Default::default()))),
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

		crate::test_for_runtimes!([development], add_pool);
		crate::test_for_runtimes!([development], add_tranche);
		crate::test_for_runtimes!([development], update_member);
		crate::test_for_runtimes!([development], update_token_price);
		crate::test_for_runtimes!([development], add_currency);
		crate::test_for_runtimes!([development], add_currency_should_fail);
		crate::test_for_runtimes!([development], allow_investment_currency);
		crate::test_for_runtimes!([development], allow_investment_currency_should_fail);
		crate::test_for_runtimes!([development], disallow_investment_currency);
		crate::test_for_runtimes!([development], disallow_investment_currency_should_fail);
		crate::test_for_runtimes!([development], schedule_upgrade);
		crate::test_for_runtimes!([development], cancel_upgrade);
		crate::test_for_runtimes!([development], update_tranche_token_metadata);
	}

	mod foreign_investments {
		use super::*;

		mod same_currencies {
			use super::*;

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
					let investor: AccountId = AccountConverter::<T, LocationToAccountId>::convert(
						(DOMAIN_MOONBEAM, Keyring::Bob.into()),
					);
					let currency_id = AUSD_CURRENCY_ID;
					let currency_decimals = currency_decimals::AUSD;

					// Create new pool
					create_currency_pool::<T>(pool_id, currency_id, currency_decimals.into());

					// Set permissions and execute initial investment
					do_initial_increase_investment::<T>(
						pool_id,
						amount,
						investor.clone(),
						currency_id,
					);

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
					let investor: AccountId = AccountConverter::<T, LocationToAccountId>::convert(
						(DOMAIN_MOONBEAM, Keyring::Bob.into()),
					);
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
					let investor: AccountId = AccountConverter::<T, LocationToAccountId>::convert(
						(DOMAIN_MOONBEAM, Keyring::Bob.into()),
					);
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
					let investor: AccountId = AccountConverter::<T, LocationToAccountId>::convert(
						(DOMAIN_MOONBEAM, Keyring::Bob.into()),
					);
					let currency_id = AUSD_CURRENCY_ID;
					let currency_decimals = currency_decimals::AUSD;
					let sending_domain_locator =
						Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain());
					enable_liquidity_pool_transferability::<T>(currency_id);

					// Create new pool
					create_currency_pool::<T>(pool_id, currency_id, currency_decimals.into());
					let investment_currency_id: CurrencyId = default_investment_id::<T>().into();
					// Set permissions and execute initial investment
					do_initial_increase_investment::<T>(
						pool_id,
						amount,
						investor.clone(),
						currency_id,
					);
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
					let investor: AccountId = AccountConverter::<T, LocationToAccountId>::convert(
						(DOMAIN_MOONBEAM, Keyring::Bob.into()),
					);
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
						orml_tokens::Pallet::<T>::balance(investment_currency_id, &investor)
							.is_zero()
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
					let investor: AccountId = AccountConverter::<T, LocationToAccountId>::convert(
						(DOMAIN_MOONBEAM, Keyring::Bob.into()),
					);
					let currency_id = AUSD_CURRENCY_ID;
					let currency_decimals = currency_decimals::AUSD;

					// Create new pool
					create_currency_pool::<T>(pool_id, currency_id, currency_decimals.into());

					// Set permissions and execute initial redemption
					do_initial_increase_redemption::<T>(
						pool_id,
						amount,
						investor.clone(),
						currency_id,
					);

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
					let investor: AccountId = AccountConverter::<T, LocationToAccountId>::convert(
						(DOMAIN_MOONBEAM, Keyring::Bob.into()),
					);
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
					let investor: AccountId = AccountConverter::<T, LocationToAccountId>::convert(
						(DOMAIN_MOONBEAM, Keyring::Bob.into()),
					);
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
					let investor: AccountId = AccountConverter::<T, LocationToAccountId>::convert(
						(DOMAIN_MOONBEAM, Keyring::Bob.into()),
					);
					let currency_id = AUSD_CURRENCY_ID;
					let currency_decimals = currency_decimals::AUSD;
					let pool_account = pallet_pool_system::pool_types::PoolLocator { pool_id }
						.into_account_truncating();

					// Create new pool
					create_currency_pool::<T>(pool_id, currency_id, currency_decimals.into());

					// Set permissions and execute initial investment
					do_initial_increase_redemption::<T>(
						pool_id,
						amount,
						investor.clone(),
						currency_id,
					);
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
					let investor: AccountId = AccountConverter::<T, LocationToAccountId>::convert(
						(DOMAIN_MOONBEAM, Keyring::Bob.into()),
					);
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

			crate::test_for_runtimes!([development], increase_invest_order);
			crate::test_for_runtimes!([development], decrease_invest_order);
			crate::test_for_runtimes!([development], cancel_invest_order);
			crate::test_for_runtimes!([development], collect_invest_order);
			crate::test_for_runtimes!(
				[development],
				partially_collect_investment_for_through_investments
			);
			crate::test_for_runtimes!([development], increase_redeem_order);
			crate::test_for_runtimes!([development], decrease_redeem_order);
			crate::test_for_runtimes!([development], cancel_redeem_order);
			crate::test_for_runtimes!([development], fully_collect_redeem_order);
			crate::test_for_runtimes!(
				[development],
				partially_collect_redemption_for_through_investments
			);

			mod should_fail {
				use super::*;

				mod decrease_should_underflow {
					use super::*;

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
							let investor: AccountId =
								AccountConverter::<T, LocationToAccountId>::convert((
									DOMAIN_MOONBEAM,
									Keyring::Bob.into(),
								));
							let currency_id: CurrencyId = AUSD_CURRENCY_ID;
							let currency_decimals = currency_decimals::AUSD;
							create_currency_pool::<T>(
								pool_id,
								currency_id,
								currency_decimals.into(),
							);
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
							let investor: AccountId =
								AccountConverter::<T, LocationToAccountId>::convert((
									DOMAIN_MOONBEAM,
									Keyring::Bob.into(),
								));
							let currency_id: CurrencyId = AUSD_CURRENCY_ID;
							let currency_decimals = currency_decimals::AUSD;
							create_currency_pool::<T>(
								pool_id,
								currency_id,
								currency_decimals.into(),
							);
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

					crate::test_for_runtimes!([development], invest_decrease_underflow);
					crate::test_for_runtimes!([development], redeem_decrease_underflow);
				}

				mod should_throw_requires_collect {
					use super::*;

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
							let investor: AccountId =
								AccountConverter::<T, LocationToAccountId>::convert((
									DOMAIN_MOONBEAM,
									Keyring::Bob.into(),
								));
							let currency_id: CurrencyId = AUSD_CURRENCY_ID;
							let currency_decimals = currency_decimals::AUSD;
							create_currency_pool::<T>(
								pool_id,
								currency_id,
								currency_decimals.into(),
							);
							do_initial_increase_investment::<T>(
								pool_id,
								amount,
								investor.clone(),
								currency_id,
							);
							enable_liquidity_pool_transferability::<T>(currency_id);

							// Prepare collection
							let pool_account =
								pallet_pool_system::pool_types::PoolLocator { pool_id }
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
							let investor: AccountId =
								AccountConverter::<T, LocationToAccountId>::convert((
									DOMAIN_MOONBEAM,
									Keyring::Bob.into(),
								));
							let currency_id: CurrencyId = AUSD_CURRENCY_ID;
							let currency_decimals = currency_decimals::AUSD;
							create_currency_pool::<T>(
								pool_id,
								currency_id,
								currency_decimals.into(),
							);
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
							let pool_account =
								pallet_pool_system::pool_types::PoolLocator { pool_id }
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

					crate::test_for_runtimes!([development], invest_requires_collect);
					crate::test_for_runtimes!([development], redeem_requires_collect);
				}

				mod payment_payout_currency {
					use super::*;

					fn invalid_invest_payment_currency<T: Runtime + FudgeSupport>() {
						let mut env = FudgeEnv::<T>::from_parachain_storage(
							Genesis::default()
								.add(genesis::balances::<T>(cfg(1_000)))
								.storage(),
						);

						setup_test(&mut env);

						env.parachain_state_mut(|| {
							let pool_id = POOL_ID;
							let investor: AccountId =
								AccountConverter::<T, LocationToAccountId>::convert((
									DOMAIN_MOONBEAM,
									Keyring::Bob.into(),
								));
							let pool_currency = AUSD_CURRENCY_ID;
							let currency_decimals = currency_decimals::AUSD;
							let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
							let amount = 6 * decimals(18);

							create_currency_pool::<T>(
								pool_id,
								pool_currency,
								currency_decimals.into(),
							);
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

					fn invalid_redeem_payout_currency<T: Runtime + FudgeSupport>() {
						let mut env = FudgeEnv::<T>::from_parachain_storage(
							Genesis::default()
								.add(genesis::balances::<T>(cfg(1_000)))
								.storage(),
						);

						setup_test(&mut env);

						env.parachain_state_mut(|| {
							let pool_id = POOL_ID;
							let investor: AccountId =
								AccountConverter::<T, LocationToAccountId>::convert((
									DOMAIN_MOONBEAM,
									Keyring::Bob.into(),
								));
							let pool_currency = AUSD_CURRENCY_ID;
							let currency_decimals = currency_decimals::AUSD;
							let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
							let amount = 6 * decimals(18);

							create_currency_pool::<T>(
								pool_id,
								pool_currency,
								currency_decimals.into(),
							);
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

					fn redeem_payout_currency_not_found<T: Runtime + FudgeSupport>() {
						let mut env = FudgeEnv::<T>::from_parachain_storage(
							Genesis::default()
								.add(genesis::balances::<T>(cfg(1_000)))
								.storage(),
						);

						setup_test(&mut env);

						env.parachain_state_mut(|| {
							let pool_id = POOL_ID;
							let investor: AccountId =
								AccountConverter::<T, LocationToAccountId>::convert((
									DOMAIN_MOONBEAM,
									Keyring::Bob.into(),
								));
							let pool_currency = AUSD_CURRENCY_ID;
							let currency_decimals = currency_decimals::AUSD;
							let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
							let amount = 6 * decimals(18);

							create_currency_pool::<T>(
								pool_id,
								pool_currency,
								currency_decimals.into(),
							);
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

					crate::test_for_runtimes!([development], invalid_invest_payment_currency);
					crate::test_for_runtimes!([development], invalid_redeem_payout_currency);
					crate::test_for_runtimes!([development], redeem_payout_currency_not_found);
				}
			}
		}

		mod mismatching_currencies {
			use super::*;

			fn collect_foreign_investment_for<T: Runtime + FudgeSupport>() {
				let mut env = FudgeEnv::<T>::from_parachain_storage(
					Genesis::default()
						.add(genesis::balances::<T>(cfg(1_000)))
						.storage(),
				);

				setup_test(&mut env);

				env.parachain_state_mut(|| {
					let pool_id = POOL_ID;
					let investor: AccountId = AccountConverter::<T, LocationToAccountId>::convert(
						(DOMAIN_MOONBEAM, Keyring::Bob.into()),
					);
					let pool_currency: CurrencyId = AUSD_CURRENCY_ID;
					let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
					let pool_currency_decimals = currency_decimals::AUSD;
					let invest_amount_pool_denominated: u128 = 6 * decimals(18);
					let sending_domain_locator =
						Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain());
					let trader: AccountId = Keyring::Alice.into();
					create_currency_pool::<T>(
						pool_id,
						pool_currency,
						pool_currency_decimals.into(),
					);

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
			fn increase_fulfill_increase_decrease_decrease_partial<T: Runtime + FudgeSupport>() {
				let mut env = FudgeEnv::<T>::from_parachain_storage(
					Genesis::default()
						.add(genesis::balances::<T>(cfg(1_000)))
						.storage(),
				);

				setup_test(&mut env);

				env.parachain_state_mut(|| {
					let pool_id = POOL_ID;
					let investor: AccountId = AccountConverter::<T, LocationToAccountId>::convert(
						(DOMAIN_MOONBEAM, Keyring::Bob.into()),
					);
					let pool_currency: CurrencyId = AUSD_CURRENCY_ID;
					let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
					let pool_currency_decimals = currency_decimals::AUSD;
					let invest_amount_pool_denominated: u128 = 6 * decimals(18);
					let trader: AccountId = Keyring::Alice.into();
					create_currency_pool::<T>(
						pool_id,
						pool_currency,
						pool_currency_decimals.into(),
					);

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
					let investor: AccountId = AccountConverter::<T, LocationToAccountId>::convert(
						(DOMAIN_MOONBEAM, Keyring::Bob.into()),
					);
					let trader: AccountId = Keyring::Alice.into();
					let pool_currency: CurrencyId = AUSD_CURRENCY_ID;
					let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
					let pool_currency_decimals = currency_decimals::AUSD;
					let invest_amount_pool_denominated: u128 = 10 * decimals(18);
					create_currency_pool::<T>(
						pool_id,
						pool_currency,
						pool_currency_decimals.into(),
					);
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

			fn increase_fulfill_decrease_fulfill_partial_increase<T: Runtime + FudgeSupport>() {
				let mut env = FudgeEnv::<T>::from_parachain_storage(
					Genesis::default()
						.add(genesis::balances::<T>(cfg(1_000)))
						.storage(),
				);

				setup_test(&mut env);

				env.parachain_state_mut(|| {
					let pool_id = POOL_ID;
					let investor: AccountId = AccountConverter::<T, LocationToAccountId>::convert(
						(DOMAIN_MOONBEAM, Keyring::Bob.into()),
					);
					let pool_currency: CurrencyId = AUSD_CURRENCY_ID;
					let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
					let pool_currency_decimals = currency_decimals::AUSD;
					let invest_amount_pool_denominated: u128 = 10 * decimals(18);
					let trader: AccountId = Keyring::Alice.into();
					create_currency_pool::<T>(
						pool_id,
						pool_currency,
						pool_currency_decimals.into(),
					);

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

			crate::test_for_runtimes!([development], collect_foreign_investment_for);
			crate::test_for_runtimes!(
				[development],
				increase_fulfill_increase_decrease_decrease_partial
			);
			crate::test_for_runtimes!(
				[development],
				increase_fulfill_decrease_fulfill_partial_increase
			);
			crate::test_for_runtimes!([development], invest_swaps_happy_path);
		}
	}

	mod transfers {
		use super::*;

		fn transfer_non_tranche_tokens_from_local<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.storage(),
			);

			setup_test(&mut env);

			env.parachain_state_mut(|| {
				let initial_balance = 2 * AUSD_ED;
				let amount = initial_balance / 2;
				let dest_address = DEFAULT_DOMAIN_ADDRESS_MOONBEAM;
				let currency_id = AUSD_CURRENCY_ID;
				let source_account = Keyring::Charlie;

				// Mint sufficient balance
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(currency_id, &source_account.into()),
					0
				);
				assert_ok!(orml_tokens::Pallet::<T>::mint_into(
					currency_id,
					&source_account.into(),
					initial_balance
				));
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(currency_id, &source_account.into()),
					initial_balance
				);

				// Only `ForeignAsset` can be transferred
				assert_noop!(
					pallet_liquidity_pools::Pallet::<T>::transfer(
						RawOrigin::Signed(source_account.into()).into(),
						CurrencyId::Tranche(42u64, [0u8; 16]),
						dest_address.clone(),
						amount,
					),
					pallet_liquidity_pools::Error::<T>::InvalidTransferCurrency
				);
				assert_noop!(
					pallet_liquidity_pools::Pallet::<T>::transfer(
						RawOrigin::Signed(source_account.into()).into(),
						CurrencyId::Staking(cfg_types::tokens::StakingCurrency::BlockRewards),
						dest_address.clone(),
						amount,
					),
					pallet_liquidity_pools::Error::<T>::AssetNotFound
				);
				assert_noop!(
					pallet_liquidity_pools::Pallet::<T>::transfer(
						RawOrigin::Signed(source_account.into()).into(),
						CurrencyId::Native,
						dest_address.clone(),
						amount,
					),
					pallet_liquidity_pools::Error::<T>::AssetNotFound
				);

				// Cannot transfer as long as cross chain transferability is disabled
				assert_noop!(
					pallet_liquidity_pools::Pallet::<T>::transfer(
						RawOrigin::Signed(source_account.into()).into(),
						currency_id,
						dest_address.clone(),
						initial_balance,
					),
					pallet_liquidity_pools::Error::<T>::AssetNotLiquidityPoolsTransferable
				);

				// Enable LiquidityPools transferability
				enable_liquidity_pool_transferability::<T>(currency_id);

				// Cannot transfer more than owned
				assert_noop!(
					pallet_liquidity_pools::Pallet::<T>::transfer(
						RawOrigin::Signed(source_account.into()).into(),
						currency_id,
						dest_address.clone(),
						initial_balance.saturating_add(1),
					),
					pallet_liquidity_pools::Error::<T>::BalanceTooLow
				);

				let pre_total_issuance = orml_tokens::Pallet::<T>::total_issuance(currency_id);

				assert_ok!(pallet_liquidity_pools::Pallet::<T>::transfer(
					RawOrigin::Signed(source_account.into()).into(),
					currency_id,
					dest_address.clone(),
					amount,
				));

				assert_eq!(
					orml_tokens::Pallet::<T>::total_issuance(currency_id),
					pre_total_issuance - amount
				);
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(currency_id, &source_account.into()),
					initial_balance - amount
				);
			});
		}

		fn transfer_non_tranche_tokens_to_local<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.storage(),
			);

			setup_test(&mut env);

			env.parachain_state_mut(|| {
				let amount = DEFAULT_BALANCE_GLMR / 2;
				let currency_id = AUSD_CURRENCY_ID;
				let receiver: AccountId = Keyring::Bob.into();

				// Mock incoming decrease message
				let msg = LiquidityPoolMessage::Transfer {
					currency: general_currency_index::<T>(currency_id),
					// sender is irrelevant for other -> local
					sender: Keyring::Alice.into(),
					receiver: receiver.clone().into(),
					amount,
				};

				assert_eq!(orml_tokens::Pallet::<T>::total_issuance(currency_id), 0);

				// Finally, verify that we can now transfer the tranche to the destination
				// address
				assert_ok!(pallet_liquidity_pools::Pallet::<T>::submit(
					DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
					msg
				));

				// Verify that the correct amount was minted
				assert_eq!(
					orml_tokens::Pallet::<T>::total_issuance(currency_id),
					amount
				);
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(currency_id, &receiver),
					amount
				);

				// Verify empty transfers throw
				assert_noop!(
					pallet_liquidity_pools::Pallet::<T>::submit(
						DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
						LiquidityPoolMessage::Transfer {
							currency: general_currency_index::<T>(currency_id),
							sender: Keyring::Alice.into(),
							receiver: receiver.into(),
							amount: 0,
						},
					),
					pallet_liquidity_pools::Error::<T>::InvalidTransferAmount
				);
			});
		}

		fn transfer_tranche_tokens_from_local<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.storage(),
			);

			setup_test(&mut env);

			env.parachain_state_mut(|| {
				let pool_id = POOL_ID;
				let amount = 100_000;
				let dest_address: DomainAddress = DomainAddress::EVM(1284, [99; 20]);
				let receiver = Keyring::Bob;

				// Create the pool
				create_ausd_pool::<T>(pool_id);

				let tranche_tokens: CurrencyId = cfg_types::tokens::TrancheCurrency::generate(
					pool_id,
					default_tranche_id::<T>(pool_id),
				)
				.into();

				// Verify that we first need the destination address to be whitelisted
				assert_noop!(
					pallet_liquidity_pools::Pallet::<T>::transfer_tranche_tokens(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						pool_id,
						default_tranche_id::<T>(pool_id),
						dest_address.clone(),
						amount,
					),
					pallet_liquidity_pools::Error::<T>::UnauthorizedTransfer
				);

				// Make receiver the MembersListAdmin of this Pool
				assert_ok!(pallet_permissions::Pallet::<T>::add(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					Role::PoolRole(PoolRole::PoolAdmin),
					receiver.into(),
					PermissionScope::Pool(pool_id),
					Role::PoolRole(PoolRole::InvestorAdmin),
				));

				// Whitelist destination as TrancheInvestor of this Pool
				let valid_until = u64::MAX;

				crate::generic::utils::give_pool_role::<T>(
					AccountConverter::<T, LocationToAccountId>::convert(dest_address.clone()),
					pool_id,
					PoolRole::TrancheInvestor(default_tranche_id::<T>(pool_id), valid_until),
				);

				// Call the pallet_liquidity_pools::Pallet::<T>::update_member which ensures the
				// destination address is whitelisted.
				assert_ok!(pallet_liquidity_pools::Pallet::<T>::update_member(
					RawOrigin::Signed(receiver.into()).into(),
					pool_id,
					default_tranche_id::<T>(pool_id),
					dest_address.clone(),
					valid_until,
				));

				// Give receiver enough Tranche balance to be able to transfer it
				assert_ok!(orml_tokens::Pallet::<T>::deposit(
					tranche_tokens,
					&receiver.into(),
					amount
				));

				// Finally, verify that we can now transfer the tranche to the destination
				// address
				assert_ok!(
					pallet_liquidity_pools::Pallet::<T>::transfer_tranche_tokens(
						RawOrigin::Signed(receiver.into()).into(),
						pool_id,
						default_tranche_id::<T>(pool_id),
						dest_address.clone(),
						amount,
					)
				);

				// The account to which the tranche should have been transferred
				// to on Centrifuge for bookkeeping purposes.
				let domain_account: AccountId = Domain::convert(dest_address.domain());

				// Verify that the correct amount of the Tranche token was transferred
				// to the dest domain account on Centrifuge.
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(tranche_tokens, &domain_account),
					amount
				);
				assert!(
					orml_tokens::Pallet::<T>::free_balance(tranche_tokens, &receiver.into())
						.is_zero()
				);
			});
		}

		fn transfer_tranche_tokens_to_local<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.storage(),
			);

			setup_test(&mut env);

			env.parachain_state_mut(|| {
				// Create new pool
				let pool_id = POOL_ID;
				create_ausd_pool::<T>(pool_id);

				let amount = 100_000_000;
				let receiver: AccountId = Keyring::Bob.into();
				let sender: DomainAddress = DomainAddress::EVM(1284, [99; 20]);
				let sending_domain_locator =
					Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain());
				let tranche_id = default_tranche_id::<T>(pool_id);
				let tranche_tokens: CurrencyId =
					cfg_types::tokens::TrancheCurrency::generate(pool_id, tranche_id).into();
				let valid_until = u64::MAX;

				// Fund `DomainLocator` account of origination domain tranche tokens are
				// transferred from this account instead of minting
				assert_ok!(orml_tokens::Pallet::<T>::mint_into(
					tranche_tokens,
					&sending_domain_locator,
					amount
				));

				// Mock incoming decrease message
				let msg = LiquidityPoolMessage::TransferTrancheTokens {
					pool_id,
					tranche_id,
					sender: sender.address(),
					domain: Domain::Centrifuge,
					receiver: receiver.clone().into(),
					amount,
				};

				// Verify that we first need the receiver to be whitelisted
				assert_noop!(
					pallet_liquidity_pools::Pallet::<T>::submit(
						DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
						msg.clone()
					),
					pallet_liquidity_pools::Error::<T>::UnauthorizedTransfer
				);

				// Make receiver the MembersListAdmin of this Pool
				assert_ok!(pallet_permissions::Pallet::<T>::add(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					Role::PoolRole(PoolRole::PoolAdmin),
					receiver.clone(),
					PermissionScope::Pool(pool_id),
					Role::PoolRole(PoolRole::InvestorAdmin),
				));

				// Whitelist destination as TrancheInvestor of this Pool
				crate::generic::utils::give_pool_role::<T>(
					receiver.clone(),
					pool_id,
					PoolRole::TrancheInvestor(default_tranche_id::<T>(pool_id), valid_until),
				);

				// Finally, verify that we can now transfer the tranche to the destination
				// address
				assert_ok!(pallet_liquidity_pools::Pallet::<T>::submit(
					DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
					msg
				));

				// Verify that the correct amount of the Tranche token was transferred
				// to the dest domain account on Centrifuge.
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(tranche_tokens, &receiver),
					amount
				);
				assert!(orml_tokens::Pallet::<T>::free_balance(
					tranche_tokens,
					&sending_domain_locator
				)
				.is_zero());
			});
		}

		/// Try to transfer tranches for non-existing pools or invalid tranche
		/// ids for existing pools.
		fn transferring_invalid_tranche_tokens_should_fail<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.storage(),
			);

			setup_test(&mut env);

			env.parachain_state_mut(|| {
				let dest_address: DomainAddress = DomainAddress::EVM(1284, [99; 20]);

				let valid_pool_id: u64 = 42;
				create_ausd_pool::<T>(valid_pool_id);
				let valid_tranche_id = default_tranche_id::<T>(valid_pool_id);
				let valid_until = u64::MAX;
				let transfer_amount = 42;
				let invalid_pool_id = valid_pool_id + 1;
				let invalid_tranche_id = valid_tranche_id.map(|i| i.saturating_add(1));
				assert!(pallet_pool_system::Pallet::<T>::pool(invalid_pool_id).is_none());

				// Make Keyring::Bob the MembersListAdmin of both pools
				crate::generic::utils::give_pool_role::<T>(
					Keyring::Bob.into(),
					valid_pool_id,
					PoolRole::InvestorAdmin,
				);
				crate::generic::utils::give_pool_role::<T>(
					Keyring::Bob.into(),
					invalid_pool_id,
					PoolRole::InvestorAdmin,
				);

				// Give Keyring::Bob investor role for (valid_pool_id, invalid_tranche_id) and
				// (invalid_pool_id, valid_tranche_id)
				crate::generic::utils::give_pool_role::<T>(
					AccountConverter::<T, LocationToAccountId>::convert(dest_address.clone()),
					invalid_pool_id,
					PoolRole::TrancheInvestor(valid_tranche_id, valid_until),
				);
				crate::generic::utils::give_pool_role::<T>(
					AccountConverter::<T, LocationToAccountId>::convert(dest_address.clone()),
					valid_pool_id,
					PoolRole::TrancheInvestor(invalid_tranche_id, valid_until),
				);
				assert_noop!(
					pallet_liquidity_pools::Pallet::<T>::transfer_tranche_tokens(
						RawOrigin::Signed(Keyring::Bob.into()).into(),
						invalid_pool_id,
						valid_tranche_id,
						dest_address.clone(),
						transfer_amount
					),
					pallet_liquidity_pools::Error::<T>::PoolNotFound
				);
				assert_noop!(
					pallet_liquidity_pools::Pallet::<T>::transfer_tranche_tokens(
						RawOrigin::Signed(Keyring::Bob.into()).into(),
						valid_pool_id,
						invalid_tranche_id,
						dest_address,
						transfer_amount
					),
					pallet_liquidity_pools::Error::<T>::TrancheNotFound
				);
			});
		}

		fn transfer_cfg_to_sibling<T: Runtime + FudgeSupport>(env: &mut FudgeEnv<T>) {
			let alice_initial_balance = cfg(1_000);
			let transfer_amount = cfg(5);
			let cfg_in_sibling = CurrencyId::ForeignAsset(12);

			// CFG Metadata
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 18,
				name: "Development".into(),
				symbol: "CFG".into(),
				existential_deposit: 1_000_000_000_000,
				location: Some(VersionedMultiLocation::V3(MultiLocation::new(
					1,
					X2(
						Parachain(T::FudgeHandle::PARA_ID),
						general_key(parachains::polkadot::centrifuge::CFG_KEY),
					),
				))),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::Xcm(Default::default()),
					..CustomMetadata::default()
				},
			};

			env.parachain_state_mut(|| {
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					alice_initial_balance
				);
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&parachain_account(
						T::FudgeHandle::SIBLING_ID
					)),
					0
				);

				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta.clone(),
					Some(CurrencyId::Native),
				));
			});

			env.sibling_state_mut(|| {
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(cfg_in_sibling, &Keyring::Bob.into()),
					0
				);

				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta,
					Some(cfg_in_sibling)
				));
			});

			env.parachain_state_mut(|| {
				assert_ok!(pallet_restricted_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					CurrencyId::Native,
					transfer_amount,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Parachain(T::FudgeHandle::SIBLING_ID),
								Junction::AccountId32 {
									network: None,
									id: Keyring::Bob.into(),
								},
							),
						)
						.into()
					),
					WeightLimit::Limited(8_000_000_000_000.into()),
				));

				// Confirm that Keyring::Alice's balance is initial balance - amount transferred
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					alice_initial_balance - transfer_amount
				);

				// Verify that the amount transferred is now part of the sibling account here
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&parachain_account(
						T::FudgeHandle::SIBLING_ID
					)),
					transfer_amount
				);
			});

			env.pass(Blocks::ByNumber(1));

			env.sibling_state(|| {
				let current_balance =
					orml_tokens::Pallet::<T>::free_balance(cfg_in_sibling, &Keyring::Bob.into());

				// Verify that Keyring::Bob now has (amount transferred - fee)
				assert_eq!(current_balance, transfer_amount - fee(18));

				// Sanity check for the actual amount Keyring::Bob ends up with
				assert_eq!(current_balance, 4992960800000000000);
			});
		}

		fn transfer_cfg_to_and_from_sibling<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.storage(),
			);

			setup_test(&mut env);

			// In order to be able to transfer CFG from Moonbeam to Development, we need to
			// first send CFG from Development to Moonbeam, or else it fails since it'd be
			// like Moonbeam had minted CFG on their side.
			transfer_cfg_to_sibling::<T>(&mut env);

			let para_to_sibling_transfer_amount = cfg(5);

			let alice_balance = cfg(1_000) - para_to_sibling_transfer_amount;
			let bob_balance = para_to_sibling_transfer_amount - fee(18);
			let charlie_balance = cfg(1_000);

			let sibling_to_para_transfer_amount = cfg(4);
			// Note: This asset was registered in `transfer_cfg_to_sibling`
			let cfg_in_sibling = CurrencyId::ForeignAsset(12);

			env.parachain_state(|| {
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					alice_balance
				);
			});

			env.sibling_state_mut(|| {
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&parachain_account(
						T::FudgeHandle::PARA_ID
					)),
					0
				);

				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(cfg_in_sibling, &Keyring::Bob.into()),
					bob_balance
				);
			});

			env.sibling_state_mut(|| {
				assert_ok!(pallet_restricted_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					cfg_in_sibling,
					sibling_to_para_transfer_amount,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Parachain(T::FudgeHandle::PARA_ID),
								Junction::AccountId32 {
									network: None,
									id: Keyring::Charlie.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Limited(8_000_000_000_000.into()),
				));

				// Confirm that Charlie's balance is initial balance - amount transferred
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(cfg_in_sibling, &Keyring::Bob.into()),
					bob_balance - sibling_to_para_transfer_amount
				);
			});

			env.pass(Blocks::ByNumber(2));

			env.parachain_state(|| {
				// Verify that Charlie's balance equals the amount transferred - fee
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Into::<AccountId>::into(
						Keyring::Charlie
					)),
					charlie_balance + sibling_to_para_transfer_amount - cfg_fee(),
				);
			});
		}

		crate::test_for_runtimes!([development], transfer_non_tranche_tokens_from_local);
		crate::test_for_runtimes!([development], transfer_non_tranche_tokens_to_local);
		crate::test_for_runtimes!([development], transfer_tranche_tokens_from_local);
		crate::test_for_runtimes!([development], transfer_tranche_tokens_to_local);
		crate::test_for_runtimes!(
			[development],
			transferring_invalid_tranche_tokens_should_fail
		);
		crate::test_for_runtimes!([development], transfer_cfg_to_and_from_sibling);
	}

	mod routers {
		use super::*;

		mod axelar_evm {
			use std::ops::AddAssign;

			use super::*;

			mod utils {
				use super::*;

				pub fn mint_balance_into_derived_account<T: Runtime>(
					env: &mut impl Env<T>,
					address: H160,
					balance: u128,
				) {
					let chain_id = env.parachain_state(|| pallet_evm_chain_id::Pallet::<T>::get());

					let derived_account = AccountConverter::<T, ()>::convert_evm_address(
						chain_id,
						address.to_fixed_bytes(),
					);

					env.parachain_state_mut(|| {
						pallet_balances::Pallet::<T>::mint_into(&derived_account.into(), balance)
							.unwrap()
					});
				}
			}

			use utils::*;

			fn test_via_outbound_queue<T: Runtime + FudgeSupport>() {
				let mut env = FudgeEnv::<T>::from_parachain_storage(
					Genesis::<T>::default()
						.add(genesis::balances::<T>(cfg(1_000)))
						.add::<CouncilCollective>(genesis::council_members::<T, CouncilCollective>(
							get_council_members(),
						))
						.storage(),
				);

				let test_domain = Domain::EVM(1);

				let axelar_contract_address = H160::from_low_u64_be(1);
				let axelar_contract_code: Vec<u8> = vec![0, 0, 0];
				let axelar_contract_hash = BlakeTwo256::hash_of(&axelar_contract_code);
				let liquidity_pools_contract_address = H160::from_low_u64_be(2);

				env.parachain_state_mut(|| {
					pallet_evm::AccountCodes::<T>::insert(
						axelar_contract_address,
						axelar_contract_code,
					)
				});

				let transaction_call_cost = env
					.parachain_state(|| <T as pallet_evm::Config>::config().gas_transaction_call);

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

				let sender = Keyring::Alice.to_account_id();
				let gateway_sender = env.parachain_state(|| {
					<T as pallet_liquidity_pools_gateway::Config>::Sender::get()
				});

				let gateway_sender_h160: H160 = H160::from_slice(
					&<sp_core::crypto::AccountId32 as AsRef<[u8; 32]>>::as_ref(&gateway_sender)
						[0..20],
				);

				let msg = LiquidityPoolMessage::Transfer {
					currency: 0,
					sender: Keyring::Alice.to_account_id().into(),
					receiver: Keyring::Bob.to_account_id().into(),
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

				// Success

				// Note how both the target address and the gateway sender need to have some
				// balance.
				mint_balance_into_derived_account::<T>(
					&mut env,
					axelar_contract_address,
					cfg(1_000_000_000),
				);
				mint_balance_into_derived_account::<T>(
					&mut env,
					gateway_sender_h160,
					cfg(1_000_000),
				);

				assert_ok!(env.parachain_state_mut(|| {
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
					.expect("expected RouterExecutionSuccess event");

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

			crate::test_for_runtimes!([development], test_via_outbound_queue);
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

					let msg = Message::<Domain, PoolId, TrancheId, Balance, Quantity>::Transfer {
						currency: 0,
						sender: Keyring::Alice.into(),
						receiver: Keyring::Bob.into(),
						amount: 1_000u128,
					};

					env.parachain_state_mut(|| {
						let domain_router = router_creation_fn(
							MultiLocation {
								parents: 1,
								interior: X1(Parachain(T::FudgeHandle::SIBLING_ID)),
							}
							.into(),
							crate::utils::GLMR_CURRENCY_ID,
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
						.expect("expected RouterExecutionSuccess event");
				}

				type RouterCreationFn<T> =
					Box<dyn Fn(VersionedMultiLocation, CurrencyId) -> DomainRouter<T>>;

				pub fn get_axelar_xcm_router_fn<T: Runtime + FudgeSupport>() -> RouterCreationFn<T>
				{
					Box::new(
						|location: VersionedMultiLocation,
						 currency_id: CurrencyId|
						 -> DomainRouter<T> {
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

				pub fn get_ethereum_xcm_router_fn<T: Runtime + FudgeSupport>() -> RouterCreationFn<T>
				{
					Box::new(
						|location: VersionedMultiLocation,
						 currency_id: CurrencyId|
						 -> DomainRouter<T> {
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

			fn submit_ethereum_xcm<T: Runtime + FudgeSupport>() {
				submit_test_fn::<T>(get_ethereum_xcm_router_fn::<T>());
			}

			fn submit_axelar_xcm<T: Runtime + FudgeSupport>() {
				submit_test_fn::<T>(get_axelar_xcm_router_fn::<T>());
			}

			crate::test_for_runtimes!([development], submit_ethereum_xcm);
			crate::test_for_runtimes!([development], submit_axelar_xcm);
		}
	}

	mod gateway {
		use super::*;

		fn set_domain_router<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::<T>::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.add::<CouncilCollective>(genesis::council_members::<T, CouncilCollective>(
						get_council_members(),
					))
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

			let evm_domain = EVMDomain {
				target_contract_address: axelar_contract_address,
				target_contract_hash: axelar_contract_hash,
				fee_values: FeeValues {
					value: U256::from(10),
					gas_limit: U256::from(1_000_000),
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

			let set_domain_router_call =
				set_domain_router_call(test_domain.clone(), test_router.clone());

			let council_threshold = 2;
			let voting_period = 3;

			execute_via_democracy::<T>(
				&mut env,
				get_council_members(),
				set_domain_router_call,
				council_threshold,
				voting_period,
				0,
				0,
			);

			env.parachain_state(|| {
				let router =
					pallet_liquidity_pools_gateway::Pallet::<T>::domain_routers(test_domain)
						.expect("domain router is set");

				assert!(router.eq(&test_router));
			});
		}

		fn add_remove_instances<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::<T>::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.add::<CouncilCollective>(genesis::council_members::<T, CouncilCollective>(
						get_council_members(),
					))
					.storage(),
			);

			let test_instance = DomainAddress::EVM(1, [0; 20]);

			let add_instance_call = add_instance_call::<T>(test_instance.clone());

			let council_threshold = 2;
			let voting_period = 3;

			let (prop_index, ref_index) = execute_via_democracy::<T>(
				&mut env,
				get_council_members(),
				add_instance_call,
				council_threshold,
				voting_period,
				0,
				0,
			);

			env.parachain_state(|| {
				assert!(
					pallet_liquidity_pools_gateway::Allowlist::<T>::contains_key(
						test_instance.domain(),
						test_instance.clone()
					)
				);
			});

			let remove_instance_call = remove_instance_call::<T>(test_instance.clone());

			execute_via_democracy::<T>(
				&mut env,
				get_council_members(),
				remove_instance_call,
				council_threshold,
				voting_period,
				prop_index,
				ref_index,
			);

			env.parachain_state(|| {
				assert!(
					!pallet_liquidity_pools_gateway::Allowlist::<T>::contains_key(
						test_instance.domain(),
						test_instance.clone()
					)
				);
			});
		}

		fn process_msg<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::<T>::default()
					.add(genesis::balances::<T>(cfg(1_000)))
					.add::<CouncilCollective>(genesis::council_members::<T, CouncilCollective>(
						get_council_members(),
					))
					.storage(),
			);

			let test_instance = DomainAddress::EVM(1, [0; 20]);

			let add_instance_call = add_instance_call::<T>(test_instance.clone());

			let council_threshold = 2;
			let voting_period = 3;

			execute_via_democracy::<T>(
				&mut env,
				get_council_members(),
				add_instance_call,
				council_threshold,
				voting_period,
				0,
				0,
			);

			env.parachain_state(|| {
				assert!(
					pallet_liquidity_pools_gateway::Allowlist::<T>::contains_key(
						test_instance.domain(),
						test_instance.clone()
					)
				);
			});

			let msg = LiquidityPoolMessage::AddPool { pool_id: 123 };

			let encoded_msg = msg.serialize();

			let gateway_msg = BoundedVec::<
				u8,
				<T as pallet_liquidity_pools_gateway::Config>::MaxIncomingMessageSize,
			>::try_from(encoded_msg)
			.unwrap();

			env.parachain_state_mut(|| {
				assert_noop!(
					pallet_liquidity_pools_gateway::Pallet::<T>::process_msg(
						GatewayOrigin::Domain(test_instance).into(),
						gateway_msg,
					),
					pallet_liquidity_pools::Error::<T>::InvalidIncomingMessage,
				);
			});
		}

		crate::test_for_runtimes!([development], set_domain_router);
		crate::test_for_runtimes!([development], add_remove_instances);
		crate::test_for_runtimes!([development], process_msg);
	}
}

mod altair {
	use altair_runtime::{xcm::CurrencyIdConvert, PoolPalletIndex};

	pub const KSM_ASSET_ID: CurrencyId = CurrencyId::ForeignAsset(1000);

	use super::*;

	mod utils {
		use super::*;

		pub fn register_air<T: Runtime + FudgeSupport>() {
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 18,
				name: "Altair".into(),
				symbol: "AIR".into(),
				existential_deposit: 1_000_000_000_000,
				location: Some(VersionedMultiLocation::V3(MultiLocation::new(
					1,
					X2(
						Parachain(parachains::kusama::altair::ID),
						general_key(parachains::kusama::altair::AIR_KEY),
					),
				))),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::Xcm(Default::default()),
					..CustomMetadata::default()
				},
			};

			assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				meta,
				Some(CurrencyId::Native)
			));
		}

		pub fn register_ksm<T: Runtime + FudgeSupport>() {
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 12,
				name: "Kusama".into(),
				symbol: "KSM".into(),
				existential_deposit: 1_000_000_000,
				location: Some(VersionedMultiLocation::V3(MultiLocation::new(1, Here))),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::Xcm(Default::default()),
					..CustomMetadata::default()
				},
			};

			assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				meta,
				Some(KSM_ASSET_ID)
			));
		}

		pub fn air(amount: Balance) -> Balance {
			amount * decimals(currency_decimals::NATIVE)
		}

		pub fn ksm(amount: Balance) -> Balance {
			amount * decimals(currency_decimals::KSM)
		}

		pub fn foreign(amount: Balance, num_decimals: u32) -> Balance {
			amount * decimals(num_decimals)
		}

		pub fn air_fee() -> Balance {
			fee(currency_decimals::NATIVE)
		}

		// The fee associated with transferring KSM tokens
		pub fn ksm_fee() -> Balance {
			calc_fee(ksm_per_second())
		}
	}

	use utils::*;

	mod transfers {
		use super::*;

		fn transfer_air_to_sibling<T: Runtime + FudgeSupport>(env: &mut FudgeEnv<T>) {
			let alice_initial_balance = air(10);
			let transfer_amount = air(5);
			let air_in_sibling = CurrencyId::ForeignAsset(12);

			env.parachain_state_mut(|| {
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					alice_initial_balance
				);
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&parachain_account(
						T::FudgeHandle::SIBLING_ID
					)),
					0
				);

				// Register AIR as foreign asset in the sibling parachain
				let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
					decimals: 18,
					name: "Altair".into(),
					symbol: "AIR".into(),
					existential_deposit: 1_000_000_000_000,
					location: Some(VersionedMultiLocation::V3(MultiLocation::new(
						1,
						X2(
							Parachain(T::FudgeHandle::PARA_ID),
							general_key(parachains::kusama::altair::AIR_KEY),
						),
					))),
					additional: CustomMetadata {
						transferability: CrossChainTransferability::Xcm(Default::default()),
						..CustomMetadata::default()
					},
				};
				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta,
					Some(CurrencyId::Native)
				));
			});

			env.sibling_state_mut(|| {
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(air_in_sibling, &Keyring::Bob.into()),
					0
				);

				// Register AIR as foreign asset in the sibling parachain
				let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
					decimals: 18,
					name: "Altair".into(),
					symbol: "AIR".into(),
					existential_deposit: 1_000_000_000_000,
					location: Some(VersionedMultiLocation::V3(MultiLocation::new(
						1,
						X2(
							Parachain(T::FudgeHandle::PARA_ID),
							general_key(parachains::kusama::altair::AIR_KEY),
						),
					))),
					additional: CustomMetadata {
						transferability: CrossChainTransferability::Xcm(Default::default()),
						..CustomMetadata::default()
					},
				};
				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta,
					Some(air_in_sibling)
				));
			});

			env.pass(Blocks::ByNumber(1));

			env.parachain_state_mut(|| {
				assert_ok!(orml_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					CurrencyId::Native,
					transfer_amount,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Parachain(T::FudgeHandle::SIBLING_ID),
								Junction::AccountId32 {
									network: None,
									id: Keyring::Bob.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Limited(8_000_000_000_000.into()),
				));

				// Confirm that Alice's balance is initial balance - amount transferred
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					alice_initial_balance - transfer_amount
				);

				// Verify that the amount transferred is now part of the sibling account here
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&parachain_account(
						T::FudgeHandle::SIBLING_ID
					)),
					transfer_amount
				);
			});

			env.pass(Blocks::ByNumber(1));

			env.sibling_state_mut(|| {
				let current_balance =
					orml_tokens::Pallet::<T>::free_balance(air_in_sibling, &Keyring::Bob.into());

				// Verify that Keyring::Bob now has (amount transferred - fee)
				assert_eq!(current_balance, transfer_amount - fee(18));

				// Sanity check for the actual amount Keyring::Bob ends up with
				assert_eq!(current_balance, 4992960800000000000);
			});
		}

		fn test_air_transfers_to_and_from_sibling<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(air(10)))
					.storage(),
			);

			setup_xcm(&mut env);

			// In order to be able to transfer AIR from Sibling to Altair, we need to first
			// send AIR from Altair to Sibling, or else it fails since it'd be like Sibling
			// had minted AIR on their side.
			transfer_air_to_sibling(&mut env);

			let alice_initial_balance = air(5);
			let bob_initial_balance = air(5) - air_fee();
			let transfer_amount = air(1);

			// Note: This asset was registered in `transfer_air_to_sibling`
			let air_in_sibling = CurrencyId::ForeignAsset(12);

			env.parachain_state_mut(|| {
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					alice_initial_balance
				);
			});

			env.sibling_state_mut(|| {
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&parachain_account(
						T::FudgeHandle::PARA_ID
					)),
					0
				);
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(air_in_sibling, &Keyring::Bob.into()),
					bob_initial_balance
				);

				assert_ok!(orml_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					air_in_sibling,
					transfer_amount,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Parachain(T::FudgeHandle::PARA_ID),
								Junction::AccountId32 {
									network: None,
									id: Keyring::Alice.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Limited(8_000_000_000_000.into()),
				));

				// Confirm that Bobs's balance is initial balance - amount transferred
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(air_in_sibling, &Keyring::Bob.into()),
					bob_initial_balance - transfer_amount
				);
			});

			env.pass(Blocks::ByNumber(2));

			env.parachain_state_mut(|| {
				// Verify that Keyring::Alice now has initial balance + amount transferred - fee
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					alice_initial_balance + transfer_amount - air_fee(),
				);
			});
		}

		fn transfer_ausd_to_altair<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			setup_xcm(&mut env);

			let alice_initial_balance = ausd(10);
			let transfer_amount = ausd(7);

			env.sibling_state_mut(|| {
				register_ausd::<T>();

				assert_ok!(orml_tokens::Pallet::<T>::deposit(
					AUSD_CURRENCY_ID,
					&Keyring::Alice.into(),
					alice_initial_balance
				));

				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(
						AUSD_CURRENCY_ID,
						&parachain_account(T::FudgeHandle::PARA_ID)
					),
					0
				);
			});

			env.parachain_state_mut(|| {
				register_ausd::<T>();

				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(AUSD_CURRENCY_ID, &Keyring::Bob.into()),
					0,
				);
			});

			env.sibling_state_mut(|| {
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(
						AUSD_CURRENCY_ID,
						&Keyring::Alice.into()
					),
					ausd(10),
				);
				assert_ok!(orml_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					AUSD_CURRENCY_ID,
					transfer_amount,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Parachain(T::FudgeHandle::PARA_ID),
								Junction::AccountId32 {
									network: None,
									id: Keyring::Bob.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Limited(8_000_000_000_000.into()),
				));

				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(
						AUSD_CURRENCY_ID,
						&Keyring::Alice.into()
					),
					alice_initial_balance - transfer_amount
				);

				// Verify that the amount transferred is now part of the altair parachain
				// account here
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(
						AUSD_CURRENCY_ID,
						&parachain_account(T::FudgeHandle::PARA_ID)
					),
					transfer_amount
				);
			});

			env.pass(Blocks::ByNumber(2));

			env.parachain_state_mut(|| {
				// Verify that Keyring::Bob now has initial balance + amount transferred - fee
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(AUSD_CURRENCY_ID, &Keyring::Bob.into()),
					transfer_amount - ausd_fee()
				);
			});
		}

		fn transfer_ksm_from_relay_chain<T: Runtime + FudgeSupport>(
			env: &mut FudgeEnv<T>,
			transfer_amount: Balance,
			currency_id: CurrencyId,
			meta: AssetMetadata<Balance, CustomMetadata>,
		) {
			env.parachain_state_mut(|| {
				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta.clone(),
					Some(currency_id),
				));

				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(currency_id, &Keyring::Bob.into()),
					0
				);
			});

			env.relay_state_mut(|| {
				assert_ok!(
					pallet_balances::Pallet::<FudgeRelayRuntime<T>>::force_set_balance(
						<FudgeRelayRuntime<T> as frame_system::Config>::RuntimeOrigin::root(),
						Keyring::Alice.to_account_id().into(),
						transfer_amount * 2,
					)
				);

				assert_ok!(
					pallet_xcm::Pallet::<FudgeRelayRuntime<T>>::force_xcm_version(
						<FudgeRelayRuntime<T> as frame_system::Config>::RuntimeOrigin::root(),
						Box::new(MultiLocation::new(
							0,
							Junctions::X1(Junction::Parachain(T::FudgeHandle::PARA_ID)),
						)),
						XCM_VERSION,
					)
				);

				assert_ok!(
					pallet_xcm::Pallet::<FudgeRelayRuntime<T>>::reserve_transfer_assets(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						Box::new(Parachain(T::FudgeHandle::PARA_ID).into()),
						Box::new(
							Junction::AccountId32 {
								network: None,
								id: Keyring::Bob.into(),
							}
							.into()
						),
						Box::new((Here, transfer_amount).into()),
						0
					)
				);
			});

			env.pass(Blocks::ByNumber(1));

			env.parachain_state(|| {
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(currency_id, &Keyring::Bob.into()),
					transfer_amount - fee(meta.decimals)
				);
			});
		}

		fn transfer_ksm_to_and_from_relay_chain<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			let transfer_amount: Balance = ksm(2);
			let currency_id = CurrencyId::ForeignAsset(3001);
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 12,
				name: "Kusama".into(),
				symbol: "KSM".into(),
				existential_deposit: 1_000_000_000,
				location: Some(VersionedMultiLocation::V3(MultiLocation::new(1, Here))),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::Xcm(Default::default()),
					..CustomMetadata::default()
				},
			};

			// First we need some KSM on Altair
			transfer_ksm_from_relay_chain(&mut env, transfer_amount, currency_id, meta.clone());

			let currency_id = CurrencyId::ForeignAsset(3001);

			env.parachain_state_mut(|| {
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(currency_id, &Keyring::Bob.into()),
					transfer_amount - fee(meta.decimals)
				);

				assert_ok!(pallet_xcm::Pallet::<T>::force_xcm_version(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					Box::new(MultiLocation::new(1, Junctions::Here)),
					XCM_VERSION,
				));

				assert_ok!(orml_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					currency_id,
					ksm(1),
					Box::new(
						MultiLocation::new(
							1,
							X1(Junction::AccountId32 {
								id: Keyring::Bob.into(),
								network: None,
							})
						)
						.into()
					),
					WeightLimit::Limited(4_000_000_000.into())
				));
			});

			env.pass(Blocks::ByNumber(1));

			env.relay_state_mut(|| {
				assert_eq!(
					pallet_balances::Pallet::<FudgeRelayRuntime<T>>::free_balance(
						&Keyring::Bob.into()
					),
					999907996044
				);
			});
		}

		fn transfer_foreign_sibling_to_altair<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(air(10)))
					.storage(),
			);

			setup_xcm(&mut env);

			let sibling_asset_id = CurrencyId::ForeignAsset(1);
			let asset_location = MultiLocation::new(
				1,
				X2(Parachain(T::FudgeHandle::SIBLING_ID), general_key(&[0, 1])),
			);
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 18,
				name: "Sibling Native Token".into(),
				symbol: "SBLNG".into(),
				existential_deposit: 1_000_000_000_000,
				location: Some(VersionedMultiLocation::V3(asset_location)),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::Xcm(XcmMetadata {
						// We specify a custom fee_per_second and verify below that this value is
						// used when XCM transfer fees are charged for this token.
						fee_per_second: Some(8420000000000000000),
					}),
					..CustomMetadata::default()
				},
			};
			let transfer_amount = foreign(1, meta.decimals);

			env.sibling_state_mut(|| {
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(sibling_asset_id, &Keyring::Bob.into()),
					0
				);
				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta.clone(),
					Some(CurrencyId::Native),
				));
			});

			env.parachain_state_mut(|| {
				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta.clone(),
					Some(sibling_asset_id)
				));
			});

			env.sibling_state_mut(|| {
				assert_ok!(pallet_balances::Pallet::<T>::force_set_balance(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					Keyring::Alice.to_account_id().into(),
					transfer_amount * 2,
				));

				assert_ok!(orml_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					CurrencyId::Native,
					transfer_amount,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Parachain(T::FudgeHandle::PARA_ID),
								Junction::AccountId32 {
									network: None,
									id: Keyring::Bob.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Limited(8_000_000_000_000.into()),
				));

				// Confirm that Alice's balance is initial balance - amount transferred
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					transfer_amount
				);
			});

			env.pass(Blocks::ByNumber(2));

			env.parachain_state_mut(|| {
				let bob_balance =
					orml_tokens::Pallet::<T>::free_balance(sibling_asset_id, &Keyring::Bob.into());

				// Verify that Keyring::Bob now has initial balance + amount transferred - fee
				assert_eq!(
					bob_balance,
					transfer_amount
						- calc_fee(
							xcm_metadata(meta.additional.transferability)
								.unwrap()
								.fee_per_second
								.unwrap()
						)
				);
				// Sanity check to ensure the calculated is what is expected
				assert_eq!(bob_balance, 993264000000000000);
			});
		}

		fn transfer_wormhole_usdc_karura_to_altair<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(
				Default::default(),
				Default::default(),
				Genesis::default()
					.add(genesis::balances::<T>(air(10)))
					.storage(),
			);

			setup_xcm(&mut env);

			let usdc_asset_id = CurrencyId::ForeignAsset(39);
			let asset_location = MultiLocation::new(
				1,
				X2(
					Parachain(T::FudgeHandle::SIBLING_ID),
					general_key("0x02f3a00dd12f644daec907013b16eb6d14bf1c4cb4".as_bytes()),
				),
			);
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 6,
				name: "Wormhole USDC".into(),
				symbol: "WUSDC".into(),
				existential_deposit: 1,
				location: Some(VersionedMultiLocation::V3(asset_location)),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::Xcm(Default::default()),
					..CustomMetadata::default()
				},
			};
			let transfer_amount = foreign(12, meta.decimals);
			let alice_initial_balance = transfer_amount * 100;

			env.sibling_state_mut(|| {
				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta.clone(),
					Some(usdc_asset_id)
				));
				assert_ok!(orml_tokens::Pallet::<T>::deposit(
					usdc_asset_id,
					&Keyring::Alice.into(),
					alice_initial_balance
				));
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(usdc_asset_id, &Keyring::Alice.into()),
					alice_initial_balance
				);
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					air(10)
				);
			});

			env.parachain_state_mut(|| {
				// First, register the asset in centrifuge
				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta.clone(),
					Some(usdc_asset_id)
				));
			});

			env.sibling_state_mut(|| {
				assert_ok!(orml_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					usdc_asset_id,
					transfer_amount,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Parachain(T::FudgeHandle::PARA_ID),
								Junction::AccountId32 {
									network: None,
									id: Keyring::Bob.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Limited(8_000_000_000.into()),
				));

				// Confirm that Alice's balance is initial balance - amount transferred
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(usdc_asset_id, &Keyring::Alice.into()),
					alice_initial_balance - transfer_amount
				);
			});

			env.pass(Blocks::ByNumber(2));

			env.parachain_state_mut(|| {
				let bob_balance =
					orml_tokens::Pallet::<T>::free_balance(usdc_asset_id, &Keyring::Bob.into());

				// Sanity check to ensure the calculated is what is expected
				assert_eq!(bob_balance, 11992961);
			});
		}

		crate::test_for_runtimes!([altair], test_air_transfers_to_and_from_sibling);
		crate::test_for_runtimes!([altair], transfer_ausd_to_altair);
		crate::test_for_runtimes!([altair], transfer_ksm_to_and_from_relay_chain);
		crate::test_for_runtimes!([altair], transfer_foreign_sibling_to_altair);
		crate::test_for_runtimes!([altair], transfer_wormhole_usdc_karura_to_altair);
	}

	mod asset_registry {
		use super::*;

		fn register_air_works<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			env.parachain_state_mut(|| {
				let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
					decimals: 18,
					name: "Altair".into(),
					symbol: "AIR".into(),
					existential_deposit: 1_000_000_000_000,
					location: Some(VersionedMultiLocation::V3(MultiLocation::new(
						0,
						X1(general_key(parachains::kusama::altair::AIR_KEY)),
					))),
					additional: CustomMetadata {
						transferability: CrossChainTransferability::Xcm(Default::default()),
						..CustomMetadata::default()
					},
				};

				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta,
					Some(CurrencyId::Native)
				));
			});
		}

		fn register_foreign_asset_works<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			env.parachain_state_mut(|| {
				let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
					decimals: 12,
					name: "Acala Dollar".into(),
					symbol: "AUSD".into(),
					existential_deposit: 1_000_000_000_000,
					location: Some(VersionedMultiLocation::V3(MultiLocation::new(
						1,
						X2(
							Parachain(T::FudgeHandle::SIBLING_ID),
							general_key(parachains::kusama::karura::AUSD_KEY),
						),
					))),
					additional: CustomMetadata {
						transferability: CrossChainTransferability::Xcm(Default::default()),
						..CustomMetadata::default()
					},
				};

				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta,
					Some(CurrencyId::ForeignAsset(42))
				));
			});
		}

		// Verify that registering tranche tokens is not allowed through extrinsics
		fn register_tranche_asset_blocked<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			env.parachain_state_mut(|| {
				let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
					decimals: 12,
					name: "Tranche Token 1".into(),
					symbol: "TRNCH".into(),
					existential_deposit: 1_000_000_000_000,
					location: Some(VersionedMultiLocation::V3(MultiLocation::new(
						1,
						X2(Parachain(2000), general_key(&[42])),
					))),
					additional: CustomMetadata {
						transferability: CrossChainTransferability::Xcm(Default::default()),
						..CustomMetadata::default()
					},
				};

				// It fails with `BadOrigin` even when submitted with `Origin::root` since we
				// only allow for tranche tokens to be registered through the pools pallet.
				let asset_id = CurrencyId::Tranche(42, [42u8; 16]);
				assert_noop!(
					orml_asset_registry::Pallet::<T>::register_asset(
						<T as frame_system::Config>::RuntimeOrigin::root(),
						meta,
						Some(asset_id)
					),
					BadOrigin
				);
			});
		}

		crate::test_for_runtimes!([altair], register_air_works);
		crate::test_for_runtimes!([altair], register_foreign_asset_works);
		crate::test_for_runtimes!([altair], register_tranche_asset_blocked);
	}

	mod currency_id_convert {
		use super::*;

		fn convert_air<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			assert_eq!(parachains::kusama::altair::AIR_KEY.to_vec(), vec![0, 1]);

			env.parachain_state_mut(|| {
				// The way AIR is represented relative within the Altair runtime
				let air_location_inner: MultiLocation =
					MultiLocation::new(0, X1(general_key(parachains::kusama::altair::AIR_KEY)));

				// register air
				register_air::<T>();

				assert_eq!(
					<CurrencyIdConvert as C1<_, _>>::convert(air_location_inner),
					Ok(CurrencyId::Native),
				);

				// The canonical way AIR is represented out in the wild
				let air_location_canonical: MultiLocation = MultiLocation::new(
					1,
					X2(
						Parachain(T::FudgeHandle::PARA_ID),
						general_key(parachains::kusama::altair::AIR_KEY),
					),
				);

				assert_eq!(
					<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::Native),
					Some(air_location_canonical)
				)
			});
		}

		/// Verify that Tranche tokens are not handled by the CurrencyIdConvert
		/// since we don't allow Tranche tokens to be transferable through XCM.
		fn convert_tranche<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			let tranche_currency = CurrencyId::Tranche(401, [0; 16]);
			let tranche_id =
				WeakBoundedVec::<u8, ConstU32<32>>::force_from(tranche_currency.encode(), None);
			let tranche_multilocation = MultiLocation {
				parents: 1,
				interior: X3(
					Parachain(T::FudgeHandle::PARA_ID),
					PalletInstance(PoolPalletIndex::get()),
					GeneralKey {
						length: tranche_id.len() as u8,
						data: vec_to_fixed_array(tranche_id.to_vec()),
					},
				),
			};

			env.parachain_state_mut(|| {
				assert_eq!(
					<CurrencyIdConvert as C1<_, _>>::convert(tranche_multilocation),
					Err(tranche_multilocation),
				);
			});

			env.parachain_state_mut(|| {
				assert_eq!(
					<CurrencyIdConvert as C2<_, _>>::convert(tranche_currency),
					None
				)
			});
		}

		fn convert_ausd<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			env.parachain_state_mut(|| {
				assert_eq!(parachains::kusama::karura::AUSD_KEY, &[0, 129]);

				let ausd_location: MultiLocation = MultiLocation::new(
					1,
					X2(
						Parachain(T::FudgeHandle::SIBLING_ID),
						general_key(parachains::kusama::karura::AUSD_KEY),
					),
				);

				register_ausd::<T>();

				assert_eq!(
					<CurrencyIdConvert as C1<_, _>>::convert(ausd_location.clone()),
					Ok(AUSD_CURRENCY_ID),
				);

				assert_eq!(
					<CurrencyIdConvert as C2<_, _>>::convert(AUSD_CURRENCY_ID),
					Some(ausd_location)
				)
			});
		}

		fn convert_ksm<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			let ksm_location: MultiLocation = MultiLocation::parent().into();

			env.parachain_state_mut(|| {
				register_ksm::<T>();

				assert_eq!(
					<CurrencyIdConvert as C1<_, _>>::convert(ksm_location),
					Ok(KSM_ASSET_ID),
				);

				assert_eq!(
					<CurrencyIdConvert as C2<_, _>>::convert(KSM_ASSET_ID),
					Some(ksm_location)
				)
			});
		}

		fn convert_unkown_multilocation<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			let unknown_location: MultiLocation = MultiLocation::new(
				1,
				X2(Parachain(T::FudgeHandle::PARA_ID), general_key(&[42])),
			);

			env.parachain_state_mut(|| {
				assert!(<CurrencyIdConvert as C1<_, _>>::convert(unknown_location).is_err());
			});
		}

		fn convert_unsupported_currency<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			env.parachain_state_mut(|| {
				assert_eq!(
					<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::Tranche(
						0,
						[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
					)),
					None
				)
			});
		}

		crate::test_for_runtimes!([altair], convert_air);
		crate::test_for_runtimes!([altair], convert_tranche);
		crate::test_for_runtimes!([altair], convert_ausd);
		crate::test_for_runtimes!([altair], convert_ksm);
		crate::test_for_runtimes!([altair], convert_unkown_multilocation);
		crate::test_for_runtimes!([altair], convert_unsupported_currency);
	}
}

mod centrifuge {
	use centrifuge_runtime::xcm::CurrencyIdConvert;

	use super::*;

	mod utils {
		use xcm::v3::NetworkId;

		use super::*;

		/// The test asset id attributed to DOT
		pub const DOT_ASSET_ID: CurrencyId = CurrencyId::ForeignAsset(91);

		pub const LP_ETH_USDC: CurrencyId = CurrencyId::ForeignAsset(100_001);

		pub const USDC: CurrencyId = CurrencyId::ForeignAsset(6);

		/// An Asset that is NOT XCM transferable
		pub const NO_XCM_ASSET_ID: CurrencyId = CurrencyId::ForeignAsset(401);

		/// Register DOT in the asset registry.
		/// It should be executed within an externalities environment.
		pub fn register_dot<T: Runtime>() {
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 10,
				name: "Polkadot".into(),
				symbol: "DOT".into(),
				existential_deposit: 100_000_000,
				location: Some(VersionedMultiLocation::V3(MultiLocation::parent())),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::Xcm(Default::default()),
					..CustomMetadata::default()
				},
			};
			assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				meta,
				Some(DOT_ASSET_ID)
			));
		}

		pub fn register_lp_eth_usdc<T: Runtime>() {
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 6,
				name: "LP Ethereum Wrapped USDC".into(),
				symbol: "LpEthUSDC".into(),
				existential_deposit: 1_000,
				location: Some(VersionedMultiLocation::V3(MultiLocation::new(
					0,
					X3(
						PalletInstance(103),
						GlobalConsensus(NetworkId::Ethereum { chain_id: 1 }),
						AccountKey20 {
							network: None,
							key: hex_literal::hex!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
						},
					),
				))),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::LiquidityPools,
					..CustomMetadata::default()
				},
			};

			assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				meta,
				Some(LP_ETH_USDC)
			));
		}

		pub fn register_usdc<T: Runtime>() {
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 6,
				name: "USD Circle".into(),
				symbol: "USDC".into(),
				existential_deposit: 1_000,
				location: Some(VersionedMultiLocation::V3(MultiLocation::new(
					1,
					X3(
						Junction::Parachain(1000),
						Junction::PalletInstance(50),
						Junction::GeneralIndex(1337),
					),
				))),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::Xcm(Default::default()),
					..CustomMetadata::default()
				},
			};
			assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				meta,
				Some(USDC)
			));
		}

		/// Register CFG in the asset registry.
		/// It should be executed within an externalities environment.
		pub fn register_cfg<T: Runtime>(para_id: u32) {
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 18,
				name: "Centrifuge".into(),
				symbol: "CFG".into(),
				existential_deposit: 1_000_000_000_000,
				location: Some(VersionedMultiLocation::V3(MultiLocation::new(
					1,
					X2(
						Parachain(para_id),
						general_key(parachains::polkadot::centrifuge::CFG_KEY),
					),
				))),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::Xcm(Default::default()),
					..CustomMetadata::default()
				},
			};

			assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				meta,
				Some(CurrencyId::Native)
			));
		}

		/// Register CFG in the asset registry as XCM v2, just like it is in
		/// production. It should be executed within an externalities
		/// environment.
		pub fn register_cfg_v2<T: Runtime + FudgeSupport>() {
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 18,
				name: "Centrifuge".into(),
				symbol: "CFG".into(),
				existential_deposit: 1_000_000_000_000,
				location: Some(VersionedMultiLocation::V2(xcm::v2::MultiLocation::new(
					1,
					xcm::v2::Junctions::X2(
						xcm::v2::Junction::Parachain(T::FudgeHandle::PARA_ID),
						xcm::v2::Junction::GeneralKey(
							WeakBoundedVec::<u8, ConstU32<32>>::force_from(
								parachains::polkadot::centrifuge::CFG_KEY.into(),
								None,
							),
						),
					),
				))),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::Xcm(Default::default()),
					..CustomMetadata::default()
				},
			};

			assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				meta,
				Some(CurrencyId::Native)
			));
		}

		/// Register a token whose `CrossChainTransferability` does NOT include
		/// XCM.
		pub fn register_no_xcm_token<T: Runtime + FudgeSupport>() {
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 18,
				name: "NO XCM".into(),
				symbol: "NXCM".into(),
				existential_deposit: 1_000_000_000_000,
				location: None,
				additional: CustomMetadata {
					transferability: CrossChainTransferability::LiquidityPools,
					..CustomMetadata::default()
				},
			};

			assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				meta,
				Some(NO_XCM_ASSET_ID)
			));
		}

		// The fee associated with transferring DOT tokens
		pub fn dot_fee() -> Balance {
			fee(10)
		}

		pub fn lp_eth_usdc_fee() -> Balance {
			fee(6)
		}

		pub fn usdc_fee() -> Balance {
			fee(6)
		}

		pub fn dot(amount: Balance) -> Balance {
			amount * decimals(10)
		}

		pub fn lp_eth_usdc(amount: Balance) -> Balance {
			amount * decimals(6)
		}

		pub fn usdc(amount: Balance) -> Balance {
			amount * decimals(6)
		}

		pub fn foreign(amount: Balance, num_decimals: u32) -> Balance {
			amount * decimals(num_decimals)
		}

		pub fn transfer_dot_from_relay_chain<T: Runtime + FudgeSupport>(env: &mut FudgeEnv<T>) {
			let alice_initial_dot = dot(10);
			let transfer_amount: Balance = dot(3);

			env.parachain_state_mut(|| {
				register_dot::<T>();
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(DOT_ASSET_ID, &Keyring::Alice.into()),
					0
				);
			});

			env.relay_state_mut(|| {
				assert_ok!(
					pallet_balances::Pallet::<FudgeRelayRuntime<T>>::force_set_balance(
						<FudgeRelayRuntime<T> as frame_system::Config>::RuntimeOrigin::root(),
						Keyring::Alice.to_account_id().into(),
						alice_initial_dot,
					)
				);

				assert_ok!(
					pallet_xcm::Pallet::<FudgeRelayRuntime<T>>::force_xcm_version(
						<FudgeRelayRuntime<T> as frame_system::Config>::RuntimeOrigin::root(),
						Box::new(MultiLocation::new(
							0,
							Junctions::X1(Junction::Parachain(T::FudgeHandle::PARA_ID)),
						)),
						XCM_VERSION,
					)
				);

				assert_ok!(
					pallet_xcm::Pallet::<FudgeRelayRuntime<T>>::reserve_transfer_assets(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						Box::new(Parachain(T::FudgeHandle::PARA_ID).into()),
						Box::new(
							Junction::AccountId32 {
								network: None,
								id: Keyring::Alice.into(),
							}
							.into()
						),
						Box::new((Here, transfer_amount).into()),
						0
					)
				);

				assert_eq!(
					pallet_balances::Pallet::<FudgeRelayRuntime<T>>::free_balance(
						&Keyring::Alice.into()
					),
					alice_initial_dot - transfer_amount
				);
			});

			env.pass(Blocks::ByNumber(1));

			env.parachain_state(|| {
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(DOT_ASSET_ID, &Keyring::Alice.into()),
					transfer_amount - dot_fee()
				);
			});
		}
	}

	use utils::*;

	mod asset_registry {
		use super::*;

		fn register_cfg_works<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			env.parachain_state_mut(|| {
				let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
					decimals: 18,
					name: "Centrifuge".into(),
					symbol: "CFG".into(),
					existential_deposit: 1_000_000_000_000,
					location: Some(VersionedMultiLocation::V3(MultiLocation::new(
						0,
						X1(general_key(parachains::polkadot::centrifuge::CFG_KEY)),
					))),
					additional: CustomMetadata {
						transferability: CrossChainTransferability::Xcm(Default::default()),
						..CustomMetadata::default()
					},
				};

				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta,
					Some(CurrencyId::Native)
				));
			});
		}

		fn register_foreign_asset_works<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			env.parachain_state_mut(|| {
				let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
					decimals: 12,
					name: "Acala Dollar".into(),
					symbol: "AUSD".into(),
					existential_deposit: 1_000_000_000_000,
					location: Some(VersionedMultiLocation::V3(MultiLocation::new(
						1,
						X2(
							Parachain(parachains::polkadot::acala::ID),
							general_key(parachains::polkadot::acala::AUSD_KEY),
						),
					))),
					additional: CustomMetadata {
						transferability: CrossChainTransferability::Xcm(Default::default()),
						..CustomMetadata::default()
					},
				};

				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta,
					Some(CurrencyId::ForeignAsset(42))
				));
			});
		}

		// Verify that registering tranche tokens is not allowed through extrinsics
		fn register_tranche_asset_blocked<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			env.parachain_state_mut(|| {
				let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
					decimals: 12,
					name: "Tranche Token 1".into(),
					symbol: "TRNCH".into(),
					existential_deposit: 1_000_000_000_000,
					location: Some(VersionedMultiLocation::V3(MultiLocation::new(
						1,
						X2(Parachain(2000), general_key(&[42])),
					))),
					additional: CustomMetadata {
						transferability: CrossChainTransferability::Xcm(Default::default()),
						..CustomMetadata::default()
					},
				};

				// It fails with `BadOrigin` even when submitted with `Origin::root` since we
				// only allow for tranche tokens to be registered through the pools pallet.
				let asset_id = CurrencyId::Tranche(42, [42u8; 16]);
				assert_noop!(
					orml_asset_registry::Pallet::<T>::register_asset(
						<T as frame_system::Config>::RuntimeOrigin::root(),
						meta,
						Some(asset_id)
					),
					BadOrigin
				);
			});
		}

		crate::test_for_runtimes!([centrifuge], register_cfg_works);
		crate::test_for_runtimes!([centrifuge], register_foreign_asset_works);
		crate::test_for_runtimes!([centrifuge], register_tranche_asset_blocked);
	}

	mod currency_id_convert {
		use super::*;

		fn convert_cfg<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			assert_eq!(parachains::polkadot::centrifuge::CFG_KEY, &[0, 1]);

			env.parachain_state_mut(|| {
				// The way CFG is represented relative within the Centrifuge runtime
				let cfg_location_inner: MultiLocation = MultiLocation::new(
					0,
					X1(general_key(parachains::polkadot::centrifuge::CFG_KEY)),
				);

				register_cfg::<T>(T::FudgeHandle::PARA_ID);

				assert_eq!(
					<CurrencyIdConvert as C1<_, _>>::convert(cfg_location_inner),
					Ok(CurrencyId::Native),
				);

				// The canonical way CFG is represented out in the wild
				let cfg_location_canonical: MultiLocation = MultiLocation::new(
					1,
					X2(
						Parachain(parachains::polkadot::centrifuge::ID),
						general_key(parachains::polkadot::centrifuge::CFG_KEY),
					),
				);

				assert_eq!(
					<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::Native),
					Some(cfg_location_canonical)
				)
			});
		}

		/// Verify that even with CFG registered in the AssetRegistry with a XCM
		/// v2 MultiLocation, that `CurrencyIdConvert` can look it up given an
		/// identical location in XCM v3.
		fn convert_cfg_xcm_v2<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			assert_eq!(parachains::polkadot::centrifuge::CFG_KEY, &[0, 1]);

			env.parachain_state_mut(|| {
				// Registered as xcm v2
				register_cfg_v2::<T>();

				// The way CFG is represented relative within the Centrifuge runtime in xcm v3
				let cfg_location_inner: MultiLocation = MultiLocation::new(
					0,
					X1(general_key(parachains::polkadot::centrifuge::CFG_KEY)),
				);

				assert_eq!(
					<CurrencyIdConvert as C1<_, _>>::convert(cfg_location_inner),
					Ok(CurrencyId::Native),
				);

				// The canonical way CFG is represented out in the wild
				let cfg_location_canonical: MultiLocation = MultiLocation::new(
					1,
					X2(
						Parachain(parachains::polkadot::centrifuge::ID),
						general_key(parachains::polkadot::centrifuge::CFG_KEY),
					),
				);

				assert_eq!(
					<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::Native),
					Some(cfg_location_canonical)
				)
			});
		}

		/// Verify that a registered token that is NOT XCM transferable is
		/// filtered out by CurrencyIdConvert as expected.
		fn convert_no_xcm_token<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			env.parachain_state_mut(|| {
				register_no_xcm_token::<T>();

				assert_eq!(
					<CurrencyIdConvert as C2<_, _>>::convert(NO_XCM_ASSET_ID),
					None
				)
			});
		}

		fn convert_dot<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			let dot_location: MultiLocation = MultiLocation::parent();

			env.parachain_state_mut(|| {
				register_dot::<T>();

				assert_eq!(
					<CurrencyIdConvert as C1<_, _>>::convert(dot_location),
					Ok(DOT_ASSET_ID),
				);

				assert_eq!(
					<CurrencyIdConvert as C2<_, _>>::convert(DOT_ASSET_ID),
					Some(dot_location)
				)
			});
		}

		fn convert_unknown_multilocation<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			let unknown_location: MultiLocation = MultiLocation::new(
				1,
				X2(
					Parachain(T::FudgeHandle::PARA_ID),
					general_key([42].as_ref()),
				),
			);

			env.parachain_state_mut(|| {
				assert!(<CurrencyIdConvert as C1<_, _>>::convert(unknown_location).is_err());
			});
		}

		fn convert_unsupported_currency<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			env.parachain_state_mut(|| {
				assert_eq!(
					<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::Tranche(
						0,
						[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
					)),
					None
				)
			});
		}

		crate::test_for_runtimes!([centrifuge], convert_cfg);
		crate::test_for_runtimes!([centrifuge], convert_cfg_xcm_v2);
		crate::test_for_runtimes!([centrifuge], convert_no_xcm_token);
		crate::test_for_runtimes!([centrifuge], convert_dot);
		crate::test_for_runtimes!([centrifuge], convert_unknown_multilocation);
		crate::test_for_runtimes!([centrifuge], convert_unsupported_currency);
	}

	mod restricted_transfers {
		use cfg_types::tokens::{CurrencyId::Native, FilterCurrency};

		use super::*;
		use crate::generic::envs::runtime_env::RuntimeEnv;

		const TRANSFER_AMOUNT: u128 = 10;

		fn xcm_location() -> MultiLocation {
			MultiLocation::new(
				1,
				X1(AccountId32 {
					id: Keyring::Alice.into(),
					network: None,
				}),
			)
		}

		fn allowed_xcm_location() -> Location {
			Location::XCM(BlakeTwo256::hash(&xcm_location().encode()))
		}

		fn add_allowance<T: Runtime>(account: Keyring, asset: CurrencyId, location: Location) {
			assert_ok!(
				pallet_transfer_allowlist::Pallet::<T>::add_transfer_allowance(
					RawOrigin::Signed(account.into()).into(),
					FilterCurrency::Specific(asset),
					location
				)
			);
		}

		#[test]
		fn _test() {
			restrict_cfg_extrinsic::<crate::chain::centrifuge::Runtime>()
		}

		fn restrict_cfg_extrinsic<T: Runtime>() {
			let mut env = RuntimeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(TRANSFER_AMOUNT + 10)))
					.add(orml_tokens::GenesisConfig::<T> {
						balances: vec![(
							Keyring::Alice.to_account_id(),
							USDC,
							T::ExistentialDeposit::get() + usdc(TRANSFER_AMOUNT),
						)],
					})
					.storage(),
			);

			let (pre_transfer_alice, pre_transfer_bob, pre_transfer_charlie) = env
				.parachain_state_mut(|| {
					// NOTE: The para-id is not relevant here
					register_cfg::<T>(2031);

					assert_ok!(
						pallet_transfer_allowlist::Pallet::<T>::add_transfer_allowance(
							RawOrigin::Signed(Keyring::Alice.into()).into(),
							FilterCurrency::All,
							Location::Local(Keyring::Bob.to_account_id())
						)
					);

					(
						pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.to_account_id()),
						pallet_balances::Pallet::<T>::free_balance(&Keyring::Bob.to_account_id()),
						pallet_balances::Pallet::<T>::free_balance(
							&Keyring::Charlie.to_account_id(),
						),
					)
				});

			let call = pallet_balances::Call::<T>::transfer {
				dest: Keyring::Charlie.into(),
				value: cfg(TRANSFER_AMOUNT),
			};
			env.submit_now(Keyring::Alice, call).unwrap();

			let call = pallet_balances::Call::<T>::transfer {
				dest: Keyring::Bob.into(),
				value: cfg(TRANSFER_AMOUNT),
			};
			let fee = env.submit_now(Keyring::Alice, call).unwrap();

			// Restrict also CFG local
			env.parachain_state(|| {
				let after_transfer_alice =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.to_account_id());
				let after_transfer_bob =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Bob.to_account_id());
				let after_transfer_charlie =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Charlie.to_account_id());

				assert_eq!(
					after_transfer_alice,
					pre_transfer_alice - cfg(TRANSFER_AMOUNT) - 2 * fee
				);
				assert_eq!(after_transfer_bob, pre_transfer_bob + cfg(TRANSFER_AMOUNT));
				assert_eq!(after_transfer_charlie, pre_transfer_charlie);
			});
		}

		fn restrict_all<T: Runtime>() {
			let mut env = RuntimeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(TRANSFER_AMOUNT + 10)))
					.add(orml_tokens::GenesisConfig::<T> {
						balances: vec![(
							Keyring::Alice.to_account_id(),
							USDC,
							T::ExistentialDeposit::get() + usdc(TRANSFER_AMOUNT),
						)],
					})
					.storage(),
			);

			// Set allowance
			env.parachain_state_mut(|| {
				assert_ok!(
					pallet_transfer_allowlist::Pallet::<T>::add_transfer_allowance(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						FilterCurrency::All,
						Location::Local(Keyring::Bob.to_account_id())
					)
				);
			});

			// Restrict USDC local
			env.parachain_state_mut(|| {
				register_usdc::<T>();

				let pre_transfer_alice =
					orml_tokens::Pallet::<T>::free_balance(USDC, &Keyring::Alice.to_account_id());
				let pre_transfer_bob =
					orml_tokens::Pallet::<T>::free_balance(USDC, &Keyring::Bob.to_account_id());
				let pre_transfer_charlie =
					orml_tokens::Pallet::<T>::free_balance(USDC, &Keyring::Charlie.to_account_id());

				assert_noop!(
					pallet_restricted_tokens::Pallet::<T>::transfer(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						Keyring::Charlie.into(),
						USDC,
						lp_eth_usdc(TRANSFER_AMOUNT)
					),
					pallet_restricted_tokens::Error::<T>::PreConditionsNotMet
				);

				let after_transfer_alice =
					orml_tokens::Pallet::<T>::free_balance(USDC, &Keyring::Alice.to_account_id());
				let after_transfer_charlie =
					orml_tokens::Pallet::<T>::free_balance(USDC, &Keyring::Charlie.to_account_id());

				assert_eq!(after_transfer_alice, pre_transfer_alice);
				assert_eq!(after_transfer_charlie, pre_transfer_charlie);

				assert_ok!(pallet_restricted_tokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					Keyring::Bob.into(),
					USDC,
					usdc(TRANSFER_AMOUNT)
				),);

				let after_transfer_alice =
					orml_tokens::Pallet::<T>::free_balance(USDC, &Keyring::Alice.to_account_id());
				let after_transfer_bob =
					orml_tokens::Pallet::<T>::free_balance(USDC, &Keyring::Bob.to_account_id());
				let after_transfer_charlie =
					orml_tokens::Pallet::<T>::free_balance(USDC, &Keyring::Charlie.to_account_id());

				assert_eq!(
					after_transfer_alice,
					pre_transfer_alice - usdc(TRANSFER_AMOUNT)
				);
				assert_eq!(after_transfer_bob, pre_transfer_bob + usdc(TRANSFER_AMOUNT));
				assert_eq!(after_transfer_charlie, pre_transfer_charlie);
			});

			// Restrict also CFG local
			env.parachain_state_mut(|| {
				// NOTE: The para-id is not relevant here
				register_cfg::<T>(2031);

				let pre_transfer_alice =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.to_account_id());
				let pre_transfer_bob =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Bob.to_account_id());
				let pre_transfer_charlie =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Charlie.to_account_id());

				assert_noop!(
					pallet_restricted_tokens::Pallet::<T>::transfer(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						Keyring::Charlie.into(),
						Native,
						cfg(TRANSFER_AMOUNT)
					),
					pallet_restricted_tokens::Error::<T>::PreConditionsNotMet
				);

				let after_transfer_alice =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.to_account_id());
				let after_transfer_charlie =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Charlie.to_account_id());

				assert_eq!(after_transfer_alice, pre_transfer_alice);
				assert_eq!(after_transfer_charlie, pre_transfer_charlie);

				assert_ok!(pallet_restricted_tokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					Keyring::Bob.into(),
					Native,
					cfg(TRANSFER_AMOUNT)
				),);

				let after_transfer_alice =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.to_account_id());
				let after_transfer_bob =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Bob.to_account_id());
				let after_transfer_charlie =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Charlie.to_account_id());

				assert_eq!(
					after_transfer_alice,
					pre_transfer_alice - cfg(TRANSFER_AMOUNT)
				);
				assert_eq!(after_transfer_bob, pre_transfer_bob + cfg(TRANSFER_AMOUNT));
				assert_eq!(after_transfer_charlie, pre_transfer_charlie);
			});
		}

		fn restrict_lp_eth_usdc_transfer<T: Runtime>() {
			let mut env = RuntimeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(10)))
					.add(orml_tokens::GenesisConfig::<T> {
						balances: vec![(
							Keyring::Alice.to_account_id(),
							LP_ETH_USDC,
							T::ExistentialDeposit::get() + lp_eth_usdc(TRANSFER_AMOUNT),
						)],
					})
					.storage(),
			);

			env.parachain_state_mut(|| {
				register_lp_eth_usdc::<T>();

				let pre_transfer_alice = orml_tokens::Pallet::<T>::free_balance(
					LP_ETH_USDC,
					&Keyring::Alice.to_account_id(),
				);
				let pre_transfer_bob = orml_tokens::Pallet::<T>::free_balance(
					LP_ETH_USDC,
					&Keyring::Bob.to_account_id(),
				);
				let pre_transfer_charlie = orml_tokens::Pallet::<T>::free_balance(
					LP_ETH_USDC,
					&Keyring::Charlie.to_account_id(),
				);

				add_allowance::<T>(
					Keyring::Alice,
					LP_ETH_USDC,
					Location::Local(Keyring::Bob.to_account_id()),
				);

				assert_noop!(
					pallet_restricted_tokens::Pallet::<T>::transfer(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						Keyring::Charlie.into(),
						LP_ETH_USDC,
						lp_eth_usdc(TRANSFER_AMOUNT)
					),
					pallet_restricted_tokens::Error::<T>::PreConditionsNotMet
				);

				let after_transfer_alice = orml_tokens::Pallet::<T>::free_balance(
					LP_ETH_USDC,
					&Keyring::Alice.to_account_id(),
				);
				let after_transfer_charlie = orml_tokens::Pallet::<T>::free_balance(
					LP_ETH_USDC,
					&Keyring::Charlie.to_account_id(),
				);

				assert_eq!(after_transfer_alice, pre_transfer_alice);
				assert_eq!(after_transfer_charlie, pre_transfer_charlie);

				assert_ok!(pallet_restricted_tokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					Keyring::Bob.into(),
					LP_ETH_USDC,
					lp_eth_usdc(TRANSFER_AMOUNT)
				),);

				let after_transfer_alice = orml_tokens::Pallet::<T>::free_balance(
					LP_ETH_USDC,
					&Keyring::Alice.to_account_id(),
				);
				let after_transfer_bob = orml_tokens::Pallet::<T>::free_balance(
					LP_ETH_USDC,
					&Keyring::Bob.to_account_id(),
				);
				let after_transfer_charlie = orml_tokens::Pallet::<T>::free_balance(
					LP_ETH_USDC,
					&Keyring::Charlie.to_account_id(),
				);

				assert_eq!(
					after_transfer_alice,
					pre_transfer_alice - lp_eth_usdc(TRANSFER_AMOUNT)
				);
				assert_eq!(
					after_transfer_bob,
					pre_transfer_bob + lp_eth_usdc(TRANSFER_AMOUNT)
				);
				assert_eq!(after_transfer_charlie, pre_transfer_charlie);
			});
		}

		fn restrict_lp_eth_usdc_lp_transfer<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(10)))
					.add(orml_tokens::GenesisConfig::<T> {
						balances: vec![(
							Keyring::Alice.to_account_id(),
							LP_ETH_USDC,
							T::ExistentialDeposit::get() + lp_eth_usdc(TRANSFER_AMOUNT),
						)],
					})
					.storage(),
			);

			setup_xcm(&mut env);

			env.parachain_state_mut(|| {
				register_usdc::<T>();
				register_lp_eth_usdc::<T>();

				assert_ok!(orml_tokens::Pallet::<T>::set_balance(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					<T as pallet_liquidity_pools_gateway::Config>::Sender::get().into(),
					USDC,
					usdc(1_000),
					0,
				));

				let router = DomainRouter::EthereumXCM(EthereumXCMRouter::<T> {
					router: XCMRouter {
						xcm_domain: XcmDomain {
							location: Box::new(
								MultiLocation::new(1, X1(Parachain(T::FudgeHandle::SIBLING_ID)))
									.into(),
							),
							ethereum_xcm_transact_call_index: BoundedVec::truncate_from(vec![
								38, 0,
							]),
							contract_address: H160::from_low_u64_be(11),
							max_gas_limit: 700_000,
							transact_required_weight_at_most: Default::default(),
							overall_weight: Default::default(),
							fee_currency: USDC,
							fee_amount: usdc(1),
						},
						_marker: Default::default(),
					},
					_marker: Default::default(),
				});

				assert_ok!(
					pallet_liquidity_pools_gateway::Pallet::<T>::set_domain_router(
						<T as frame_system::Config>::RuntimeOrigin::root(),
						Domain::EVM(1),
						router,
					)
				);

				let receiver = H160::from_slice(
					&<sp_runtime::AccountId32 as AsRef<[u8; 32]>>::as_ref(
						&Keyring::Charlie.to_account_id(),
					)[0..20],
				);

				let domain_address = DomainAddress::EVM(1, receiver.into());

				add_allowance::<T>(
					Keyring::Alice,
					LP_ETH_USDC,
					Location::Address(domain_address.clone()),
				);

				assert_noop!(
					pallet_liquidity_pools::Pallet::<T>::transfer(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						LP_ETH_USDC,
						DomainAddress::EVM(1, [1u8; 20]),
						lp_eth_usdc(TRANSFER_AMOUNT),
					),
					pallet_transfer_allowlist::Error::<T>::NoAllowanceForDestination
				);

				let total_issuance_pre = orml_tokens::Pallet::<T>::total_issuance(LP_ETH_USDC);

				assert_ok!(pallet_liquidity_pools::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					LP_ETH_USDC,
					domain_address,
					lp_eth_usdc(TRANSFER_AMOUNT),
				));

				assert_eq!(
					orml_tokens::Pallet::<T>::total_issuance(LP_ETH_USDC),
					total_issuance_pre - lp_eth_usdc(TRANSFER_AMOUNT),
				);
			});
		}

		fn restrict_usdc_transfer<T: Runtime>() {
			let mut env = RuntimeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(10)))
					.add(orml_tokens::GenesisConfig::<T> {
						balances: vec![(
							Keyring::Alice.to_account_id(),
							USDC,
							T::ExistentialDeposit::get() + usdc(TRANSFER_AMOUNT),
						)],
					})
					.storage(),
			);

			env.parachain_state_mut(|| {
				register_usdc::<T>();

				let pre_transfer_alice =
					orml_tokens::Pallet::<T>::free_balance(USDC, &Keyring::Alice.to_account_id());
				let pre_transfer_bob =
					orml_tokens::Pallet::<T>::free_balance(USDC, &Keyring::Bob.to_account_id());
				let pre_transfer_charlie =
					orml_tokens::Pallet::<T>::free_balance(USDC, &Keyring::Charlie.to_account_id());

				add_allowance::<T>(
					Keyring::Alice,
					USDC,
					Location::Local(Keyring::Bob.to_account_id()),
				);

				assert_noop!(
					pallet_restricted_tokens::Pallet::<T>::transfer(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						Keyring::Charlie.into(),
						USDC,
						lp_eth_usdc(TRANSFER_AMOUNT)
					),
					pallet_restricted_tokens::Error::<T>::PreConditionsNotMet
				);

				let after_transfer_alice =
					orml_tokens::Pallet::<T>::free_balance(USDC, &Keyring::Alice.to_account_id());
				let after_transfer_charlie =
					orml_tokens::Pallet::<T>::free_balance(USDC, &Keyring::Charlie.to_account_id());

				assert_eq!(after_transfer_alice, pre_transfer_alice);
				assert_eq!(after_transfer_charlie, pre_transfer_charlie);

				assert_ok!(pallet_restricted_tokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					Keyring::Bob.into(),
					USDC,
					usdc(TRANSFER_AMOUNT)
				),);

				let after_transfer_alice =
					orml_tokens::Pallet::<T>::free_balance(USDC, &Keyring::Alice.to_account_id());
				let after_transfer_bob =
					orml_tokens::Pallet::<T>::free_balance(USDC, &Keyring::Bob.to_account_id());
				let after_transfer_charlie =
					orml_tokens::Pallet::<T>::free_balance(USDC, &Keyring::Charlie.to_account_id());

				assert_eq!(
					after_transfer_alice,
					pre_transfer_alice - usdc(TRANSFER_AMOUNT)
				);
				assert_eq!(after_transfer_bob, pre_transfer_bob + usdc(TRANSFER_AMOUNT));
				assert_eq!(after_transfer_charlie, pre_transfer_charlie);
			});
		}

		fn restrict_usdc_xcm_transfer<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(
				<paras::GenesisConfig as BuildStorage<FudgeRelayRuntime<T>>>::build_storage(
					&paras::GenesisConfig {
						paras: vec![(
							1000.into(),
							ParaGenesisArgs {
								genesis_head: Default::default(),
								validation_code: ValidationCode::from(vec![0, 1, 2, 3]),
								para_kind: ParaKind::Parachain,
							},
						)],
					},
				)
				.unwrap(),
				Genesis::default()
					.add(genesis::balances::<T>(cfg(10)))
					.storage(),
				Default::default(),
			);

			setup_xcm(&mut env);

			setup_usdc_xcm(&mut env);

			env.sibling_state_mut(|| {
				register_usdc::<T>();
			});

			env.parachain_state_mut(|| {
				register_usdc::<T>();

				let alice_initial_usdc = usdc(3_000);

				assert_ok!(orml_tokens::Pallet::<T>::mint_into(
					USDC,
					&Keyring::Alice.into(),
					alice_initial_usdc
				));

				assert_ok!(
					pallet_transfer_allowlist::Pallet::<T>::add_transfer_allowance(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						FilterCurrency::Specific(USDC),
						Location::XCM(BlakeTwo256::hash(
							&MultiLocation::new(
								1,
								X2(
									Parachain(T::FudgeHandle::SIBLING_ID),
									Junction::AccountId32 {
										id: Keyring::Alice.into(),
										network: None,
									}
								)
							)
							.encode()
						))
					)
				);

				assert_noop!(
					pallet_restricted_xtokens::Pallet::<T>::transfer(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						USDC,
						usdc(1_000),
						Box::new(
							MultiLocation::new(
								1,
								X2(
									Parachain(T::FudgeHandle::SIBLING_ID),
									Junction::AccountId32 {
										id: Keyring::Bob.into(),
										network: None,
									}
								)
							)
							.into()
						),
						WeightLimit::Unlimited,
					),
					pallet_transfer_allowlist::Error::<T>::NoAllowanceForDestination
				);

				assert_ok!(pallet_restricted_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					USDC,
					usdc(1_000),
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Parachain(T::FudgeHandle::SIBLING_ID),
								Junction::AccountId32 {
									id: Keyring::Alice.into(),
									network: None,
								}
							)
						)
						.into()
					),
					WeightLimit::Unlimited,
				));

				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(USDC, &Keyring::Alice.into()),
					alice_initial_usdc - usdc(1_000),
				);
			});

			// NOTE - we cannot confirm that the Alice account present on the
			// sibling receives this transfer since the orml_xtokens pallet
			// sends a message to parachain 1000 (the parachain of the USDC
			// currency) which in turn should send a message to the sibling.
			// Since parachain 1000 is just a dummy added in the paras
			// genesis config and not an actual sibling with a runtime, the
			// transfer does not take place.
		}

		fn restrict_dot_transfer<T: Runtime>() {
			let mut env = RuntimeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(10)))
					.add(orml_tokens::GenesisConfig::<T> {
						balances: vec![(
							Keyring::Alice.to_account_id(),
							DOT_ASSET_ID,
							T::ExistentialDeposit::get() + dot(TRANSFER_AMOUNT),
						)],
					})
					.storage(),
			);

			env.parachain_state_mut(|| {
				register_dot::<T>();

				let pre_transfer_alice = orml_tokens::Pallet::<T>::free_balance(
					DOT_ASSET_ID,
					&Keyring::Alice.to_account_id(),
				);
				let pre_transfer_bob = orml_tokens::Pallet::<T>::free_balance(
					DOT_ASSET_ID,
					&Keyring::Bob.to_account_id(),
				);
				let pre_transfer_charlie = orml_tokens::Pallet::<T>::free_balance(
					DOT_ASSET_ID,
					&Keyring::Charlie.to_account_id(),
				);

				add_allowance::<T>(
					Keyring::Alice,
					DOT_ASSET_ID,
					Location::Local(Keyring::Bob.to_account_id()),
				);

				assert_noop!(
					pallet_restricted_tokens::Pallet::<T>::transfer(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						Keyring::Charlie.into(),
						DOT_ASSET_ID,
						dot(TRANSFER_AMOUNT)
					),
					pallet_restricted_tokens::Error::<T>::PreConditionsNotMet
				);

				let after_transfer_alice = orml_tokens::Pallet::<T>::free_balance(
					DOT_ASSET_ID,
					&Keyring::Alice.to_account_id(),
				);
				let after_transfer_charlie = orml_tokens::Pallet::<T>::free_balance(
					DOT_ASSET_ID,
					&Keyring::Charlie.to_account_id(),
				);

				assert_eq!(after_transfer_alice, pre_transfer_alice);
				assert_eq!(after_transfer_charlie, pre_transfer_charlie);

				assert_ok!(pallet_restricted_tokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					Keyring::Bob.into(),
					DOT_ASSET_ID,
					dot(TRANSFER_AMOUNT)
				),);

				let after_transfer_alice = orml_tokens::Pallet::<T>::free_balance(
					DOT_ASSET_ID,
					&Keyring::Alice.to_account_id(),
				);
				let after_transfer_bob = orml_tokens::Pallet::<T>::free_balance(
					DOT_ASSET_ID,
					&Keyring::Bob.to_account_id(),
				);
				let after_transfer_charlie = orml_tokens::Pallet::<T>::free_balance(
					DOT_ASSET_ID,
					&Keyring::Charlie.to_account_id(),
				);

				assert_eq!(
					after_transfer_alice,
					pre_transfer_alice - dot(TRANSFER_AMOUNT)
				);
				assert_eq!(after_transfer_bob, pre_transfer_bob + dot(TRANSFER_AMOUNT));
				assert_eq!(after_transfer_charlie, pre_transfer_charlie);
			});
		}

		fn restrict_dot_xcm_transfer<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(10)))
					.storage(),
			);

			transfer_dot_from_relay_chain(&mut env);

			env.parachain_state_mut(|| {
				let alice_initial_dot =
					orml_tokens::Pallet::<T>::free_balance(DOT_ASSET_ID, &Keyring::Alice.into());

				assert_eq!(alice_initial_dot, dot(3) - dot_fee());

				assert_ok!(pallet_xcm::Pallet::<T>::force_xcm_version(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					Box::new(MultiLocation::new(1, Junctions::Here)),
					XCM_VERSION,
				));

				assert_ok!(
					pallet_transfer_allowlist::Pallet::<T>::add_transfer_allowance(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						FilterCurrency::Specific(DOT_ASSET_ID),
						allowed_xcm_location()
					)
				);

				assert_noop!(
					pallet_restricted_xtokens::Pallet::<T>::transfer(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						DOT_ASSET_ID,
						dot(1),
						Box::new(
							MultiLocation::new(
								1,
								X1(Junction::AccountId32 {
									id: Keyring::Bob.into(),
									network: None,
								})
							)
							.into()
						),
						WeightLimit::Unlimited,
					),
					pallet_transfer_allowlist::Error::<T>::NoAllowanceForDestination
				);

				assert_ok!(pallet_restricted_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					DOT_ASSET_ID,
					dot(1),
					Box::new(
						MultiLocation::new(
							1,
							X1(Junction::AccountId32 {
								id: Keyring::Alice.into(),
								network: None,
							})
						)
						.into()
					),
					WeightLimit::Unlimited,
				));

				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(DOT_ASSET_ID, &Keyring::Alice.into()),
					alice_initial_dot - dot(1),
				);
			});

			env.pass(Blocks::ByNumber(1));

			env.relay_state_mut(|| {
				assert_eq!(
					pallet_balances::Pallet::<FudgeRelayRuntime<T>>::free_balance(
						&Keyring::Alice.into()
					),
					79628418552
				);
			});
		}

		crate::test_for_runtimes!([centrifuge], restrict_lp_eth_usdc_transfer);
		crate::test_for_runtimes!([centrifuge], restrict_lp_eth_usdc_lp_transfer);
		crate::test_for_runtimes!([centrifuge], restrict_usdc_transfer);
		crate::test_for_runtimes!([centrifuge], restrict_usdc_xcm_transfer);
		crate::test_for_runtimes!([centrifuge], restrict_dot_transfer);
		crate::test_for_runtimes!([centrifuge], restrict_dot_xcm_transfer);
		crate::test_for_runtimes!([centrifuge], restrict_all);
	}

	mod transfers {
		use super::*;

		fn transfer_cfg_to_sibling<T: Runtime + FudgeSupport>(env: &mut FudgeEnv<T>) {
			let alice_initial_balance = cfg(10);
			let transfer_amount = cfg(5);
			let cfg_in_sibling = CurrencyId::ForeignAsset(12);

			// CFG Metadata
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 18,
				name: "Centrifuge".into(),
				symbol: "CFG".into(),
				existential_deposit: 1_000_000_000_000,
				location: Some(VersionedMultiLocation::V3(MultiLocation::new(
					1,
					X2(
						Parachain(T::FudgeHandle::PARA_ID),
						general_key(parachains::polkadot::centrifuge::CFG_KEY),
					),
				))),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::Xcm(Default::default()),
					..CustomMetadata::default()
				},
			};

			env.parachain_state_mut(|| {
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					alice_initial_balance
				);
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&parachain_account(
						T::FudgeHandle::SIBLING_ID
					)),
					0
				);

				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta.clone(),
					Some(CurrencyId::Native),
				));
			});

			env.sibling_state_mut(|| {
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(cfg_in_sibling, &Keyring::Bob.into()),
					0
				);

				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta,
					Some(cfg_in_sibling)
				));
			});

			env.parachain_state_mut(|| {
				assert_ok!(orml_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					CurrencyId::Native,
					transfer_amount,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Parachain(T::FudgeHandle::SIBLING_ID),
								Junction::AccountId32 {
									network: None,
									id: Keyring::Bob.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Limited(8_000_000_000_000.into()),
				));

				// Confirm that Alice's balance is initial balance - amount transferred
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					alice_initial_balance - transfer_amount
				);

				// Verify that the amount transferred is now part of the sibling account here
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&parachain_account(
						T::FudgeHandle::SIBLING_ID
					)),
					transfer_amount
				);
			});

			env.pass(Blocks::ByNumber(1));

			env.sibling_state_mut(|| {
				let current_balance =
					orml_tokens::Pallet::<T>::free_balance(cfg_in_sibling, &Keyring::Bob.into());

				// Verify that Keyring::Bob now has (amount transferred - fee)
				assert_eq!(current_balance, transfer_amount - fee(18));

				// Sanity check for the actual amount Keyring::Bob ends up with
				assert_eq!(current_balance, 4992960800000000000);
			});
		}

		fn test_cfg_transfers_to_and_from_sibling<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(10)))
					.storage(),
			);

			setup_xcm(&mut env);

			// In order to be able to transfer CFG from Sibling to Centrifuge, we need to
			// first send CFG from Centrifuge to Sibling, or else it fails since it'd be
			// like Sibling had minted CFG on their side.
			transfer_cfg_to_sibling(&mut env);

			let alice_initial_balance = cfg(5);
			let bob_initial_balance = cfg(5) - cfg_fee();
			let transfer_amount = cfg(1);
			// Note: This asset was registered in `transfer_cfg_to_sibling`
			let cfg_in_sibling = CurrencyId::ForeignAsset(12);

			env.parachain_state_mut(|| {
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					alice_initial_balance
				);
			});

			env.sibling_state_mut(|| {
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&parachain_account(
						T::FudgeHandle::PARA_ID
					)),
					0
				);
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(cfg_in_sibling, &Keyring::Bob.into()),
					bob_initial_balance
				);
			});

			env.sibling_state_mut(|| {
				assert_ok!(orml_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					cfg_in_sibling,
					transfer_amount,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Parachain(T::FudgeHandle::PARA_ID),
								Junction::AccountId32 {
									network: None,
									id: Keyring::Alice.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Limited(8_000_000_000_000.into()),
				));

				// Confirm that Bobs's balance is initial balance - amount transferred
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(cfg_in_sibling, &Keyring::Bob.into()),
					bob_initial_balance - transfer_amount
				);
			});

			env.pass(Blocks::ByNumber(2));

			env.parachain_state_mut(|| {
				// Verify that Keyring::Alice now has initial balance + amount transferred - fee
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					alice_initial_balance + transfer_amount - cfg_fee(),
				);
			});
		}

		fn transfer_ausd_to_centrifuge<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			setup_xcm(&mut env);

			let alice_initial_balance = ausd(10);
			let transfer_amount = ausd(7);

			env.sibling_state_mut(|| {
				register_ausd::<T>();

				assert_ok!(orml_tokens::Pallet::<T>::deposit(
					AUSD_CURRENCY_ID,
					&Keyring::Alice.into(),
					alice_initial_balance
				));

				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(
						AUSD_CURRENCY_ID,
						&parachain_account(T::FudgeHandle::PARA_ID)
					),
					0
				);
			});

			env.parachain_state_mut(|| {
				register_ausd::<T>();

				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(AUSD_CURRENCY_ID, &Keyring::Bob.into()),
					0,
				);
			});

			env.sibling_state_mut(|| {
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(
						AUSD_CURRENCY_ID,
						&Keyring::Alice.into()
					),
					ausd(10),
				);
				assert_ok!(orml_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					AUSD_CURRENCY_ID,
					transfer_amount,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Parachain(T::FudgeHandle::PARA_ID),
								Junction::AccountId32 {
									network: None,
									id: Keyring::Bob.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Limited(8_000_000_000_000.into()),
				));

				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(
						AUSD_CURRENCY_ID,
						&Keyring::Alice.into()
					),
					alice_initial_balance - transfer_amount
				);

				// Verify that the amount transferred is now part of the centrifuge parachain
				// account here
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(
						AUSD_CURRENCY_ID,
						&parachain_account(T::FudgeHandle::PARA_ID)
					),
					transfer_amount
				);
			});

			env.pass(Blocks::ByNumber(2));

			env.parachain_state_mut(|| {
				// Verify that Keyring::Bob now has initial balance + amount transferred - fee
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(AUSD_CURRENCY_ID, &Keyring::Bob.into()),
					transfer_amount - ausd_fee()
				);
			});
		}

		fn transfer_dot_to_and_from_relay_chain<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			transfer_dot_from_relay_chain(&mut env);

			env.parachain_state_mut(|| {
				let alice_initial_dot =
					orml_tokens::Pallet::<T>::free_balance(DOT_ASSET_ID, &Keyring::Alice.into());

				assert_eq!(alice_initial_dot, dot(3) - dot_fee());

				assert_ok!(pallet_xcm::Pallet::<T>::force_xcm_version(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					Box::new(MultiLocation::new(1, Junctions::Here)),
					XCM_VERSION,
				));

				assert_ok!(orml_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					DOT_ASSET_ID,
					dot(1),
					Box::new(
						MultiLocation::new(
							1,
							X1(Junction::AccountId32 {
								id: Keyring::Alice.into(),
								network: None,
							})
						)
						.into()
					),
					WeightLimit::Unlimited,
				));

				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(DOT_ASSET_ID, &Keyring::Alice.into()),
					alice_initial_dot - dot(1),
				);
			});

			env.pass(Blocks::ByNumber(1));

			env.relay_state_mut(|| {
				assert_eq!(
					pallet_balances::Pallet::<FudgeRelayRuntime<T>>::free_balance(
						&Keyring::Alice.into()
					),
					79628418552
				);
			});
		}

		fn transfer_foreign_sibling_to_centrifuge<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_parachain_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(10)))
					.storage(),
			);

			setup_xcm(&mut env);

			let sibling_asset_id = CurrencyId::ForeignAsset(1);
			let asset_location = MultiLocation::new(
				1,
				X2(Parachain(T::FudgeHandle::SIBLING_ID), general_key(&[0, 1])),
			);
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 18,
				name: "Sibling Native Token".into(),
				symbol: "SBLNG".into(),
				existential_deposit: 1_000_000_000_000,
				location: Some(VersionedMultiLocation::V3(asset_location)),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::Xcm(XcmMetadata {
						// We specify a custom fee_per_second and verify below that this value is
						// used when XCM transfer fees are charged for this token.
						fee_per_second: Some(8420000000000000000),
					}),
					..CustomMetadata::default()
				},
			};
			let transfer_amount = foreign(1, meta.decimals);

			env.sibling_state_mut(|| {
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(sibling_asset_id, &Keyring::Bob.into()),
					0
				);
				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta.clone(),
					Some(CurrencyId::Native),
				));
			});

			env.parachain_state_mut(|| {
				// First, register the asset in centrifuge
				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta.clone(),
					Some(sibling_asset_id)
				));
			});

			env.sibling_state_mut(|| {
				assert_ok!(pallet_balances::Pallet::<T>::force_set_balance(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					Keyring::Alice.to_account_id().into(),
					transfer_amount * 2,
				));

				assert_ok!(orml_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					CurrencyId::Native,
					transfer_amount,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Parachain(T::FudgeHandle::PARA_ID),
								Junction::AccountId32 {
									network: None,
									id: Keyring::Bob.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Limited(8_000_000_000_000.into()),
				));

				// Confirm that Alice's balance is initial balance - amount transferred
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					transfer_amount
				);
			});

			env.pass(Blocks::ByNumber(2));

			env.parachain_state_mut(|| {
				let bob_balance =
					orml_tokens::Pallet::<T>::free_balance(sibling_asset_id, &Keyring::Bob.into());

				// Verify that Keyring::Bob now has initial balance + amount transferred - fee
				assert_eq!(
					bob_balance,
					transfer_amount
						- calc_fee(
							xcm_metadata(meta.additional.transferability)
								.unwrap()
								.fee_per_second
								.unwrap()
						)
				);
				// Sanity check to ensure the calculated is what is expected
				assert_eq!(bob_balance, 993264000000000000);
			});
		}

		fn transfer_wormhole_usdc_acala_to_centrifuge<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(
				Default::default(),
				Default::default(),
				Genesis::default()
					.add(genesis::balances::<T>(cfg(10)))
					.storage(),
			);

			setup_xcm(&mut env);

			let usdc_asset_id = CurrencyId::ForeignAsset(39);
			let asset_location = MultiLocation::new(
				1,
				X2(
					Parachain(T::FudgeHandle::SIBLING_ID),
					general_key("0x02f3a00dd12f644daec907013b16eb6d14bf1c4cb4".as_bytes()),
				),
			);
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 6,
				name: "Wormhole USDC".into(),
				symbol: "WUSDC".into(),
				existential_deposit: 1,
				location: Some(VersionedMultiLocation::V3(asset_location)),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::Xcm(Default::default()),
					..CustomMetadata::default()
				},
			};
			let transfer_amount = foreign(12, meta.decimals);
			let alice_initial_balance = transfer_amount * 100;

			env.sibling_state_mut(|| {
				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta.clone(),
					Some(usdc_asset_id)
				));
				assert_ok!(orml_tokens::Pallet::<T>::deposit(
					usdc_asset_id,
					&Keyring::Alice.into(),
					alice_initial_balance
				));
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(usdc_asset_id, &Keyring::Alice.into()),
					alice_initial_balance
				);
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					cfg(10)
				);
			});

			env.parachain_state_mut(|| {
				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta.clone(),
					Some(usdc_asset_id)
				));
			});

			env.sibling_state_mut(|| {
				assert_ok!(orml_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					usdc_asset_id,
					transfer_amount,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Parachain(T::FudgeHandle::PARA_ID),
								Junction::AccountId32 {
									network: None,
									id: Keyring::Bob.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Limited(8_000_000_000.into()),
				));
				// Confirm that Alice's balance is initial balance - amount transferred
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(usdc_asset_id, &Keyring::Alice.into()),
					alice_initial_balance - transfer_amount
				);
			});

			env.pass(Blocks::ByNumber(2));

			env.parachain_state_mut(|| {
				let bob_balance =
					orml_tokens::Pallet::<T>::free_balance(usdc_asset_id, &Keyring::Bob.into());

				// Sanity check to ensure the calculated is what is expected
				assert_eq!(bob_balance, 11992961);
			});
		}

		crate::test_for_runtimes!([centrifuge], test_cfg_transfers_to_and_from_sibling);
		crate::test_for_runtimes!([centrifuge], transfer_ausd_to_centrifuge);
		crate::test_for_runtimes!([centrifuge], transfer_dot_to_and_from_relay_chain);
		crate::test_for_runtimes!([centrifuge], transfer_foreign_sibling_to_centrifuge);
		crate::test_for_runtimes!([centrifuge], transfer_wormhole_usdc_acala_to_centrifuge);
	}
}

mod all {
	use super::*;

	mod restricted_calls {
		use super::*;

		fn xtokens_transfer<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			env.parachain_state_mut(|| {
				assert_noop!(
					orml_xtokens::Pallet::<T>::transfer(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						CurrencyId::Tranche(401, [0; 16]),
						42,
						Box::new(
							MultiLocation::new(
								1,
								X2(
									Parachain(T::FudgeHandle::SIBLING_ID),
									Junction::AccountId32 {
										network: None,
										id: Keyring::Bob.into(),
									}
								)
							)
							.into()
						),
						WeightLimit::Limited(8_000_000_000_000.into()),
					),
					orml_xtokens::Error::<T>::NotCrossChainTransferableCurrency
				);
			});
		}

		fn xtokens_transfer_multiasset<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			let tranche_currency = CurrencyId::Tranche(401, [0; 16]);
			let tranche_id =
				WeakBoundedVec::<u8, ConstU32<32>>::force_from(tranche_currency.encode(), None);
			let tranche_location = MultiLocation {
				parents: 1,
				interior: X3(
					Parachain(123),
					PalletInstance(42),
					GeneralKey {
						length: tranche_id.len() as u8,
						data: vec_to_fixed_array(tranche_id.to_vec()),
					},
				),
			};
			let tranche_multi_asset = VersionedMultiAsset::from(MultiAsset::from((
				AssetId::Concrete(tranche_location),
				Fungibility::Fungible(42),
			)));

			env.parachain_state_mut(|| {
				assert_noop!(
					orml_xtokens::Pallet::<T>::transfer_multiasset(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						Box::new(tranche_multi_asset),
						Box::new(
							MultiLocation::new(
								1,
								X2(
									Parachain(T::FudgeHandle::SIBLING_ID),
									Junction::AccountId32 {
										network: None,
										id: Keyring::Bob.into(),
									}
								)
							)
							.into()
						),
						WeightLimit::Limited(8_000_000_000_000.into()),
					),
					orml_xtokens::Error::<T>::XcmExecutionFailed
				);
			});
		}

		fn xtokens_transfer_multiassets<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::default();

			let tranche_currency = CurrencyId::Tranche(401, [0; 16]);
			let tranche_id =
				WeakBoundedVec::<u8, ConstU32<32>>::force_from(tranche_currency.encode(), None);
			let tranche_location = MultiLocation {
				parents: 1,
				interior: X3(
					Parachain(123),
					PalletInstance(42),
					GeneralKey {
						length: tranche_id.len() as u8,
						data: vec_to_fixed_array(tranche_id.to_vec()),
					},
				),
			};
			let tranche_multi_asset = MultiAsset::from((
				AssetId::Concrete(tranche_location),
				Fungibility::Fungible(42),
			));

			env.parachain_state_mut(|| {
				assert_noop!(
					orml_xtokens::Pallet::<T>::transfer_multiassets(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						Box::new(VersionedMultiAssets::from(MultiAssets::from(vec![
							tranche_multi_asset
						]))),
						0,
						Box::new(
							MultiLocation::new(
								1,
								X2(
									Parachain(T::FudgeHandle::SIBLING_ID),
									Junction::AccountId32 {
										network: None,
										id: Keyring::Bob.into(),
									}
								)
							)
							.into()
						),
						WeightLimit::Limited(8_000_000_000_000.into()),
					),
					orml_xtokens::Error::<T>::XcmExecutionFailed
				);
			});
		}

		crate::test_for_runtimes!(all, xtokens_transfer);
		crate::test_for_runtimes!(all, xtokens_transfer_multiasset);
		crate::test_for_runtimes!(all, xtokens_transfer_multiassets);
	}
}
