use frame_support::weights::Weight;

pub trait WeightInfo {
	fn update_portfolio_valuation(x: u32, y: u32) -> Weight;
}

impl WeightInfo for () {
	fn update_portfolio_valuation(_: u32, _: u32) -> Weight {
		Weight::zero()
	}
}
