use crate::{anchor, proofs};
use rstd::vec::Vec;
use support::{decl_event, decl_module, dispatch::Result, ensure};
use system::ensure_signed;

pub trait Trait: anchor::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_event!(
    pub enum Event<T> where <T as system::Trait>::Hash {
        DepositAsset(Hash),
    }
);

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin  {
        fn deposit_event() = default;

        fn validate_mint(origin, anchor_id: T::Hash, deposit_address: [u8; 20], pfs: Vec<proofs::Proof>) -> Result {
            ensure_signed(origin)?;

            // get the anchor data from anchor ID
            let anchor_data = <anchor::Module<T>>::get_anchor_by_id(anchor_id).ok_or("Anchor doesn't exist")?;

            // validate proofs
            ensure!(Self::validate_proofs(anchor_data.get_doc_root(), &pfs), "Invalid proofs");

            // get the bundled hash
            let bh = Self::get_bundled_hash(pfs, deposit_address);

            Self::deposit_event(RawEvent::DepositAsset(bh));

            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    fn validate_proofs(doc_root: T::Hash, pfs: &Vec<proofs::Proof>) -> bool {
        let mut dr: [u8; 32] = Default::default();
        dr.clone_from_slice(doc_root.as_ref());
        proofs::validate_proofs(dr, pfs)
    }

    fn get_bundled_hash(pfs: Vec<proofs::Proof>, deposit_address: [u8; 20]) -> T::Hash {
        let bh = proofs::bundled_hash(pfs, deposit_address);
        let mut res: T::Hash = Default::default();
        res.as_mut().copy_from_slice(&bh);
        res
    }
}
