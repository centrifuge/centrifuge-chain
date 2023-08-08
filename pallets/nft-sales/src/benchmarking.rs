#![cfg(feature = "runtime-benchmarks")]
use cfg_primitives::CFG as CURRENCY;
use cfg_types::tokens::CurrencyId;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::traits::tokens::nonfungibles::{Create, Mutate};
use frame_system::RawOrigin;
use orml_tokens::{self as OrmlTokens};
use orml_traits::MultiCurrency;

use crate::{self as pallet_nft_sales, *};

pub const AUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(1);

benchmarks! {
	where_clause {
		where
		T: Config
			+ pallet_uniques::Config<CollectionId = <T as Config>::CollectionId>
			+ pallet_uniques::Config<ItemId = <T as Config>::ItemId>
			+ orml_tokens::Config<CurrencyId = crate::CurrencyOf<T>>
			+ orml_tokens::Config<Balance = crate::BalanceOf<T>>
			+ pallet_balances::Config,
		<T as pallet_balances::Config>::Balance: From<u128>,
		<T as pallet_nft_sales::Config>::CollectionId: From<u64>,
		<T as OrmlTokens::Config>::Balance: From<u128>,
		<T as OrmlTokens::Config>::CurrencyId: From<CurrencyId>,
		<<T as pallet_nft_sales::Config>::Fungibles as fungibles::Inspect<AccountIdOf<T>>>::AssetId: From<CurrencyId>,
		<<T as pallet_nft_sales::Config>::Fungibles as fungibles::Inspect<AccountIdOf<T>>>::Balance: From<u128>,
	}

	// Add an NFT for sale
	add {
		let seller_account = account::<T::AccountId>("seller", 0, 0);
		let seller_origin: RawOrigin<T::AccountId> = RawOrigin::Signed(seller_account.clone());
		deposit_native_balance::<T>(&seller_account);

		// We need the NFT to exist in the pallet-uniques before we can put it for sale
		let (collection_id, item_id) = mint_nft::<T>(0, 1, &seller_account);
		// Define the price
		let price: Price<crate::CurrencyOf<T>, crate::BalanceOf<T>> = Price { currency: AUSD_CURRENCY_ID.into(), amount: 10_000u128.into() };

	}: _(seller_origin, collection_id, item_id, price)
	verify {
		assert!(<Sales<T>>::contains_key(collection_id, item_id), "NFT should be for sale now");
	}

	// Remove an NFT from sale
	remove {
		let seller_account = account::<T::AccountId>("seller", 0, 0);
		let seller_origin: RawOrigin<T::AccountId> = RawOrigin::Signed(seller_account.clone());
		deposit_native_balance::<T>(&seller_account);

		// We need the NFT to exist in the pallet-uniques before we can put it for sale
		let (collection_id, item_id) = mint_nft::<T>(0, 1, &seller_account);
		// Define the price
		let price: Price<crate::CurrencyOf<T>, crate::BalanceOf<T>> = Price { currency: AUSD_CURRENCY_ID.into(), amount: 10_000u128.into() };

		// We need the nft in the storage beforehand to be able to remove it
		<Sales<T>>::insert(collection_id, item_id, Sale { seller: seller_account, price});

	}: _(seller_origin, collection_id, item_id)
	verify {
		assert!(<Sales<T>>::get(collection_id, item_id).is_none(), "The NFT should have been removed from sale");
	}

	// Remove an NFT from sale
	buy {
		let seller_account = account::<T::AccountId>("seller", 0, 0);
		let seller_origin: RawOrigin<T::AccountId> = RawOrigin::Signed(seller_account.clone());
		deposit_native_balance::<T>(&seller_account);

		// We need the NFT to exist in the pallet-uniques before we can put it for sale
		let (collection_id, item_id) = mint_nft::<T>(0, 1, &seller_account);
		// Define the price
		let price: Price<crate::CurrencyOf<T>, crate::BalanceOf<T>> = Price { currency: AUSD_CURRENCY_ID.into(), amount: 10_000u128.into() };

		// We need the nft in the storage beforehand to be able to remove it
		<Sales<T>>::insert(collection_id, item_id, Sale { seller: seller_account, price: price.clone()});

		// We need the buyer to have enough balance to pay for the NFT
		let buyer_account = account::<T::AccountId>("buyer", 0, 0);
		let buyer_origin: RawOrigin<T::AccountId> = RawOrigin::Signed(buyer_account.clone());
		deposit_token_balance::<T>(&buyer_account, AUSD_CURRENCY_ID, 100_000u128.into());

	}: _(buyer_origin, collection_id, item_id, price)
	verify {
		assert!(<Sales<T>>::get(collection_id, item_id).is_none(), "The NFT should have been removed from sale once bought");
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime,);

#[allow(dead_code)]
fn deposit_token_balance<T>(
	account: &T::AccountId,
	currency_id: CurrencyId,
	balance: <T as OrmlTokens::Config>::Balance,
) where
	T: Config + OrmlTokens::Config,
	<T as OrmlTokens::Config>::CurrencyId: From<CurrencyId>,
{
	<OrmlTokens::Pallet<T> as MultiCurrency<T::AccountId>>::deposit(
		currency_id.into(),
		account,
		balance,
	)
	.expect("should not fail to set new token balance");
}

fn deposit_native_balance<T>(account: &T::AccountId)
where
	T: Config + pallet_balances::Config,
	<T as pallet_balances::Config>::Balance: From<u128>,
{
	use frame_support::traits::Currency;

	let min_balance: <T as pallet_balances::Config>::Balance = (10_000_000u128 * CURRENCY).into();
	let _ = pallet_balances::Pallet::<T>::make_free_balance_be(account, min_balance);
}

pub(crate) fn create_nft_class<T>(
	collection_id: u64,
	owner: T::AccountId,
) -> <T as pallet_nft_sales::Config>::CollectionId
where
	T: frame_system::Config
		+ pallet_nft_sales::Config
		+ pallet_uniques::Config
		+ pallet_uniques::Config<CollectionId = <T as Config>::CollectionId>,
	<T as pallet_uniques::Config>::CollectionId: From<u64>,
{
	// Create class. Shouldn't fail.
	let uniques_class_id: <T as pallet_uniques::Config>::CollectionId = collection_id.into();
	<pallet_uniques::Pallet<T> as Create<T::AccountId>>::create_collection(
		&uniques_class_id,
		&owner,
		&owner,
	)
	.expect("class creation should not fail");
	uniques_class_id
}

pub(crate) fn mint_nft<T>(
	class_id_raw: u64,
	instance_id_raw: u128,
	owner: &T::AccountId,
) -> (
	<T as pallet_uniques::Config>::CollectionId,
	<T as pallet_nft_sales::Config>::ItemId,
)
where
	T: frame_system::Config
		+ pallet_nft_sales::Config
		+ pallet_uniques::Config
		+ pallet_uniques::Config<CollectionId = <T as Config>::CollectionId>
		+ pallet_uniques::Config<ItemId = <T as Config>::ItemId>,
	<T as pallet_uniques::Config>::ItemId: From<ItemIdOf<T>>,
	<T as pallet_uniques::Config>::CollectionId: From<CollectionIdOf<T>>,
	<T as pallet_nft_sales::Config>::CollectionId: From<u64>,
{
	// Create the NFT class
	let collection_id: <T as pallet_uniques::Config>::CollectionId =
		create_nft_class::<T>(class_id_raw, owner.clone());

	// Mint the NFT
	let item_id: <T as pallet_nft_sales::Config>::ItemId = instance_id_raw.into();
	<pallet_uniques::Pallet<T> as Mutate<T::AccountId>>::mint_into(&collection_id, &item_id, owner)
		.expect("mint should not fail");

	// Done
	(collection_id, item_id)
}
