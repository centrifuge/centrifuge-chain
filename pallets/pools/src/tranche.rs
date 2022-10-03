// Copyright 2021 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_traits::TrancheToken as TrancheTokenT;
#[cfg(test)]
use cfg_types::CurrencyId;
use cfg_types::{CustomMetadata, XcmMetadata};
use frame_support::{sp_runtime::ArithmeticError, StorageHasher};
use orml_traits::asset_registry::AssetMetadata;
use polkadot_parachain::primitives::Id as ParachainId;
use rev_slice::{RevSlice, SliceExt};
use sp_arithmetic::traits::{checked_pow, BaseArithmetic, Unsigned};
use sp_runtime::WeakBoundedVec;
use xcm::{
	latest::MultiLocation,
	prelude::{GeneralKey, PalletInstance, Parachain, X3},
	VersionedMultiLocation,
};

/// Trait for converting a pool+tranche ID pair to a CurrencyId
///
/// This should be implemented in the runtime to convert from the
/// PoolId and TrancheId types to a CurrencyId that represents that
/// tranche.
///
/// The pool epoch logic assumes that every tranche has a UNIQUE
/// currency, but nothing enforces that. Failure to ensure currency
/// uniqueness will almost certainly cause some wild bugs.
use super::*;

/// Types alias for EpochExecutionTranche
#[allow(dead_code)]
pub(super) type EpochExecutionTrancheOf<T> = EpochExecutionTranche<
	<T as Config>::Balance,
	<T as Config>::BalanceRatio,
	<T as Config>::TrancheWeight,
>;

/// Types alias for Tranches
pub(super) type TranchesOf<T> = Tranches<
	<T as Config>::Balance,
	<T as Config>::InterestRate,
	<T as Config>::TrancheWeight,
	<T as Config>::CurrencyId,
	<T as Config>::TrancheId,
	<T as Config>::PoolId,
>;

/// Types alias for Tranche
pub(super) type TrancheOf<T> = Tranche<
	<T as Config>::Balance,
	<T as Config>::InterestRate,
	<T as Config>::TrancheWeight,
	<T as Config>::CurrencyId,
>;

/// Type that indicates the seniority of a tranche
pub type Seniority = u32;

