use frame_support::sp_runtime::traits::{IdentifyAccount, Member, AtLeast32BitUnsigned, MaybeSerializeDeserialize, MaybeSerialize};
use frame_support::Parameter;
use frame_support::dispatch::Codec;
use frame_support::dispatch::fmt::Debug;
use sp_runtime::traits::MaybeDisplay;

/// Reward Trait of the Claim Pallet
///
/// This trait defines the functionality a Reward Pallet must satisfy, so that it can be used
/// with the Claim Pallet.
pub trait Reward {
    /// The account from the parachain, that the claimer provided in his claim call.
    type ParachainAccountId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeSerialize + Ord
        + Default;
    /// The contribution amount in the token of the relay chain.
    type ContributionAmount: Parameter + Member + AtLeast32BitUnsigned + Codec + Default + Copy +
        MaybeSerializeDeserialize + Debug;

    /// Rewarding function that will be called ones the Claim Pallet has verified the claimer
    /// If this function returns successfully, any subsequent claim of the same claimer will be
    /// rejected by the claim module.
    fn reward(who: &Self::ParachainAccountId, contribution: &Self::ContributionAmount) -> Result<(),()>;
}
