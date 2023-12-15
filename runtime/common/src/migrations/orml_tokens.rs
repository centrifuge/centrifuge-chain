use frame_support::traits::OnRuntimeUpgrade;

pub struct Migration<T: orml_tokens::Config + frame_system::Config>(sp_std::marker::PhantomData<T>);

impl<T> OnRuntimeUpgrade for Migration<T> where T: orml_tokens::Config + frame_system::Config {}
