use cfg_types::{
	domain_address::DomainAddress,
	locations::Location,
	tokens::{CurrencyId::Native, FilterCurrency},
};
use frame_support::{assert_noop, assert_ok, traits::Get, BoundedVec};
use liquidity_pools_gateway_routers::{DomainRouter, EthereumXCMRouter, XCMRouter, XcmDomain};
use orml_traits::MultiCurrency;
use xcm::v3::MultiLocation;

use super::*;
use crate::{
	generic::{
		cases::liquidity_pools::utils::setup_xcm,
		config::Runtime,
		env::Env,
		envs::{
			fudge_env::{handle::FudgeHandle, FudgeEnv, FudgeSupport},
			runtime_env::RuntimeEnv,
		},
		utils::{genesis, genesis::Genesis},
	},
	utils::accounts::Keyring,
};

const TRANSFER_AMOUNT: u128 = 10;

fn xcm_location() -> MultiLocation {
	MultiLocation::new(
		1,
		xcm::v3::Junctions::X1(AccountId32 {
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

	let (pre_transfer_alice, pre_transfer_bob, pre_transfer_charlie) =
		env.parachain_state_mut(|| {
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
				pallet_balances::Pallet::<T>::free_balance(&Keyring::Charlie.to_account_id()),
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

		let pre_transfer_alice =
			orml_tokens::Pallet::<T>::free_balance(LP_ETH_USDC, &Keyring::Alice.to_account_id());
		let pre_transfer_bob =
			orml_tokens::Pallet::<T>::free_balance(LP_ETH_USDC, &Keyring::Bob.to_account_id());
		let pre_transfer_charlie =
			orml_tokens::Pallet::<T>::free_balance(LP_ETH_USDC, &Keyring::Charlie.to_account_id());

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

		let after_transfer_alice =
			orml_tokens::Pallet::<T>::free_balance(LP_ETH_USDC, &Keyring::Alice.to_account_id());
		let after_transfer_charlie =
			orml_tokens::Pallet::<T>::free_balance(LP_ETH_USDC, &Keyring::Charlie.to_account_id());

		assert_eq!(after_transfer_alice, pre_transfer_alice);
		assert_eq!(after_transfer_charlie, pre_transfer_charlie);

		assert_ok!(pallet_restricted_tokens::Pallet::<T>::transfer(
			RawOrigin::Signed(Keyring::Alice.into()).into(),
			Keyring::Bob.into(),
			LP_ETH_USDC,
			lp_eth_usdc(TRANSFER_AMOUNT)
		),);

		let after_transfer_alice =
			orml_tokens::Pallet::<T>::free_balance(LP_ETH_USDC, &Keyring::Alice.to_account_id());
		let after_transfer_bob =
			orml_tokens::Pallet::<T>::free_balance(LP_ETH_USDC, &Keyring::Bob.to_account_id());
		let after_transfer_charlie =
			orml_tokens::Pallet::<T>::free_balance(LP_ETH_USDC, &Keyring::Charlie.to_account_id());

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
						MultiLocation::new(1, X1(Parachain(T::FudgeHandle::SIBLING_ID))).into(),
					),
					ethereum_xcm_transact_call_index: BoundedVec::truncate_from(vec![38, 0]),
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
		<paras::GenesisConfig as GenesisBuild<FudgeRelayRuntime<T>>>::build_storage(
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

		let pre_transfer_alice =
			orml_tokens::Pallet::<T>::free_balance(DOT_ASSET_ID, &Keyring::Alice.to_account_id());
		let pre_transfer_bob =
			orml_tokens::Pallet::<T>::free_balance(DOT_ASSET_ID, &Keyring::Bob.to_account_id());
		let pre_transfer_charlie =
			orml_tokens::Pallet::<T>::free_balance(DOT_ASSET_ID, &Keyring::Charlie.to_account_id());

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

		let after_transfer_alice =
			orml_tokens::Pallet::<T>::free_balance(DOT_ASSET_ID, &Keyring::Alice.to_account_id());
		let after_transfer_charlie =
			orml_tokens::Pallet::<T>::free_balance(DOT_ASSET_ID, &Keyring::Charlie.to_account_id());

		assert_eq!(after_transfer_alice, pre_transfer_alice);
		assert_eq!(after_transfer_charlie, pre_transfer_charlie);

		assert_ok!(pallet_restricted_tokens::Pallet::<T>::transfer(
			RawOrigin::Signed(Keyring::Alice.into()).into(),
			Keyring::Bob.into(),
			DOT_ASSET_ID,
			dot(TRANSFER_AMOUNT)
		),);

		let after_transfer_alice =
			orml_tokens::Pallet::<T>::free_balance(DOT_ASSET_ID, &Keyring::Alice.to_account_id());
		let after_transfer_bob =
			orml_tokens::Pallet::<T>::free_balance(DOT_ASSET_ID, &Keyring::Bob.to_account_id());
		let after_transfer_charlie =
			orml_tokens::Pallet::<T>::free_balance(DOT_ASSET_ID, &Keyring::Charlie.to_account_id());

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
			pallet_balances::Pallet::<FudgeRelayRuntime<T>>::free_balance(&Keyring::Alice.into()),
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
