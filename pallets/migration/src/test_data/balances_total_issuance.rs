pub const TOTAL_ISSUANCE: TotalIssuanceKeyValue = TotalIssuanceKeyValue {
	key: [
		194, 38, 18, 118, 204, 157, 31, 133, 152, 234, 75, 106, 116, 177, 92, 47, 87, 200, 117,
		228, 207, 247, 65, 72, 228, 98, 143, 38, 75, 151, 76, 128,
	],
	value: [
		11, 33, 147, 140, 187, 124, 58, 152, 71, 94, 99, 1, 0, 0, 0, 0,
	],
};

pub struct TotalIssuanceKeyValue {
	pub key: [u8; 32],
	pub value: [u8; 16],
}
