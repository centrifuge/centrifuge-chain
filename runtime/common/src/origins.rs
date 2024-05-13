// Copyright 2024 Centrifuge Foundation (centrifuge.io).
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

/// Custom origins for governance interventions.
pub mod gov {
	pub use pallet_custom_origins::*;

	#[frame_support::pallet]
	pub mod pallet_custom_origins {
		use frame_support::pallet_prelude::*;

		#[pallet::config]
		pub trait Config: frame_system::Config {}

		#[pallet::pallet]
		pub struct Pallet<T>(_);

		#[derive(PartialEq, Eq, Clone, MaxEncodedLen, Encode, Decode, TypeInfo, RuntimeDebug)]
		#[pallet::origin]
		pub enum Origin {
			/// Origin able to dispatch a whitelisted call.
			WhitelistedCaller,
			/// Origin for spending (any amount of) funds.
			Treasurer,
			/// Origin for pool related referenda.
			PoolAdmin,
			/// Origin able to cancel referenda.
			ReferendumCanceller,
			/// Origin able to kill referenda.
			ReferendumKiller,
		}

		macro_rules! decl_unit_ensures {
			( $name:ident: $success_type:ty = $success:expr ) => {
				pub struct $name;
				impl<O: Into<Result<Origin, O>> + From<Origin>>
					EnsureOrigin<O> for $name
				{
					type Success = $success_type;
					fn try_origin(o: O) -> Result<Self::Success, O> {
						o.into().and_then(|o| match o {
							Origin::$name => Ok($success),
							r => Err(O::from(r)),
						})
					}
					#[cfg(feature = "runtime-benchmarks")]
					fn try_successful_origin() -> Result<O, ()> {
						Ok(O::from(Origin::$name))
					}
				}
			};
			( $name:ident ) => { decl_unit_ensures! { $name : () = () } };
			( $name:ident: $success_type:ty = $success:expr, $( $rest:tt )* ) => {
				decl_unit_ensures! { $name: $success_type = $success }
				decl_unit_ensures! { $( $rest )* }
			};
			( $name:ident, $( $rest:tt )* ) => {
				decl_unit_ensures! { $name }
				decl_unit_ensures! { $( $rest )* }
			};
			() => {}
		}

		decl_unit_ensures!(
			WhitelistedCaller,
			PoolAdmin,
			Treasurer,
			ReferendumCanceller,
			ReferendumKiller,
		);
	}

	pub mod types {
		use cfg_primitives::AccountId;
		use frame_support::traits::{EitherOf, EitherOfDiverse};
		use frame_system::EnsureRoot;
		use pallet_collective::EnsureProportionAtLeast;

		use super::*;
		use crate::instances::{CouncilCollective, TechnicalCollective};

		// Ensure that origin is either Root or fallback to use EnsureOrigin `O`
		pub type EnsureRootOr<O> = EitherOfDiverse<EnsureRoot<AccountId>, O>;

		/// All council members must vote yes to create this origin.
		pub type AllOfCouncil = EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 1>;

		/// 1/2 of all council members must vote yes to create this origin.
		pub type HalfOfCouncil = EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 2>;

		/// 2/3 of all council members must vote yes to create this origin.
		pub type TwoThirdOfCouncil = EnsureProportionAtLeast<AccountId, CouncilCollective, 2, 3>;

		/// 3/4 of all council members must vote yes to create this origin.
		pub type ThreeFourthOfCouncil = EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 4>;

		/// 1/2 of all technical committee members must vote yes to create this
		/// origin.
		pub type HalfOfTechnicalCommitte =
			EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 2>;

		/// The origin which approves and rejects treasury spends.
		///
		/// NOTE: The council will be removed once the OpenGov transition has
		/// concluded.
		pub type TreasuryApproveOrigin = EnsureRootOr<EitherOf<TwoThirdOfCouncil, Treasurer>>;

		/// The origin which can whitelist calls.
		pub type WhitelistOrigin = EnsureRootOr<HalfOfTechnicalCommitte>;

		/// The origin which dispatches whitelisted calls.
		pub type DispatchWhitelistedOrigin = EnsureRootOr<WhitelistedCaller>;

		/// The origin which can cancel ongoing referenda.
		pub type RefCancelOrigin = EnsureRootOr<ReferendumCanceller>;

		/// The origin which can kill ongoing referenda.
		pub type RefKillerOrigin = EnsureRootOr<ReferendumKiller>;

		/// The origin which can create new pools.
		pub type PoolCreateOrigin = EnsureRootOr<PoolAdmin>;
	}
}