#[derive(Debug, Encode, PartialEq, Eq, Decode, Clone, TypeInfo)]
pub struct TrancheInput<Rate, MaxTokenNameLength, MaxTokenSymbolLength>
where
	MaxTokenNameLength: Get<u32>,
	MaxTokenSymbolLength: Get<u32>,
{
	pub tranche_type: TrancheType<Rate>,
	pub seniority: Option<Seniority>,
	pub metadata: TrancheMetadata<MaxTokenNameLength, MaxTokenSymbolLength>,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct TrancheUpdate<Rate> {
	pub tranche_type: TrancheType<Rate>,
	pub seniority: Option<Seniority>,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum TrancheLoc<TrancheId> {
	Index(TrancheIndex),
	Id(TrancheId),
}

#[derive(Copy, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum TrancheType<Rate> {
	Residual,
	NonResidual {
		interest_rate_per_sec: Rate,
		min_risk_buffer: Perquintill,
	},
}

impl<Rate> TrancheType<Rate>
where
	Rate: PartialOrd + PartialEq,
{
	/// Compares tranches with the following schema:
	///
	/// * (Residual, Residual) => true
	/// * (Residual, NonResidual) => true,
	/// * (NonResidual, Residual) => false,
	/// * (NonResidual, NonResidual) =>
	///         interest rate of next tranche must be smaller
	///         equal to the interest rate of self.
	///
	pub fn valid_next_tranche(&self, next: &TrancheType<Rate>) -> bool {
		match (self, next) {
			(TrancheType::Residual, TrancheType::Residual) => true,
			(TrancheType::Residual, TrancheType::NonResidual { .. }) => true,
			(TrancheType::NonResidual { .. }, TrancheType::Residual) => false,
			(
				TrancheType::NonResidual {
					interest_rate_per_sec: ref interest_prev,
					..
				},
				TrancheType::NonResidual {
					interest_rate_per_sec: ref interest_next,
					..
				},
			) => interest_prev >= interest_next,
		}
	}
}

#[derive(Debug, Encode, PartialEq, Eq, Decode, Clone, TypeInfo)]
pub struct TrancheMetadata<MaxTokenNameLength, MaxTokenSymbolLength>
where
	MaxTokenNameLength: Get<u32>,
	MaxTokenSymbolLength: Get<u32>,
{
	pub token_name: BoundedVec<u8, MaxTokenNameLength>,
	pub token_symbol: BoundedVec<u8, MaxTokenSymbolLength>,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct Tranche<Balance, Rate, Weight, CurrencyId> {
	pub(super) tranche_type: TrancheType<Rate>,
	pub(super) seniority: Seniority,
	pub currency: CurrencyId,

	pub(super) outstanding_invest_orders: Balance,
	pub(super) outstanding_redeem_orders: Balance,

	pub(super) debt: Balance,
	pub(super) reserve: Balance,
	pub(super) loss: Balance,
	pub(super) ratio: Perquintill,
	pub(super) last_updated_interest: Moment,

	pub(super) _phantom: PhantomData<Weight>,
}

#[cfg(test)]
impl<Balance, Rate, Weight> Default for Tranche<Balance, Rate, Weight, CurrencyId>
where
	Balance: One + Zero,
	Rate: FixedPointNumber<Inner = Balance> + One,
	Balance: FixedPointOperand + One + Zero,
{
	fn default() -> Self {
		Self {
			tranche_type: TrancheType::Residual,
			seniority: 1,
			currency: CurrencyId::Tranche(0, [0u8; 16]),
			outstanding_invest_orders: Zero::zero(),
			outstanding_redeem_orders: Zero::zero(),
			debt: Zero::zero(),
			reserve: Zero::zero(),
			loss: Zero::zero(),
			ratio: Perquintill::one(),
			last_updated_interest: 0,
			_phantom: PhantomData::default(),
		}
	}
}

impl<Balance, Rate, Weight, Currency> Tranche<Balance, Rate, Weight, Currency>
where
	Balance: Zero + Copy + BaseArithmetic + FixedPointOperand + Unsigned + From<u64>,
	Rate: FixedPointNumber<Inner = Balance> + One + Copy,
	Balance: FixedPointOperand,
	Weight: Copy + From<u128>,
{
	pub fn order_as_currency<BalanceRatio>(
		&self,
		price: &BalanceRatio,
	) -> Result<(Balance, Balance), DispatchError>
	where
		BalanceRatio: FixedPointNumber<Inner = Balance>,
	{
		let orders = (
			self.outstanding_invest_orders,
			price
				.checked_mul_int(self.outstanding_redeem_orders)
				.ok_or(ArithmeticError::Overflow)?,
		);

		Ok(orders)
	}

	pub fn balance(&self) -> Result<Balance, DispatchError> {
		self.debt
			.checked_add(&self.reserve)
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn free_balance(&self) -> Result<Balance, DispatchError> {
		Ok(self.reserve)
	}

	pub fn accrue(&mut self, now: Moment) -> DispatchResult {
		let delta = now - self.last_updated_interest;
		let interest = self.interest_rate_per_sec();
		// NOTE: `checked_pow` can return 1 for 0^0 which is fine
		//       for us, as we simply have the same debt if this happens
		let total_interest = checked_pow(
			interest,
			delta
				.try_into()
				.map_err(|_| DispatchError::Other("Usize should be at least 64 bits."))?,
		)
		.ok_or(ArithmeticError::Overflow)?;
		self.debt = total_interest
			.checked_mul_int(self.debt)
			.ok_or(ArithmeticError::Overflow)?;
		self.last_updated_interest = now;

		Ok(())
	}

	pub fn min_risk_buffer(&self) -> Perquintill {
		match &self.tranche_type {
			TrancheType::Residual => Perquintill::zero(),
			TrancheType::NonResidual {
				min_risk_buffer, ..
			} => *min_risk_buffer,
		}
	}

	pub fn interest_rate_per_sec(&self) -> Rate {
		match &self.tranche_type {
			TrancheType::Residual => One::one(),
			TrancheType::NonResidual {
				interest_rate_per_sec,
				..
			} => *interest_rate_per_sec,
		}
	}

	pub fn debt(&mut self, now: Moment) -> Result<Balance, DispatchError> {
		self.accrue(now)?;
		Ok(self.debt)
	}

	pub fn create_asset_metadata(
		&self,
		decimals: u32,
		parachain_id: ParachainId,
		pallet_index: u8,
		token_name: Vec<u8>,
		token_symbol: Vec<u8>,
	) -> AssetMetadata<Balance, CustomMetadata>
	where
		Balance: Zero,
		Currency: Encode,
		CustomMetadata: Parameter + Member + TypeInfo,
	{
		let tranche_id =
			WeakBoundedVec::<u8, ConstU32<32>>::force_from(self.currency.encode(), None);

		AssetMetadata {
			decimals,
			name: token_name,
			symbol: token_symbol,
			existential_deposit: Zero::zero(),
			location: Some(VersionedMultiLocation::V1(MultiLocation {
				parents: 1,
				interior: X3(
					Parachain(parachain_id.into()),
					PalletInstance(pallet_index),
					GeneralKey(tranche_id),
				),
			})),
			additional: CustomMetadata {
				mintable: false,
				permissioned: true,
				pool_currency: false,
				xcm: XcmMetadata {
					fee_per_second: None,
				},
			},
		}
	}
}

/// The index type for tranches
///
/// The `TrancheIndex` can be seen as an normal index into a vector, just
/// specified here as new-type to make this clear. U64 in order to keep the public api
/// clear.
/// In contrast to a `TrancheId` a `TrancheIndex` is not unique and does NOT refer to a
/// specific tranche, but rather to a specific tranche-location in the tranche-structure
/// of a pool.
//
// Example:
//
// Given the following tranche structure:
// ----
// Tranche-A     -> Index: 0, Id: Twox128::hash(pool_id + 0)
// Tranche-B     -> Index: 1, Id: Twox128::hash(pool_id + 1)
// Tranche-C     -> Index: 2, Id: Twox128::hash(pool_id + 2)
// ----
//
// Now replacing Tranche-B with Tranche-D
// ----
// Tranche-A     -> Index: 0, Id: Twox128::hash(pool_id + 0)
// Tranche-D     -> Index: 1, Id: Twox128::hash(pool_id + 3)
// Tranche-C     -> Index: 2, Id: Twox128::hash(pool_id + 2)
// ----
//
// One can see, that the index of Tranche-B and Tranche-D are equal
// but their ids will be different.
pub type TrancheIndex = u64;

/// The salt type for tranches
///
/// This type is used to generate unique but deterministic ids
/// for tranches of a pool.
///
/// The assumption for uniqueness only holds as long as pool-ids
/// are not reusable!
pub type TrancheSalt<PoolId> = (TrancheIndex, PoolId);

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct Tranches<Balance, Rate, Weight, Currency, TrancheId, PoolId> {
	pub tranches: Vec<Tranche<Balance, Rate, Weight, Currency>>,
	ids: Vec<TrancheId>,
	salt: TrancheSalt<PoolId>,
}

impl<Balance, Rate, Weight, CurrencyId, TrancheId, PoolId>
	Tranches<Balance, Rate, Weight, CurrencyId, TrancheId, PoolId>
where
	CurrencyId: Copy,
	Balance: Zero + Copy + BaseArithmetic + FixedPointOperand + Unsigned + From<u64>,
	Weight: Copy + From<u128>,
	Rate: One + Copy + FixedPointNumber<Inner = Balance>,
	TrancheId: Clone + From<[u8; 16]> + sp_std::cmp::PartialEq,
	PoolId: Copy + Encode,
{
	pub fn from_input<TrancheToken, MaxTokenNameLength, MaxTokenSymbolLength>(
		pool: PoolId,
		tranche_inputs: Vec<TrancheInput<Rate, MaxTokenNameLength, MaxTokenSymbolLength>>,
		now: Moment,
	) -> Result<Self, DispatchError>
	where
		TrancheToken: TrancheTokenT<PoolId, TrancheId, CurrencyId>,
		MaxTokenNameLength: Get<u32>,
		MaxTokenSymbolLength: Get<u32>,
	{
		let tranches = Vec::with_capacity(tranche_inputs.len());
		let ids = Vec::with_capacity(tranche_inputs.len());
		let salt = (0, pool);
		let mut tranches = Tranches {
			tranches,
			ids,
			salt,
		};

		for (index, tranche_input) in tranche_inputs.into_iter().enumerate() {
			tranches.add::<TrancheToken, MaxTokenNameLength, MaxTokenSymbolLength>(
				index.try_into().map_err(|_| ArithmeticError::Overflow)?,
				tranche_input,
				now,
			)?;
		}

		Ok(tranches)
	}

	pub fn new<TrancheToken>(
		pool: PoolId,
		tranches: Vec<Tranche<Balance, Rate, Weight, CurrencyId>>,
	) -> Result<Self, DispatchError>
	where
		TrancheToken: TrancheTokenT<PoolId, TrancheId, CurrencyId>,
	{
		let mut ids = Vec::with_capacity(tranches.len());
		let mut salt = (0, pool);

		for (index, _tranche) in tranches.iter().enumerate() {
			ids.push(Tranches::<
				Balance,
				Rate,
				Weight,
				CurrencyId,
				TrancheId,
				PoolId,
			>::id_from_salt(salt));
			salt = (
				(index.checked_add(1).ok_or(ArithmeticError::Overflow)?)
					.try_into()
					.map_err(|_| ArithmeticError::Overflow)?,
				pool,
			);
		}

		Ok(Self {
			tranches,
			ids,
			salt,
		})
	}

	pub fn tranche_currency(&self, id: TrancheLoc<TrancheId>) -> Option<CurrencyId> {
		self.get_tranche(id).map(|tranche| tranche.currency)
	}

	pub fn tranche_id(&self, id: TrancheLoc<TrancheId>) -> Option<TrancheId> {
		match id {
			TrancheLoc::Id(id) => Some(id),
			TrancheLoc::Index(index) => index
				.try_into()
				.ok()
				.and_then(|index: usize| self.ids.get(index).cloned()),
		}
	}

	pub fn tranche_index(&self, id: &TrancheLoc<TrancheId>) -> Option<TrancheIndex> {
		match id {
			TrancheLoc::Index(index) => Some(*index),
			TrancheLoc::Id(id) => self
				.ids
				.iter()
				.position(|curr_id| curr_id == id)
				.and_then(|index| index.try_into().ok()),
		}
	}

	pub fn get_mut_tranche(
		&mut self,
		id: TrancheLoc<TrancheId>,
	) -> Option<&mut Tranche<Balance, Rate, Weight, CurrencyId>> {
		match id {
			TrancheLoc::Index(index) => {
				let index: Option<usize> = index.try_into().ok();
				if let Some(index) = index {
					self.tranches.get_mut(index)
				} else {
					None
				}
			}
			TrancheLoc::Id(id) => {
				let index = self.tranche_index(&TrancheLoc::Id(id));
				if let Some(index) = index {
					let index: Option<usize> = index.try_into().ok();
					if let Some(index) = index {
						self.tranches.get_mut(index)
					} else {
						None
					}
				} else {
					None
				}
			}
		}
	}

	pub fn get_tranche(
		&self,
		id: TrancheLoc<TrancheId>,
	) -> Option<&Tranche<Balance, Rate, Weight, CurrencyId>> {
		match id {
			TrancheLoc::Index(index) => {
				let index: Option<usize> = index.try_into().ok();
				if let Some(index) = index {
					self.tranches.get(index)
				} else {
					None
				}
			}
			TrancheLoc::Id(id) => {
				let index = self.tranche_index(&TrancheLoc::Id(id));
				if let Some(index) = index {
					let index: Option<usize> = index.try_into().ok();
					if let Some(index) = index {
						self.tranches.get(index)
					} else {
						None
					}
				} else {
					None
				}
			}
		}
	}

	/// Defines how a given salt will be transformed into
	/// a TrancheId.
	fn id_from_salt(salt: TrancheSalt<PoolId>) -> TrancheId {
		Blake2_128::hash(salt.encode().as_slice()).into()
	}

	/// Generate ids after the following schema:
	/// * salt: The salt is a counter in our case that will always go
	///         up, even if we remove tranches.
	/// * pool-id: The pool id is ensured to be unique on-chain
	///
	/// -> tranche id = Twox128::hash(salt)
	fn next_id(&mut self) -> Result<TrancheId, DispatchError> {
		let id = Tranches::<Balance, Rate, Weight, CurrencyId, TrancheId, PoolId>::id_from_salt(
			self.salt,
		);
		self.salt = (
			self.salt
				.0
				.checked_add(1)
				.ok_or(ArithmeticError::Overflow)?,
			self.salt.1,
		);
		Ok(id)
	}

	fn create_tranche<TrancheToken>(
		&mut self,
		index: TrancheIndex,
		id: TrancheId,
		tranche_type: TrancheType<Rate>,
		seniority: Option<Seniority>,
		now: Moment,
	) -> Result<Tranche<Balance, Rate, Weight, CurrencyId>, DispatchError>
	where
		TrancheToken: TrancheTokenT<PoolId, TrancheId, CurrencyId>,
	{
		let tranche = Tranche {
			tranche_type,
			// seniority increases as index since the order is from junior to senior
			seniority: seniority
				.unwrap_or(index.try_into().map_err(|_| ArithmeticError::Overflow)?),
			currency: TrancheToken::tranche_token(self.salt.1, id),
			outstanding_invest_orders: Zero::zero(),
			outstanding_redeem_orders: Zero::zero(),
			debt: Zero::zero(),
			reserve: Zero::zero(),
			loss: Zero::zero(),
			ratio: Perquintill::zero(),
			last_updated_interest: now,
			_phantom: Default::default(),
		};
		Ok(tranche)
	}

	pub fn replace<TrancheToken, MaxTokenNameLength, MaxTokenSymbolLength>(
		&mut self,
		at: TrancheIndex,
		tranche: TrancheInput<Rate, MaxTokenNameLength, MaxTokenSymbolLength>,
		now: Moment,
	) -> DispatchResult
	where
		TrancheToken: TrancheTokenT<PoolId, TrancheId, CurrencyId>,
		MaxTokenNameLength: Get<u32>,
		MaxTokenSymbolLength: Get<u32>,
	{
		self.remove(at)?;
		self.add::<TrancheToken, MaxTokenNameLength, MaxTokenSymbolLength>(at, tranche, now)
	}

	pub fn add<TrancheToken, MaxTokenNameLength, MaxTokenSymbolLength>(
		&mut self,
		at: TrancheIndex,
		tranche: TrancheInput<Rate, MaxTokenNameLength, MaxTokenSymbolLength>,
		now: Moment,
	) -> DispatchResult
	where
		TrancheToken: TrancheTokenT<PoolId, TrancheId, CurrencyId>,
		MaxTokenNameLength: Get<u32>,
		MaxTokenSymbolLength: Get<u32>,
	{
		let at_idx = at;
		let at: usize = at.try_into().map_err(|_| ArithmeticError::Overflow)?;
		ensure!(
			at <= self.tranches.len(),
			DispatchError::Other(
				"Must add tranches either in between others or at the end. This should be catched somewhere else."
			)
		);

		let id = self.next_id()?;
		let new_tranche = self.create_tranche::<TrancheToken>(
			at_idx,
			id.clone(),
			tranche.tranche_type,
			tranche.seniority,
			now,
		)?;
		if at == 0 {
			ensure!(
				tranche.tranche_type == TrancheType::Residual,
				DispatchError::Other(
					"Top tranche must be a residual one. This should be catched somewhere else"
				)
			);
		} else {
			ensure!(
				self.tranches
					.get(at - 1)
					.expect("at is <= len and is not zero. An element before at must exist. qed.")
					.tranche_type
					.valid_next_tranche(&new_tranche.tranche_type),
				DispatchError::Other(
					"Invalid next tranche type. This should be catched somewhere else."
				)
			);
		}
		self.tranches.insert(at, new_tranche);
		self.ids.insert(at, id);

		Ok(())
	}

	pub fn remove(&mut self, at: TrancheIndex) -> DispatchResult {
		let at: usize = at.try_into().map_err(|_| ArithmeticError::Overflow)?;
		ensure!(
			at < self.tranches.len(),
			DispatchError::Other(
				"Invalid tranche index. Exceeding number of tranches. This should be catched somewhere else."
			)
		);

		self.tranches.remove(at);
		self.ids.remove(at);

		Ok(())
	}

	pub fn ids_non_residual_top(&self) -> Vec<TrancheId> {
		let mut res = Vec::with_capacity(self.tranches.len());
		self.ids.iter().rev().for_each(|id| res.push(id.clone()));
		res
	}

	pub fn combine_non_residual_top<R, F>(&self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&Tranche<Balance, Rate, Weight, CurrencyId>) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		for tranche in self.tranches.iter().rev() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_mut_non_residual_top<R, F>(&mut self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&mut Tranche<Balance, Rate, Weight, CurrencyId>) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		for tranche in self.tranches.iter_mut().rev() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_with_non_residual_top<R, I, W, F>(
		&self,
		with: I,
		mut f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&Tranche<Balance, Rate, Weight, CurrencyId>, W) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		let iter = self.tranches.iter().rev().zip(with.into_iter());

		for (tranche, w) in iter {
			let r = f(tranche, w)?;
			res.push(r);
		}

		Ok(res)
	}

	pub fn combine_with_mut_non_residual_top<R, W, I, F>(
		&mut self,
		with: I,
		mut f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&mut Tranche<Balance, Rate, Weight, CurrencyId>, W) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		let iter = self.tranches.iter_mut().rev().zip(with.into_iter());

		for (tranche, w) in iter {
			let r = f(tranche, w)?;
			res.push(r);
		}

		Ok(res)
	}

	pub fn ids_residual_top(&self) -> Vec<TrancheId> {
		self.ids.clone()
	}

	pub fn combine_residual_top<R, F>(&self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&Tranche<Balance, Rate, Weight, CurrencyId>) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		for tranche in self.tranches.iter() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_mut_residual_top<R, F>(&mut self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&mut Tranche<Balance, Rate, Weight, CurrencyId>) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		for tranche in self.tranches.iter_mut() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_with_residual_top<R, I, W, F>(
		&self,
		with: I,
		mut f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&Tranche<Balance, Rate, Weight, CurrencyId>, W) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		let iter = self.tranches.iter().zip(with.into_iter());

		for (tranche, w) in iter {
			let r = f(tranche, w)?;
			res.push(r);
		}

		Ok(res)
	}

	pub fn combine_with_mut_residual_top<R, W, I, F>(
		&mut self,
		with: I,
		mut f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&mut Tranche<Balance, Rate, Weight, CurrencyId>, W) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		// TODO: Would be nice to error out when with is larger than tranches...
		let iter = self.tranches.iter_mut().zip(with.into_iter());

		for (tranche, w) in iter {
			let r = f(tranche, w)?;
			res.push(r);
		}

		Ok(res)
	}

	pub fn orders_as_currency<BalanceRatio>(
		&self,
		prices: &[BalanceRatio],
	) -> Result<Vec<(Balance, Balance)>, DispatchError>
	where
		BalanceRatio: FixedPointNumber<Inner = Balance>,
	{
		self.combine_with_non_residual_top(prices.iter(), |tranche, price| {
			tranche.order_as_currency(price)
		})
	}

	pub fn calculate_prices<BalanceRatio, Tokens, AccountId>(
		&mut self,
		total_assets: Balance,
		now: Moment,
	) -> Result<Vec<BalanceRatio>, DispatchError>
	where
		BalanceRatio: FixedPointNumber<Inner = Balance>,
		Tokens: Inspect<AccountId, AssetId = CurrencyId, Balance = Balance>,
	{
		let mut remaining_assets = total_assets;
		let pool_is_zero = total_assets == Zero::zero();

		// we are gonna reverse the order
		// such that prices are calculated from most senior to junior
		// there by all the remaining assets are given to the most junior tranche
		let mut prices = self.combine_mut_non_residual_top(|tranche| {
			let total_issuance = Tokens::total_issuance(tranche.currency);

			if pool_is_zero || total_issuance == Zero::zero() {
				Ok(One::one())
			} else if tranche.tranche_type == TrancheType::Residual {
				BalanceRatio::checked_from_rational(remaining_assets, total_issuance)
					.ok_or(ArithmeticError::Overflow.into())
			} else {
				tranche.accrue(now)?;
				let tranche_balance = tranche.balance()?;

				// Indicates that a tranche has been wiped out and/or a tranche has
				// lost value due to defaults.
				let tranche_value = if tranche_balance > remaining_assets {
					let left_over_assets = remaining_assets;
					remaining_assets = Zero::zero();
					left_over_assets
				} else {
					remaining_assets = remaining_assets
						.checked_sub(&tranche_balance)
						.expect("Tranche value smaller equal remaining assets. qed.");
					tranche_balance
				};
				BalanceRatio::checked_from_rational(tranche_value, total_issuance)
					.ok_or(ArithmeticError::Overflow.into())
			}
		})?;

		// NOTE: We always pass around data in order NonResidual-to-Residual.
		//       -> So we need to reverse here again.
		prices.reverse();
		Ok(prices)
	}

	pub fn num_tranches(&self) -> usize {
		self.tranches.len()
	}

	pub fn into_tranches(self) -> Vec<Tranche<Balance, Rate, Weight, CurrencyId>> {
		self.tranches
	}

	pub fn non_residual_tranches(&self) -> Option<&[Tranche<Balance, Rate, Weight, CurrencyId>]> {
		if let Some((_head, tail)) = self.tranches.as_slice().split_first() {
			Some(tail)
		} else {
			None
		}
	}

	pub fn non_residual_tranches_mut(
		&mut self,
	) -> Option<&mut [Tranche<Balance, Rate, Weight, CurrencyId>]> {
		if let Some((_head, tail)) = self.tranches.as_mut_slice().split_first_mut() {
			Some(tail)
		} else {
			None
		}
	}

	pub fn residual_tranche(&self) -> Option<&Tranche<Balance, Rate, Weight, CurrencyId>> {
		if let Some((head, _tail)) = self.tranches.as_slice().split_first() {
			Some(head)
		} else {
			None
		}
	}

	pub fn residual_tranche_mut(
		&mut self,
	) -> Option<&mut Tranche<Balance, Rate, Weight, CurrencyId>> {
		if let Some((head, _tail)) = self.tranches.as_mut_slice().split_first_mut() {
			Some(head)
		} else {
			None
		}
	}

	pub fn non_residual_top_slice(&self) -> &RevSlice<Tranche<Balance, Rate, Weight, CurrencyId>> {
		self.tranches.rev()
	}

	pub fn non_residual_top_slice_mut(
		&mut self,
	) -> &mut RevSlice<Tranche<Balance, Rate, Weight, CurrencyId>> {
		self.tranches.rev_mut()
	}

	pub fn residual_top_slice(&self) -> &[Tranche<Balance, Rate, Weight, CurrencyId>] {
		self.tranches.as_slice()
	}

	pub fn residual_top_slice_mut(&mut self) -> &mut [Tranche<Balance, Rate, Weight, CurrencyId>] {
		self.tranches.as_mut_slice()
	}

	pub fn supplies(&self) -> Result<Vec<Balance>, DispatchError> {
		self.tranches
			.iter()
			.map(|tranche| tranche.debt.checked_add(&tranche.reserve))
			.collect::<Option<Vec<_>>>()
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn acc_supply(&self) -> Result<Balance, DispatchError> {
		self.tranches
			.iter()
			.fold(Some(Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| {
					acc.checked_add(&tranche.debt)
						.and_then(|acc| acc.checked_add(&tranche.reserve))
				})
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn outstanding_investments(&self) -> Vec<Balance> {
		self.tranches
			.iter()
			.map(|tranche| tranche.outstanding_invest_orders)
			.collect()
	}

	pub fn acc_outstanding_investments(&self) -> Result<Balance, DispatchError> {
		self.tranches
			.iter()
			.fold(Some(Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.outstanding_invest_orders))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn outstanding_redemptions(&self) -> Vec<Balance> {
		self.tranches
			.iter()
			.map(|tranche| tranche.outstanding_redeem_orders)
			.collect()
	}

	pub fn acc_outstanding_redemptions(&self) -> Result<Balance, DispatchError> {
		self.tranches
			.iter()
			.fold(Some(Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.outstanding_redeem_orders))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn calculate_weights(&self) -> Vec<(Weight, Weight)> {
		let n_tranches: u32 = self.tranches.len().try_into().expect("MaxTranches is u32");
		let redeem_starts = 10u128.checked_pow(n_tranches).unwrap_or(u128::MAX);

		// The desired order priority is:
		// - Senior redemptions
		// - Junior redemptions
		// - Junior investments
		// - Senior investments
		// We ensure this by having a higher base weight for redemptions,
		// increasing the redemption weights by seniority,
		// and decreasing the investment weight by seniority.
		self.tranches
			.iter()
			.map(|tranche| {
				(
					10u128
						.checked_pow(
							n_tranches
								.checked_sub(tranche.seniority)
								.unwrap_or(u32::MAX),
						)
						.unwrap_or(u128::MAX)
						.into(),
					redeem_starts
						.checked_mul(10u128.pow(tranche.seniority.saturating_add(1)))
						.unwrap_or(u128::MAX)
						.into(),
				)
			})
			.collect()
	}

	pub fn min_risk_buffers(&self) -> Vec<Perquintill> {
		self.tranches
			.iter()
			.map(|tranche| tranche.min_risk_buffer())
			.collect()
	}

	pub fn seniorities(&self) -> Vec<Seniority> {
		self.tranches
			.iter()
			.map(|tranche| tranche.seniority)
			.collect::<Vec<_>>()
	}

	pub fn rebalance_tranches(
		&mut self,
		now: Moment,
		pool_total_reserve: Balance,
		pool_nav: Balance,
		tranche_ratios: &[Perquintill],
		executed_amounts: &[(Balance, Balance)],
	) -> DispatchResult {
		// Calculate the new fraction of the total pool value that each tranche contains
		// This is based on the tranche values at time of epoch close.
		let total_assets = pool_total_reserve
			.checked_add(&pool_nav)
			.ok_or(ArithmeticError::Overflow)?;

		// Calculate the new total asset value for each tranche
		// This uses the current state of the tranches, rather than the cached epoch-close-time values.
		let mut total_assets = total_assets;
		let tranche_assets = self.combine_with_mut_non_residual_top(
			executed_amounts.iter().rev(),
			|tranche, (invest, redeem)| {
				tranche.accrue(now)?;

				tranche
					.debt
					.checked_add(&tranche.reserve)
					.ok_or(ArithmeticError::Overflow)?
					.checked_add(invest)
					.ok_or(ArithmeticError::Overflow)?
					.checked_sub(redeem)
					.ok_or(ArithmeticError::Underflow.into())
					.map(|value| {
						if value > total_assets {
							let assets = total_assets;
							total_assets = Zero::zero();
							assets
						} else {
							total_assets = total_assets
								.checked_sub(&value)
								.expect("total_assets greater equal value. qed.");
							value
						}
					})
			},
		)?;

		// Rebalance tranches based on the new tranche asset values and ratios
		let mut remaining_nav = pool_nav;
		let mut remaining_reserve = pool_total_reserve;
		self.combine_with_mut_non_residual_top(
			tranche_ratios.iter().rev().zip(tranche_assets.iter()),
			|tranche, (ratio, value)| {
				tranche.ratio = *ratio;
				if tranche.tranche_type == TrancheType::Residual {
					tranche.debt = remaining_nav;
					tranche.reserve = remaining_reserve;
				} else {
					tranche.debt = ratio.mul_ceil(pool_nav);
					if tranche.debt > *value {
						tranche.debt = *value;
					}
					tranche.reserve = value.saturating_sub(tranche.debt);
					remaining_nav =
						remaining_nav
							.checked_sub(&tranche.debt)
							.ok_or(DispatchError::Other(
							"Corrupted pool-state. Pool NAV should be able to handle tranche debt substraction.",
						))?;
					remaining_reserve =
						remaining_reserve
							.checked_sub(&tranche.reserve)
							.ok_or(DispatchError::Other(
							"Corrupted pool-state. Pool reserve should be able to handle tranche reserve substraction.",
						))?;
				}
				Ok(())
			},
		)
		.map(|_| ())
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct EpochExecutionTranche<Balance, BalanceRatio, Weight> {
	pub(super) supply: Balance,
	pub(super) price: BalanceRatio,
	pub(super) invest: Balance,
	pub(super) redeem: Balance,
	pub(super) min_risk_buffer: Perquintill,
	pub(super) seniority: Seniority,

	pub(super) _phantom: PhantomData<Weight>,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct EpochExecutionTranches<Balance, BalanceRatio, Weight> {
	tranches: Vec<EpochExecutionTranche<Balance, BalanceRatio, Weight>>,
}

impl<Balance, BalanceRatio, Weight> EpochExecutionTranches<Balance, BalanceRatio, Weight>
where
	Balance: Zero + Copy + BaseArithmetic + Unsigned + From<u64>,
	Weight: Copy + From<u128>,
	BalanceRatio: Copy,
{
	pub fn new(tranches: Vec<EpochExecutionTranche<Balance, BalanceRatio, Weight>>) -> Self {
		Self { tranches }
	}

	pub fn non_residual_tranches(
		&self,
	) -> Option<&[EpochExecutionTranche<Balance, BalanceRatio, Weight>]> {
		if let Some((_head, tail)) = self.tranches.as_slice().split_first() {
			Some(tail)
		} else {
			None
		}
	}

	pub fn non_residual_tranches_mut(
		&mut self,
	) -> Option<&mut [EpochExecutionTranche<Balance, BalanceRatio, Weight>]> {
		if let Some((_head, tail)) = self.tranches.as_mut_slice().split_first_mut() {
			Some(tail)
		} else {
			None
		}
	}

	pub fn residual_tranche(
		&self,
	) -> Option<&EpochExecutionTranche<Balance, BalanceRatio, Weight>> {
		if let Some((head, _tail)) = self.tranches.as_slice().split_first() {
			Some(head)
		} else {
			None
		}
	}

	pub fn residual_tranche_mut(
		&mut self,
	) -> Option<&mut EpochExecutionTranche<Balance, BalanceRatio, Weight>> {
		if let Some((head, _tail)) = self.tranches.as_mut_slice().split_first_mut() {
			Some(head)
		} else {
			None
		}
	}

	pub fn num_tranches(&self) -> usize {
		self.tranches.len()
	}

	pub fn into_tranches(self) -> Vec<EpochExecutionTranche<Balance, BalanceRatio, Weight>> {
		self.tranches
	}

	pub fn non_residual_top_slice(
		&self,
	) -> &RevSlice<EpochExecutionTranche<Balance, BalanceRatio, Weight>> {
		self.tranches.rev()
	}

	pub fn non_residual_top_slice_mut(
		&mut self,
	) -> &mut RevSlice<EpochExecutionTranche<Balance, BalanceRatio, Weight>> {
		self.tranches.rev_mut()
	}

	pub fn residual_top_slice(&self) -> &[EpochExecutionTranche<Balance, BalanceRatio, Weight>] {
		self.tranches.as_slice()
	}

	pub fn residual_top_slice_mut(
		&mut self,
	) -> &mut [EpochExecutionTranche<Balance, BalanceRatio, Weight>] {
		self.tranches.as_mut_slice()
	}

	pub fn combine_non_residual_top<R, F>(&self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&EpochExecutionTranche<Balance, BalanceRatio, Weight>) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		for tranche in self.tranches.iter().rev() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_mut_non_residual_top<R, F>(&mut self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&mut EpochExecutionTranche<Balance, BalanceRatio, Weight>,
		) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		for tranche in &mut self.tranches.iter_mut().rev() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_with_non_residual_top<R, I, W, F>(
		&self,
		with: I,
		mut f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&EpochExecutionTranche<Balance, BalanceRatio, Weight>,
			W,
		) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		let iter = self.tranches.iter().rev().zip(with.into_iter());

		for (tranche, w) in iter {
			let r = f(tranche, w)?;
			res.push(r);
		}

		Ok(res)
	}

	pub fn combine_with_mut_non_residual_top<R, W, I, F>(
		&mut self,
		with: I,
		mut f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&mut EpochExecutionTranche<Balance, BalanceRatio, Weight>,
			W,
		) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		let iter = self.tranches.iter_mut().rev().zip(with.into_iter());

		for (tranche, w) in iter {
			let r = f(tranche, w)?;
			res.push(r);
		}

		Ok(res)
	}

	pub fn combine_residual_top<R, F>(&self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&EpochExecutionTranche<Balance, BalanceRatio, Weight>) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		for tranche in self.tranches.iter() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_mut_residual_top<R, F>(&mut self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&mut EpochExecutionTranche<Balance, BalanceRatio, Weight>,
		) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		for tranche in self.tranches.iter_mut() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_with_residual_top<R, I, W, F>(
		&self,
		with: I,
		mut f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&EpochExecutionTranche<Balance, BalanceRatio, Weight>,
			W,
		) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		let iter = self.tranches.iter().zip(with.into_iter());

		for (tranche, w) in iter {
			let r = f(tranche, w)?;
			res.push(r);
		}

		Ok(res)
	}

	pub fn combine_with_mut_residual_top<R, W, I, F>(
		&mut self,
		with: I,
		mut f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&mut EpochExecutionTranche<Balance, BalanceRatio, Weight>,
			W,
		) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		let iter = self.tranches.iter_mut().zip(with.into_iter());

		for (tranche, w) in iter {
			let r = f(tranche, w)?;
			res.push(r);
		}

		Ok(res)
	}

	pub fn prices(&self) -> Vec<BalanceRatio> {
		self.tranches.iter().map(|tranche| tranche.price).collect()
	}

	pub fn supplies_with_fulfillment(
		&self,
		fulfillments: &[TrancheSolution],
	) -> Result<Vec<Balance>, DispatchError> {
		self.combine_with_residual_top(fulfillments, |tranche, solution| {
			tranche
				.supply
				.checked_add(&solution.invest_fulfillment.mul_floor(tranche.invest))
				.ok_or(ArithmeticError::Overflow)?
				.checked_sub(&solution.redeem_fulfillment.mul_floor(tranche.redeem))
				.ok_or(ArithmeticError::Underflow.into())
		})
	}

	pub fn acc_supply_with_fulfillment(
		&self,
		fulfillments: &[TrancheSolution],
	) -> Result<Balance, DispatchError> {
		self.supplies_with_fulfillment(fulfillments)?
			.iter()
			.fold(Some(Balance::zero()), |acc, add| {
				acc.and_then(|sum| sum.checked_add(add))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn supplies(&self) -> Vec<Balance> {
		self.tranches.iter().map(|tranche| tranche.supply).collect()
	}

	pub fn acc_supply(&self) -> Result<Balance, DispatchError> {
		self.tranches
			.iter()
			.fold(Some(Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.supply))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn investments(&self) -> Vec<Balance> {
		self.tranches.iter().map(|tranche| tranche.invest).collect()
	}

	pub fn acc_investments(&self) -> Result<Balance, DispatchError> {
		self.tranches
			.iter()
			.fold(Some(Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.invest))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn redemptions(&self) -> Vec<Balance> {
		self.tranches.iter().map(|tranche| tranche.redeem).collect()
	}

	pub fn acc_redemptions(&self) -> Result<Balance, DispatchError> {
		self.tranches
			.iter()
			.fold(Some(Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.redeem))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn calculate_weights(&self) -> Vec<(Weight, Weight)> {
		let n_tranches: u32 = self.tranches.len().try_into().expect("MaxTranches is u32");
		let redeem_starts = 10u128.checked_pow(n_tranches).unwrap_or(u128::MAX);

		// The desired order priority is:
		// - Senior redemptions
		// - Junior redemptions
		// - Junior investments
		// - Senior investments
		// We ensure this by having a higher base weight for redemptions,
		// increasing the redemption weights by seniority,
		// and decreasing the investment weight by seniority.
		self.tranches
			.iter()
			.map(|tranche| {
				(
					10u128
						.checked_pow(
							n_tranches
								.checked_sub(tranche.seniority)
								.unwrap_or(u32::MAX),
						)
						.unwrap_or(u128::MAX)
						.into(),
					redeem_starts
						.checked_mul(10u128.pow(tranche.seniority.saturating_add(1)))
						.unwrap_or(u128::MAX)
						.into(),
				)
			})
			.collect()
	}

	pub fn min_risk_buffers(&self) -> Vec<Perquintill> {
		self.tranches
			.iter()
			.map(|tranche| tranche.min_risk_buffer)
			.collect()
	}
}

pub(crate) fn calculate_risk_buffers<Balance, BalanceRatio>(
	tranche_supplies: &[Balance],
	tranche_prices: &[BalanceRatio],
) -> Result<Vec<Perquintill>, DispatchError>
where
	BalanceRatio: Copy + FixedPointNumber,
	Balance: Copy + BaseArithmetic + FixedPointOperand + Unsigned + From<u64>,
{
	let tranche_values: Vec<_> = tranche_supplies
		.iter()
		.zip(tranche_prices)
		.map(|(supply, price)| price.checked_mul_int(*supply))
		.collect::<Option<Vec<_>>>()
		.ok_or(ArithmeticError::Overflow)?;

	let pool_value = tranche_values
		.iter()
		.fold(Some(Zero::zero()), |sum: Option<Balance>, tranche_value| {
			sum.and_then(|sum| sum.checked_add(tranche_value))
		})
		.ok_or(ArithmeticError::Overflow)?;

	// Iterate over the tranches senior => junior.
	// Buffer of most senior tranche is pool value -y senior tranche value.
	// Buffer of each subordinate tranche is the buffer of the
	// previous more senior tranche - this tranche value.
	let mut remaining_subordinate_value = pool_value;
	let mut risk_buffers: Vec<Perquintill> = tranche_values
		.iter()
		.rev()
		.map(|tranche_value| {
			remaining_subordinate_value = remaining_subordinate_value
				.checked_sub(tranche_value)
				.unwrap_or(Zero::zero());
			Perquintill::from_rational(remaining_subordinate_value, pool_value)
		})
		.collect::<Vec<Perquintill>>();

	risk_buffers.reverse();

	Ok(risk_buffers)
}

#[cfg(test)]
pub mod test {
	#[test]
	fn reverse_slice_panics_on_out_of_bounds() {}

	#[test]
	fn reverse_works_for_both_tranches() {}

	#[test]
	fn accrue_overflows_safely() {}
}
