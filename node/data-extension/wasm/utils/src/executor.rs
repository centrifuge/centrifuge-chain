use wasmtime::{AsContext, AsContextMut, Config, Engine, Instance, Module, Store};

use crate::{checked_range, unpack_ptr_and_len};

pub(crate) type InnerError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("Engine build error: {0}")]
	EngineBuild(InnerError),

	#[error("Module build error: {0}")]
	ModuleBuild(InnerError),

	#[error("Memory not found")]
	MemoryNotFound,

	#[error("Memory grow error: {0}")]
	MemoryGrow(InnerError),

	#[error("Memory write error: {0}")]
	MemoryWrite(InnerError),

	#[error("Invalid output range")]
	InvalidOutputRange,

	#[error("Instance creation error: {0}")]
	InstanceCreation(InnerError),

	#[error("Wasm fn retrieval error: {0}")]
	WasmFnRetrieval(InnerError),

	#[error("Wasm fn call error: {0}")]
	WasmFnCall(InnerError),
}

pub trait WasmExecutor {
	fn call_fn(&self, fn_name: &str, input_data: &[u8]) -> Result<Vec<u8>, Error>;
}

pub struct InMemoryWasmExecutor {
	engine: Engine,
	module: Module,
}

const MEMORY_NAME: &str = "memory";

impl InMemoryWasmExecutor {
	pub fn new(wasm_blob: &[u8]) -> Result<Self, Error> {
		let engine = Engine::new(&Config::default()).map_err(|e| Error::EngineBuild(e.into()))?;
		let module = Module::new(&engine, wasm_blob).map_err(|e| Error::ModuleBuild(e.into()))?;

		Ok(Self { engine, module })
	}

	fn write_to_wasm_memory(
		&self,
		instance: &Instance,
		store: &mut Store<()>,
		input_data: &[u8],
	) -> Result<(u32, u32), Error> {
		let memory = instance
			.get_memory(store.as_context_mut(), MEMORY_NAME)
			.ok_or(Error::MemoryNotFound)?;

		let offset = memory.size(store.as_context());

		memory
			.grow(store.as_context_mut(), input_data.len() as u64)
			.map_err(|e| Error::MemoryGrow(e.into()))?;

		memory
			.write(store.as_context_mut(), offset as usize, input_data)
			.map_err(|e| Error::MemoryWrite(e.into()))?;

		Ok((offset as u32, input_data.len() as u32))
	}

	fn read_from_wasm_memory(
		&self,
		instance: &Instance,
		store: &mut Store<()>,
		output_ptr: u32,
		output_len: u32,
	) -> Result<Vec<u8>, Error> {
		let mut dest = vec![0u8; output_len as usize];

		let memory = instance
			.get_memory(store.as_context_mut(), MEMORY_NAME)
			.ok_or(Error::MemoryNotFound)?;

		let mem_data = memory.data(store.as_context());
		let mem_size = mem_data.len();

		let range = checked_range(output_ptr as usize, output_len as usize, mem_size)
			.ok_or(Error::InvalidOutputRange)?;

		dest.copy_from_slice(&mem_data[range]);

		Ok(dest)
	}
}

impl WasmExecutor for InMemoryWasmExecutor {
	fn call_fn(&self, fn_name: &str, input_data: &[u8]) -> Result<Vec<u8>, Error> {
		let mut store = Store::new(&self.engine, ());
		let instance = Instance::new(&mut store, &self.module, &[])
			.map_err(|e| Error::InstanceCreation(e.into()))?;

		let (input_ptr, input_len) =
			self.write_to_wasm_memory(&instance, &mut store, input_data)?;

		let wasm_fn = instance
			.get_typed_func::<(u32, u32), u64>(&mut store, fn_name)
			.map_err(|e| Error::WasmFnRetrieval(e.into()))?;

		let wasm_res = wasm_fn
			.call(&mut store, (input_ptr, input_len))
			.map_err(|e| Error::WasmFnCall(e.into()))?;

		let (output_ptr, output_len) = unpack_ptr_and_len(wasm_res);

		self.read_from_wasm_memory(&instance, &mut store, output_ptr, output_len)
	}
}

#[cfg(test)]
mod test {
	use std::fs;

	use serde::{Deserialize, Serialize};

	use crate::{InMemoryWasmExecutor, WasmExecutor};

	#[derive(Debug, Serialize, Deserialize)]
	pub struct TestStruct {
		vec: Vec<u8>,
	}

	/// The wasm tested here was generated from:
	///
	/// ```no_run
	/// use std::slice::from_raw_parts;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Serialize, Deserialize)]
	/// pub struct TestStruct {
	///     vec: Vec<u8>,
	/// }
	///
	/// #[no_mangle]
	/// pub unsafe fn test_slice(input_ptr: *mut u8, input_len: usize) -> u64 {
	///     let ret = if input_len == 0 { &[0u8; 0] } else { unsafe { from_raw_parts(input_ptr, input_len) } };
	///
	///     let mut r: TestStruct = serde_json::from_slice(ret).expect("can decode TestStruct");
	///
	///     r.vec.push(11);
	///
	///     let res = serde_json::to_vec(&r).expect("can encode test struct");
	///
	///     return_from_wasm(res.as_slice())
	/// }
	///
	/// pub fn return_from_wasm(value: &[u8]) -> u64 {
	///     let ptr = value.as_ptr() as u64;
	///     let length = value.len() as u64;
	///     let res = ptr | (length << 32);
	///
	///     res
	/// }
	/// ```
	#[test]
	fn call_works() {
		let wasm_bytes = fs::read("./src/test/test.wasm").unwrap();

		let ex = InMemoryWasmExecutor::new(wasm_bytes.as_slice()).expect("can build wasm executor");

		let test_struct = TestStruct {
			vec: vec![1, 2, 3, 4, 5],
		};
		let test_bytes = serde_json::to_vec(&test_struct).expect("can serialize test struct");

		let res = ex
			.call_fn("test_slice", test_bytes.as_slice())
			.expect("can call wasm fn");

		let second_struct: TestStruct =
			serde_json::from_slice(&res).expect("can decode test struct");

		let expected_vec = vec![1, 2, 3, 4, 5, 11];

		assert_eq!(expected_vec, second_struct.vec);
	}
}
