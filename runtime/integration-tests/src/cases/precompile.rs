use axelar_gateway_precompile::SourceConverter;
use cfg_primitives::CFG;
use cfg_traits::liquidity_pools::LPEncoding;
use cfg_types::domain_address::{Domain, DomainAddress};
use ethabi::{Function, Param, ParamType, Token};
use frame_support::assert_ok;
use frame_system::RawOrigin;
use orml_traits::MultiCurrency;
use pallet_evm::AddressMapping;
use pallet_liquidity_pools::Message;
use runtime_common::evm::precompile::LP_AXELAR_GATEWAY;
use sp_core::{H160, H256, U256};
use sp_runtime::traits::{BlakeTwo256, Hash};

use crate::{
	config::Runtime,
	env::Env,
	envs::runtime_env::RuntimeEnv,
	utils::{
		currency::{usd18, CurrencyInfo, Usd18},
		evm,
		genesis::{self, Genesis},
	},
};

#[test_runtimes(all)]
fn axelar_precompile_execute<T: Runtime>() {
	RuntimeEnv::<T>::from_parachain_storage(
		Genesis::default()
			.add(genesis::assets::<T>([(Usd18.id(), &Usd18.metadata())]))
			.storage(),
	)
	.parachain_state_mut(|| {
		let lp_axelar_gateway = H160::from_low_u64_be(LP_AXELAR_GATEWAY);

		let sender_address = H160::from_low_u64_be(1_000_002);
		let receiver_address = H160::from_low_u64_be(1_000_003);

		let source_address = H160::from_low_u64_be(1111);
		let evm_chain_name = String::from("Ethereum");
		let evm_chain_id = 0;
		let command_id = H256::from_low_u64_be(5678);
		let transfer_amount = usd18(100);

		let derived_sender_account = T::AddressMapping::into_account_id(sender_address);
		let derived_receiver_account = T::AddressMapping::into_account_id(receiver_address);

		evm::mint_balance_into_derived_account::<T>(sender_address, 1 * CFG);

		let general_currency_id =
			pallet_liquidity_pools::Pallet::<T>::try_get_general_index(Usd18.id()).unwrap();

		axelar_gateway_precompile::Pallet::<T>::set_gateway(RawOrigin::Root.into(), sender_address)
			.unwrap();

		axelar_gateway_precompile::Pallet::<T>::set_converter(
			RawOrigin::Root.into(),
			BlakeTwo256::hash(evm_chain_name.as_bytes()),
			SourceConverter {
				domain: Domain::EVM(evm_chain_id),
			},
		)
		.unwrap();

		pallet_liquidity_pools_gateway::Pallet::<T>::add_instance(
			RawOrigin::Root.into(),
			DomainAddress::EVM(evm_chain_id, source_address.0),
		)
		.unwrap();

		let msg = Message::TransferAssets {
			currency: general_currency_id,
			receiver: derived_receiver_account.clone().into(),
			amount: transfer_amount,
		};

		#[allow(deprecated)] // Due `constant` field. Can be remove in future ethabi
		let eth_function_encoded = Function {
			name: "execute".into(),
			inputs: vec![
				Param {
					name: "commandId".into(),
					kind: ParamType::FixedBytes(32),
					internal_type: None,
				},
				Param {
					name: "sourceChain".into(),
					kind: ParamType::String,
					internal_type: None,
				},
				Param {
					name: "sourceAddress".into(),
					kind: ParamType::String,
					internal_type: None,
				},
				Param {
					name: "payload".into(),
					kind: ParamType::Bytes,
					internal_type: None,
				},
			],
			outputs: vec![],
			constant: Some(false),
			state_mutability: Default::default(),
		}
		.encode_input(&[
			Token::FixedBytes(command_id.0.to_vec()),
			Token::String(evm_chain_name),
			Token::String(String::from_utf8(source_address.as_fixed_bytes().to_vec()).unwrap()),
			Token::Bytes(msg.serialize()),
		])
		.expect("cannot encode input for test contract function");

		assert_ok!(pallet_evm::Pallet::<T>::call(
			RawOrigin::Root.into(),
			sender_address,
			lp_axelar_gateway,
			eth_function_encoded.to_vec(),
			U256::from(0),
			0x100000,
			U256::from(1_000_000_000),
			None,
			Some(U256::from(0)),
			Vec::new(),
		));

		assert_eq!(
			orml_tokens::Pallet::<T>::free_balance(Usd18.id(), &derived_receiver_account),
			transfer_amount
		);
	});
}
