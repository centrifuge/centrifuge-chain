use std::ops::Range;

pub fn checked_range(offset: usize, len: usize, max: usize) -> Option<Range<usize>> {
	let end = offset.checked_add(len)?;
	(end <= max).then(|| offset..end)
}

pub fn unpack_ptr_and_len(val: u64) -> (u32, u32) {
	let ptr = (val & (!0u32 as u64)) as u32;
	let len = (val >> 32) as u32;

	(ptr, len)
}

pub fn pack_ptr_and_len(value: &[u8]) -> u64 {
	let ptr = value.as_ptr() as u64;
	let length = value.len() as u64;
	let res = ptr | (length << 32);

	res
}
