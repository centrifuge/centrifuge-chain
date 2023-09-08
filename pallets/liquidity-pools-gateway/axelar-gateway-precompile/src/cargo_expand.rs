#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2021::*;
#[macro_use]
extern crate std;
use cfg_types::domain_address::{Domain, DomainAddress};
use codec::alloc::string::ToString;
use ethabi::Token;
use fp_evm::PrecompileHandle;
use frame_support::{Blake2_256, StorageHasher};
use pallet_evm::{ExitError, PrecompileFailure};
use precompile_utils::prelude::*;
use sp_core::{bounded::BoundedVec, ConstU32, H160, H256, U256};
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::vec::Vec;
pub use crate::weights::WeightInfo;
pub const MAX_SOURCE_CHAIN_BYTES: u32 = 128;
pub const MAX_SOURCE_ADDRESS_BYTES: u32 = 32;
pub const MAX_TOKEN_SYMBOL_BYTES: u32 = 32;
pub const MAX_PAYLOAD_BYTES: u32 = 1024;
pub const PREFIX_CONTRACT_CALL_APPROVED: [u8; 32] = [
    0x07u8, 0xb0, 0xd4, 0x30, 0x4f, 0x82, 0x01, 0x2b, 0xd3, 0xb7, 0x0b, 0x1d, 0x53, 0x1c, 0x16,
    0x0e, 0x32, 0x60, 0x67, 0xc9, 0x08, 0x29, 0xe2, 0xa3, 0xd3, 0x86, 0x72, 0x2a, 0xd1, 0x0b, 0x89,
    0xc3,
];
pub type String<const U32: u32> = BoundedString<ConstU32<U32>>;
pub type Bytes<const U32: u32> = BoundedBytes<ConstU32<U32>>;
pub use pallet::*;
pub mod weights {
    use frame_support::weights::{constants::RocksDbWeight, Weight};
    pub trait WeightInfo {
        fn set_gateway() -> Weight;
        fn set_converter() -> Weight;
    }
    impl WeightInfo for () {
        fn set_gateway() -> Weight {
            Weight::from_parts(17_000_000, 5991)
                .saturating_add(RocksDbWeight::get().reads(2))
                .saturating_add(RocksDbWeight::get().writes(1))
        }
        fn set_converter() -> Weight {
            Weight::from_parts(17_000_000, 5991)
                .saturating_add(RocksDbWeight::get().reads(2))
                .saturating_add(RocksDbWeight::get().writes(1))
        }
    }
}
pub struct SourceConverter {
    domain: Domain,
}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for SourceConverter {}
#[automatically_derived]
impl ::core::cmp::PartialEq for SourceConverter {
    #[inline]
    fn eq(&self, other: &SourceConverter) -> bool {
        self.domain == other.domain
    }
}
#[automatically_derived]
impl ::core::clone::Clone for SourceConverter {
    #[inline]
    fn clone(&self) -> SourceConverter {
        SourceConverter {
            domain: ::core::clone::Clone::clone(&self.domain),
        }
    }
}
#[allow(deprecated)]
const _: () = {
    #[automatically_derived]
    impl ::codec::Encode for SourceConverter {
        fn size_hint(&self) -> usize {
            ::codec::Encode::size_hint(&&self.domain)
        }
        fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
            &self,
            __codec_dest_edqy: &mut __CodecOutputEdqy,
        ) {
            ::codec::Encode::encode_to(&&self.domain, __codec_dest_edqy)
        }
        fn encode(&self) -> ::codec::alloc::vec::Vec<::core::primitive::u8> {
            ::codec::Encode::encode(&&self.domain)
        }
        fn using_encoded<R, F: ::core::ops::FnOnce(&[::core::primitive::u8]) -> R>(
            &self,
            f: F,
        ) -> R {
            ::codec::Encode::using_encoded(&&self.domain, f)
        }
    }
    #[automatically_derived]
    impl ::codec::EncodeLike for SourceConverter {}
};
#[allow(deprecated)]
const _: () = {
    #[automatically_derived]
    impl ::codec::Decode for SourceConverter {
        fn decode<__CodecInputEdqy: ::codec::Input>(
            __codec_input_edqy: &mut __CodecInputEdqy,
        ) -> ::core::result::Result<Self, ::codec::Error> {
            ::core::result::Result::Ok(SourceConverter {
                domain: {
                    let __codec_res_edqy = <Domain as ::codec::Decode>::decode(__codec_input_edqy);
                    match __codec_res_edqy {
                        ::core::result::Result::Err(e) => {
                            return ::core::result::Result::Err(
                                e.chain("Could not decode `SourceConverter::domain`"),
                            )
                        }
                        ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                    }
                },
            })
        }
    }
};
#[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
const _: () = {
    impl ::scale_info::TypeInfo for SourceConverter {
        type Identity = Self;
        fn type_info() -> ::scale_info::Type {
            ::scale_info::Type::builder()
                .path(::scale_info::Path::new(
                    "SourceConverter",
                    "axelar_gateway_precompile",
                ))
                .type_params(::alloc::vec::Vec::new())
                .composite(
                    ::scale_info::build::Fields::named()
                        .field(|f| f.ty::<Domain>().name("domain").type_name("Domain")),
                )
        }
    };
};
const _: () = {
    impl ::codec::MaxEncodedLen for SourceConverter {
        fn max_encoded_len() -> ::core::primitive::usize {
            0_usize.saturating_add(<Domain>::max_encoded_len())
        }
    }
};
const _: () = {
    impl core::fmt::Debug for SourceConverter {
        fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
            fmt.debug_struct("SourceConverter")
                .field("domain", &self.domain)
                .finish()
        }
    }
};
impl SourceConverter {
    pub fn try_convert(&self, maybe_address: &[u8]) -> Option<DomainAddress> {
        match self.domain {
            Domain::Centrifuge => Some(DomainAddress::Centrifuge(Self::try_into_32bytes(
                maybe_address,
            )?)),
            Domain::EVM(id) => Some(DomainAddress::EVM(
                id,
                Self::try_into_20bytes(maybe_address)?,
            )),
        }
    }
    fn try_into_32bytes(maybe_address: &[u8]) -> Option<[u8; 32]> {
        if maybe_address.len() == 32 {
            let mut address: [u8; 32] = [0u8; 32];
            address.copy_from_slice(maybe_address);
            Some(address)
        } else {
            None
        }
    }
    fn try_into_20bytes(maybe_address: &[u8]) -> Option<[u8; 20]> {
        if maybe_address.len() == 20 {
            let mut address: [u8; 20] = [0u8; 20];
            address.copy_from_slice(maybe_address);
            Some(address)
        } else {
            None
        }
    }
}
///
///			The module that hosts all the
///			[FRAME](https://docs.substrate.io/main-docs/build/events-errors/)
///			types needed to add this pallet to a
///			runtime.
///
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_core::{H160, H256};
    use super::SourceConverter;
    use crate::weights::WeightInfo;
    ///
    ///			The [pallet](https://docs.substrate.io/reference/frame-pallets/#pallets) implementing
    ///			the on-chain logic.
    ///
    pub struct Pallet<T>(frame_support::sp_std::marker::PhantomData<(T)>);
    const _: () = {
        impl<T> core::clone::Clone for Pallet<T> {
            fn clone(&self) -> Self {
                Self(core::clone::Clone::clone(&self.0))
            }
        }
    };
    const _: () = {
        impl<T> core::cmp::Eq for Pallet<T> {}
    };
    const _: () = {
        impl<T> core::cmp::PartialEq for Pallet<T> {
            fn eq(&self, other: &Self) -> bool {
                true && self.0 == other.0
            }
        }
    };
    const _: () = {
        impl<T> core::fmt::Debug for Pallet<T> {
            fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
                fmt.debug_tuple("Pallet").field(&self.0).finish()
            }
        }
    };
    ///
    ///			Configuration trait of this pallet.
    ///
    ///			Implement this type for a runtime in order to customize this pallet.
    ///
    pub trait Config:
        frame_system::Config + pallet_evm::Config + pallet_liquidity_pools_gateway::Config
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The origin that is allowed to set the gateway address we accept
        /// messageas from
        type AdminOrigin: EnsureOrigin<<Self as frame_system::Config>::RuntimeOrigin>;
        type WeightInfo: WeightInfo;
    }
    #[allow(type_alias_bounds)]
    pub type AxelarGatewayContract<T: Config> =
        StorageValue<_GeneratedPrefixForStorageAxelarGatewayContract<T>, H160, ValueQuery>;
    /// `SourceConversion` is a `hash_of(Vec<u8>)` where the `Vec<u8>` is the
    /// blake256-hash of the source-chain identifier used by the Axelar network.
    #[allow(type_alias_bounds)]
    pub type SourceConversion<T: Config> = StorageMap<
        _GeneratedPrefixForStorageSourceConversion<T>,
        Twox64Concat,
        H256,
        SourceConverter,
    >;
    ///
    ///					Can be used to configure the
    ///					[genesis state](https://docs.substrate.io/v3/runtime/chain-specs#the-genesis-state)
    ///					of this pallet.
    ///
    #[cfg(feature = "std")]
    #[serde(rename_all = "camelCase")]
    #[serde(deny_unknown_fields)]
    #[serde(bound(serialize = ""))]
    #[serde(bound(deserialize = ""))]
    #[serde(crate = "frame_support::serde")]
    pub struct GenesisConfig<T> {
        pub gateway: H160,
        _phantom: core::marker::PhantomData<T>,
    }
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        use frame_support::serde as _serde;
        #[automatically_derived]
        impl<T> frame_support::serde::Serialize for GenesisConfig<T> {
            fn serialize<__S>(
                &self,
                __serializer: __S,
            ) -> frame_support::serde::__private::Result<__S::Ok, __S::Error>
            where
                __S: frame_support::serde::Serializer,
            {
                let mut __serde_state = _serde::Serializer::serialize_struct(
                    __serializer,
                    "GenesisConfig",
                    false as usize + 1 + 1,
                )?;
                _serde::ser::SerializeStruct::serialize_field(
                    &mut __serde_state,
                    "gateway",
                    &self.gateway,
                )?;
                _serde::ser::SerializeStruct::serialize_field(
                    &mut __serde_state,
                    "phantom",
                    &self._phantom,
                )?;
                _serde::ser::SerializeStruct::end(__serde_state)
            }
        }
    };
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        use frame_support::serde as _serde;
        #[automatically_derived]
        impl<'de, T> frame_support::serde::Deserialize<'de> for GenesisConfig<T> {
            fn deserialize<__D>(
                __deserializer: __D,
            ) -> frame_support::serde::__private::Result<Self, __D::Error>
            where
                __D: frame_support::serde::Deserializer<'de>,
            {
                #[allow(non_camel_case_types)]
                #[doc(hidden)]
                enum __Field {
                    __field0,
                    __field1,
                }
                #[doc(hidden)]
                struct __FieldVisitor;
                impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                    type Value = __Field;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "field identifier")
                    }
                    fn visit_u64<__E>(
                        self,
                        __value: u64,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            0u64 => _serde::__private::Ok(__Field::__field0),
                            1u64 => _serde::__private::Ok(__Field::__field1),
                            _ => _serde::__private::Err(_serde::de::Error::invalid_value(
                                _serde::de::Unexpected::Unsigned(__value),
                                &"field index 0 <= i < 2",
                            )),
                        }
                    }
                    fn visit_str<__E>(
                        self,
                        __value: &str,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            "gateway" => _serde::__private::Ok(__Field::__field0),
                            "phantom" => _serde::__private::Ok(__Field::__field1),
                            _ => _serde::__private::Err(_serde::de::Error::unknown_field(
                                __value, FIELDS,
                            )),
                        }
                    }
                    fn visit_bytes<__E>(
                        self,
                        __value: &[u8],
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            b"gateway" => _serde::__private::Ok(__Field::__field0),
                            b"phantom" => _serde::__private::Ok(__Field::__field1),
                            _ => {
                                let __value = &_serde::__private::from_utf8_lossy(__value);
                                _serde::__private::Err(_serde::de::Error::unknown_field(
                                    __value, FIELDS,
                                ))
                            }
                        }
                    }
                }
                impl<'de> _serde::Deserialize<'de> for __Field {
                    #[inline]
                    fn deserialize<__D>(
                        __deserializer: __D,
                    ) -> _serde::__private::Result<Self, __D::Error>
                    where
                        __D: _serde::Deserializer<'de>,
                    {
                        _serde::Deserializer::deserialize_identifier(__deserializer, __FieldVisitor)
                    }
                }
                #[doc(hidden)]
                struct __Visitor<'de, T> {
                    marker: _serde::__private::PhantomData<GenesisConfig<T>>,
                    lifetime: _serde::__private::PhantomData<&'de ()>,
                }
                impl<'de, T> _serde::de::Visitor<'de> for __Visitor<'de, T> {
                    type Value = GenesisConfig<T>;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "struct GenesisConfig")
                    }
                    #[inline]
                    fn visit_seq<__A>(
                        self,
                        mut __seq: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::SeqAccess<'de>,
                    {
                        let __field0 =
                            match _serde::de::SeqAccess::next_element::<H160>(&mut __seq)? {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            0usize,
                                            &"struct GenesisConfig with 2 elements",
                                        ),
                                    )
                                }
                            };
                        let __field1 = match _serde::de::SeqAccess::next_element::<
                            core::marker::PhantomData<T>,
                        >(&mut __seq)?
                        {
                            _serde::__private::Some(__value) => __value,
                            _serde::__private::None => {
                                return _serde::__private::Err(_serde::de::Error::invalid_length(
                                    1usize,
                                    &"struct GenesisConfig with 2 elements",
                                ))
                            }
                        };
                        _serde::__private::Ok(GenesisConfig {
                            gateway: __field0,
                            _phantom: __field1,
                        })
                    }
                    #[inline]
                    fn visit_map<__A>(
                        self,
                        mut __map: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::MapAccess<'de>,
                    {
                        let mut __field0: _serde::__private::Option<H160> = _serde::__private::None;
                        let mut __field1: _serde::__private::Option<core::marker::PhantomData<T>> =
                            _serde::__private::None;
                        while let _serde::__private::Some(__key) =
                            _serde::de::MapAccess::next_key::<__Field>(&mut __map)?
                        {
                            match __key {
                                __Field::__field0 => {
                                    if _serde::__private::Option::is_some(&__field0) {
                                        return _serde::__private::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field(
                                                "gateway",
                                            ),
                                        );
                                    }
                                    __field0 = _serde::__private::Some(
                                        _serde::de::MapAccess::next_value::<H160>(&mut __map)?,
                                    );
                                }
                                __Field::__field1 => {
                                    if _serde::__private::Option::is_some(&__field1) {
                                        return _serde::__private::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field(
                                                "phantom",
                                            ),
                                        );
                                    }
                                    __field1 = _serde::__private::Some(
                                        _serde::de::MapAccess::next_value::<
                                            core::marker::PhantomData<T>,
                                        >(&mut __map)?,
                                    );
                                }
                            }
                        }
                        let __field0 = match __field0 {
                            _serde::__private::Some(__field0) => __field0,
                            _serde::__private::None => {
                                _serde::__private::de::missing_field("gateway")?
                            }
                        };
                        let __field1 = match __field1 {
                            _serde::__private::Some(__field1) => __field1,
                            _serde::__private::None => {
                                _serde::__private::de::missing_field("phantom")?
                            }
                        };
                        _serde::__private::Ok(GenesisConfig {
                            gateway: __field0,
                            _phantom: __field1,
                        })
                    }
                }
                #[doc(hidden)]
                const FIELDS: &'static [&'static str] = &["gateway", "phantom"];
                _serde::Deserializer::deserialize_struct(
                    __deserializer,
                    "GenesisConfig",
                    FIELDS,
                    __Visitor {
                        marker: _serde::__private::PhantomData::<GenesisConfig<T>>,
                        lifetime: _serde::__private::PhantomData,
                    },
                )
            }
        }
    };
    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            GenesisConfig {
                gateway: Default::default(),
                _phantom: Default::default(),
            }
        }
    }
    #[cfg(feature = "std")]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            AxelarGatewayContract::<T>::set(self.gateway)
        }
    }
    ///
    ///			The [event](https://docs.substrate.io/main-docs/build/events-errors/) emitted
    ///			by this pallet.
    ///
    #[scale_info(skip_type_params(T), capture_docs = "always")]
    pub enum Event<T: Config> {
        GatewaySet {
            address: H160,
        },
        ConverterSet {
            id_hash: H256,
            converter: SourceConverter,
        },
        #[doc(hidden)]
        #[codec(skip)]
        __Ignore(
            frame_support::sp_std::marker::PhantomData<(T)>,
            frame_support::Never,
        ),
    }
    const _: () = {
        impl<T: Config> core::clone::Clone for Event<T> {
            fn clone(&self) -> Self {
                match self {
                    Self::GatewaySet { ref address } => Self::GatewaySet {
                        address: core::clone::Clone::clone(address),
                    },
                    Self::ConverterSet {
                        ref id_hash,
                        ref converter,
                    } => Self::ConverterSet {
                        id_hash: core::clone::Clone::clone(id_hash),
                        converter: core::clone::Clone::clone(converter),
                    },
                    Self::__Ignore(ref _0, ref _1) => {
                        Self::__Ignore(core::clone::Clone::clone(_0), core::clone::Clone::clone(_1))
                    }
                }
            }
        }
    };
    const _: () = {
        impl<T: Config> core::cmp::Eq for Event<T> {}
    };
    const _: () = {
        impl<T: Config> core::cmp::PartialEq for Event<T> {
            fn eq(&self, other: &Self) -> bool {
                match (self, other) {
                    (Self::GatewaySet { address }, Self::GatewaySet { address: _0 }) => {
                        true && address == _0
                    }
                    (
                        Self::ConverterSet { id_hash, converter },
                        Self::ConverterSet {
                            id_hash: _0,
                            converter: _1,
                        },
                    ) => true && id_hash == _0 && converter == _1,
                    (Self::__Ignore(_0, _1), Self::__Ignore(_0_other, _1_other)) => {
                        true && _0 == _0_other && _1 == _1_other
                    }
                    (Self::GatewaySet { .. }, Self::ConverterSet { .. }) => false,
                    (Self::GatewaySet { .. }, Self::__Ignore { .. }) => false,
                    (Self::ConverterSet { .. }, Self::GatewaySet { .. }) => false,
                    (Self::ConverterSet { .. }, Self::__Ignore { .. }) => false,
                    (Self::__Ignore { .. }, Self::GatewaySet { .. }) => false,
                    (Self::__Ignore { .. }, Self::ConverterSet { .. }) => false,
                }
            }
        }
    };
    const _: () = {
        impl<T: Config> core::fmt::Debug for Event<T> {
            fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
                match *self {
                    Self::GatewaySet { ref address } => fmt
                        .debug_struct("Event::GatewaySet")
                        .field("address", &address)
                        .finish(),
                    Self::ConverterSet {
                        ref id_hash,
                        ref converter,
                    } => fmt
                        .debug_struct("Event::ConverterSet")
                        .field("id_hash", &id_hash)
                        .field("converter", &converter)
                        .finish(),
                    Self::__Ignore(ref _0, ref _1) => fmt
                        .debug_tuple("Event::__Ignore")
                        .field(&_0)
                        .field(&_1)
                        .finish(),
                }
            }
        }
    };
    #[allow(deprecated)]
    const _: () = {
        #[automatically_derived]
        impl<T: Config> ::codec::Encode for Event<T> {
            fn size_hint(&self) -> usize {
                1_usize
                    + match *self {
                        Event::GatewaySet { ref address } => {
                            0_usize.saturating_add(::codec::Encode::size_hint(address))
                        }
                        Event::ConverterSet {
                            ref id_hash,
                            ref converter,
                        } => 0_usize
                            .saturating_add(::codec::Encode::size_hint(id_hash))
                            .saturating_add(::codec::Encode::size_hint(converter)),
                        _ => 0_usize,
                    }
            }
            fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
                &self,
                __codec_dest_edqy: &mut __CodecOutputEdqy,
            ) {
                match *self {
                    Event::GatewaySet { ref address } => {
                        __codec_dest_edqy.push_byte(0usize as ::core::primitive::u8);
                        ::codec::Encode::encode_to(address, __codec_dest_edqy);
                    }
                    Event::ConverterSet {
                        ref id_hash,
                        ref converter,
                    } => {
                        __codec_dest_edqy.push_byte(1usize as ::core::primitive::u8);
                        ::codec::Encode::encode_to(id_hash, __codec_dest_edqy);
                        ::codec::Encode::encode_to(converter, __codec_dest_edqy);
                    }
                    _ => (),
                }
            }
        }
        #[automatically_derived]
        impl<T: Config> ::codec::EncodeLike for Event<T> {}
    };
    #[allow(deprecated)]
    const _: () = {
        #[automatically_derived]
        impl<T: Config> ::codec::Decode for Event<T> {
            fn decode<__CodecInputEdqy: ::codec::Input>(
                __codec_input_edqy: &mut __CodecInputEdqy,
            ) -> ::core::result::Result<Self, ::codec::Error> {
                match __codec_input_edqy
                    .read_byte()
                    .map_err(|e| e.chain("Could not decode `Event`, failed to read variant byte"))?
                {
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy == 0usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Event::<T>::GatewaySet {
                                address: {
                                    let __codec_res_edqy =
                                        <H160 as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(e.chain(
                                                "Could not decode `Event::GatewaySet::address`",
                                            ))
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy == 1usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Event::<T>::ConverterSet {
                                id_hash: {
                                    let __codec_res_edqy =
                                        <H256 as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(e.chain(
                                                "Could not decode `Event::ConverterSet::id_hash`",
                                            ))
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                converter: {
                                    let __codec_res_edqy =
                                        <SourceConverter as ::codec::Decode>::decode(
                                            __codec_input_edqy,
                                        );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(e.chain(
                                                "Could not decode `Event::ConverterSet::converter`",
                                            ))
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    _ => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Err(<_ as ::core::convert::Into<_>>::into(
                                "Could not decode `Event`, variant doesn't exist",
                            ))
                        })();
                    }
                }
            }
        }
    };
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        impl<T: Config> ::scale_info::TypeInfo for Event<T>
        where
            frame_support::sp_std::marker::PhantomData<(T)>: ::scale_info::TypeInfo + 'static,
            T: Config + 'static,
        {
            type Identity = Self;
            fn type_info() -> ::scale_info::Type {
                :: scale_info :: Type :: builder () . path (:: scale_info :: Path :: new ("Event" , "axelar_gateway_precompile::pallet")) . type_params (< [_] > :: into_vec (# [rustc_box] :: alloc :: boxed :: Box :: new ([:: scale_info :: TypeParameter :: new ("T" , :: core :: option :: Option :: None)]))) . docs_always (& ["\n\t\t\tThe [event](https://docs.substrate.io/main-docs/build/events-errors/) emitted\n\t\t\tby this pallet.\n\t\t\t"]) . variant (:: scale_info :: build :: Variants :: new () . variant ("GatewaySet" , | v | v . index (0usize as :: core :: primitive :: u8) . fields (:: scale_info :: build :: Fields :: named () . field (| f | f . ty :: < H160 > () . name ("address") . type_name ("H160")))) . variant ("ConverterSet" , | v | v . index (1usize as :: core :: primitive :: u8) . fields (:: scale_info :: build :: Fields :: named () . field (| f | f . ty :: < H256 > () . name ("id_hash") . type_name ("H256")) . field (| f | f . ty :: < SourceConverter > () . name ("converter") . type_name ("SourceConverter")))))
            }
        };
    };
    #[scale_info(skip_type_params(T), capture_docs = "always")]
    ///
    ///			Custom [dispatch errors](https://docs.substrate.io/main-docs/build/events-errors/)
    ///			of this pallet.
    ///
    pub enum Error<T> {
        #[doc(hidden)]
        #[codec(skip)]
        __Ignore(
            frame_support::sp_std::marker::PhantomData<(T)>,
            frame_support::Never,
        ),
        /// The given domain is not yet allowlisted, as we have no converter yet
        NoConverterForSource,
        /// A given domain expects a given structure for account bytes and it
        /// was not given here.
        AccountBytesMismatchForDomain,
    }
    #[allow(deprecated)]
    const _: () = {
        #[automatically_derived]
        impl<T> ::codec::Encode for Error<T> {
            fn size_hint(&self) -> usize {
                1_usize
                    + match *self {
                        Error::NoConverterForSource => 0_usize,
                        Error::AccountBytesMismatchForDomain => 0_usize,
                        _ => 0_usize,
                    }
            }
            fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
                &self,
                __codec_dest_edqy: &mut __CodecOutputEdqy,
            ) {
                match *self {
                    Error::NoConverterForSource => {
                        #[allow(clippy::unnecessary_cast)]
                        __codec_dest_edqy.push_byte(0usize as ::core::primitive::u8);
                    }
                    Error::AccountBytesMismatchForDomain => {
                        #[allow(clippy::unnecessary_cast)]
                        __codec_dest_edqy.push_byte(1usize as ::core::primitive::u8);
                    }
                    _ => (),
                }
            }
        }
        #[automatically_derived]
        impl<T> ::codec::EncodeLike for Error<T> {}
    };
    #[allow(deprecated)]
    const _: () = {
        #[automatically_derived]
        impl<T> ::codec::Decode for Error<T> {
            fn decode<__CodecInputEdqy: ::codec::Input>(
                __codec_input_edqy: &mut __CodecInputEdqy,
            ) -> ::core::result::Result<Self, ::codec::Error> {
                match __codec_input_edqy
                    .read_byte()
                    .map_err(|e| e.chain("Could not decode `Error`, failed to read variant byte"))?
                {
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy == 0usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Error::<T>::NoConverterForSource)
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy == 1usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Error::<T>::AccountBytesMismatchForDomain)
                        })();
                    }
                    _ => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Err(<_ as ::core::convert::Into<_>>::into(
                                "Could not decode `Error`, variant doesn't exist",
                            ))
                        })();
                    }
                }
            }
        }
    };
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        impl<T> ::scale_info::TypeInfo for Error<T>
        where
            frame_support::sp_std::marker::PhantomData<(T)>: ::scale_info::TypeInfo + 'static,
            T: 'static,
        {
            type Identity = Self;
            fn type_info() -> ::scale_info::Type {
                :: scale_info :: Type :: builder () . path (:: scale_info :: Path :: new ("Error" , "axelar_gateway_precompile::pallet")) . type_params (< [_] > :: into_vec (# [rustc_box] :: alloc :: boxed :: Box :: new ([:: scale_info :: TypeParameter :: new ("T" , :: core :: option :: Option :: None)]))) . docs_always (& ["\n\t\t\tCustom [dispatch errors](https://docs.substrate.io/main-docs/build/events-errors/)\n\t\t\tof this pallet.\n\t\t\t"]) . variant (:: scale_info :: build :: Variants :: new () . variant ("NoConverterForSource" , | v | v . index (0usize as :: core :: primitive :: u8) . docs_always (& ["The given domain is not yet allowlisted, as we have no converter yet"])) . variant ("AccountBytesMismatchForDomain" , | v | v . index (1usize as :: core :: primitive :: u8) . docs_always (& ["A given domain expects a given structure for account bytes and it" , "was not given here."])))
            }
        };
    };
    const _: () = {
        impl<T> frame_support::traits::PalletError for Error<T> {
            const MAX_ENCODED_SIZE: usize = 1;
        }
    };
    impl<T: Config> Pallet<T> {
        pub fn set_gateway(origin: OriginFor<T>, address: H160) -> DispatchResult {
            frame_support::storage::with_storage_layer(|| {
                <T as Config>::AdminOrigin::ensure_origin(origin)?;
                AxelarGatewayContract::<T>::set(address);
                Self::deposit_event(Event::<T>::GatewaySet { address });
                Ok(())
            })
        }
        pub fn set_converter(
            origin: OriginFor<T>,
            id_hash: H256,
            converter: SourceConverter,
        ) -> DispatchResult {
            frame_support::storage::with_storage_layer(|| {
                <T as Config>::AdminOrigin::ensure_origin(origin)?;
                SourceConversion::<T>::insert(id_hash, converter.clone());
                Self::deposit_event(Event::<T>::ConverterSet { id_hash, converter });
                Ok(())
            })
        }
    }
    impl<T: Config> Pallet<T> {
        #[doc(hidden)]
        pub fn pallet_constants_metadata(
        ) -> frame_support::sp_std::vec::Vec<frame_support::metadata::PalletConstantMetadata>
        {
            ::alloc::vec::Vec::new()
        }
    }
    impl<T: Config> Pallet<T> {
        #[doc(hidden)]
        pub fn error_metadata() -> Option<frame_support::metadata::PalletErrorMetadata> {
            Some(frame_support::metadata::PalletErrorMetadata {
                ty: frame_support::scale_info::meta_type::<Error<T>>(),
            })
        }
    }
    /// Type alias to `Pallet`, to be used by `construct_runtime`.
    ///
    /// Generated by `pallet` attribute macro.
    #[deprecated(note = "use `Pallet` instead")]
    #[allow(dead_code)]
    pub type Module<T> = Pallet<T>;
    impl<T: Config> frame_support::traits::GetStorageVersion for Pallet<T> {
        fn current_storage_version() -> frame_support::traits::StorageVersion {
            frame_support::traits::StorageVersion::default()
        }
        fn on_chain_storage_version() -> frame_support::traits::StorageVersion {
            frame_support::traits::StorageVersion::get::<Self>()
        }
    }
    impl<T: Config> frame_support::traits::OnGenesis for Pallet<T> {
        fn on_genesis() {
            let storage_version = frame_support::traits::StorageVersion::default();
            storage_version.put::<Self>();
        }
    }
    impl<T: Config> frame_support::traits::PalletInfoAccess for Pallet<T> {
        fn index() -> usize {
            <<T as frame_system::Config>::PalletInfo as frame_support::traits::PalletInfo>::index::<
                Self,
            >()
            .expect(
                "Pallet is part of the runtime because pallet `Config` trait is \
						implemented by the runtime",
            )
        }
        fn name() -> &'static str {
            <<T as frame_system::Config>::PalletInfo as frame_support::traits::PalletInfo>::name::<
                Self,
            >()
            .expect(
                "Pallet is part of the runtime because pallet `Config` trait is \
						implemented by the runtime",
            )
        }
        fn module_name() -> &'static str {
            < < T as frame_system :: Config > :: PalletInfo as frame_support :: traits :: PalletInfo > :: module_name :: < Self > () . expect ("Pallet is part of the runtime because pallet `Config` trait is \
						implemented by the runtime")
        }
        fn crate_version() -> frame_support::traits::CrateVersion {
            frame_support::traits::CrateVersion {
                major: 0u16,
                minor: 1u8,
                patch: 0u8,
            }
        }
    }
    impl<T: Config> frame_support::traits::PalletsInfoAccess for Pallet<T> {
        fn count() -> usize {
            1
        }
        fn infos() -> frame_support::sp_std::vec::Vec<frame_support::traits::PalletInfoData> {
            use frame_support::traits::PalletInfoAccess;
            let item = frame_support::traits::PalletInfoData {
                index: Self::index(),
                name: Self::name(),
                module_name: Self::module_name(),
                crate_version: Self::crate_version(),
            };
            <[_]>::into_vec(
                #[rustc_box]
                ::alloc::boxed::Box::new([item]),
            )
        }
    }
    impl<T: Config> frame_support::traits::StorageInfoTrait for Pallet<T> {
        fn storage_info() -> frame_support::sp_std::vec::Vec<frame_support::traits::StorageInfo> {
            #[allow(unused_mut)]
            let mut res = ::alloc::vec::Vec::new();
            {
                let mut storage_info = < AxelarGatewayContract < T > as frame_support :: traits :: StorageInfoTrait > :: storage_info () ;
                res.append(&mut storage_info);
            }
            {
                let mut storage_info =
                    <SourceConversion<T> as frame_support::traits::StorageInfoTrait>::storage_info(
                    );
                res.append(&mut storage_info);
            }
            res
        }
    }
    use frame_support::traits::{StorageInfoTrait, TrackedStorageKey, WhitelistedStorageKeys};
    impl<T: Config> WhitelistedStorageKeys for Pallet<T> {
        fn whitelisted_storage_keys() -> frame_support::sp_std::vec::Vec<TrackedStorageKey> {
            use frame_support::sp_std::vec;
            ::alloc::vec::Vec::new()
        }
    }
    mod warnings {}
    #[doc(hidden)]
    pub mod __substrate_call_check {
        #[doc(hidden)]
        pub use __is_call_part_defined_0 as is_call_part_defined;
    }
    ///Contains one variant per dispatchable that can be called by an extrinsic.
    #[codec(encode_bound())]
    #[codec(decode_bound())]
    #[scale_info(skip_type_params(T), capture_docs = "always")]
    #[allow(non_camel_case_types)]
    pub enum Call<T: Config> {
        #[doc(hidden)]
        #[codec(skip)]
        __Ignore(
            frame_support::sp_std::marker::PhantomData<(T,)>,
            frame_support::Never,
        ),
        #[codec(index = 0u8)]
        set_gateway {
            #[allow(missing_docs)]
            address: H160,
        },
        #[codec(index = 1u8)]
        set_converter {
            #[allow(missing_docs)]
            id_hash: H256,
            #[allow(missing_docs)]
            converter: SourceConverter,
        },
    }
    const _: () = {
        impl<T: Config> core::fmt::Debug for Call<T> {
            fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
                match *self {
                    Self::__Ignore(ref _0, ref _1) => fmt
                        .debug_tuple("Call::__Ignore")
                        .field(&_0)
                        .field(&_1)
                        .finish(),
                    Self::set_gateway { ref address } => fmt
                        .debug_struct("Call::set_gateway")
                        .field("address", &address)
                        .finish(),
                    Self::set_converter {
                        ref id_hash,
                        ref converter,
                    } => fmt
                        .debug_struct("Call::set_converter")
                        .field("id_hash", &id_hash)
                        .field("converter", &converter)
                        .finish(),
                }
            }
        }
    };
    const _: () = {
        impl<T: Config> core::clone::Clone for Call<T> {
            fn clone(&self) -> Self {
                match self {
                    Self::__Ignore(ref _0, ref _1) => {
                        Self::__Ignore(core::clone::Clone::clone(_0), core::clone::Clone::clone(_1))
                    }
                    Self::set_gateway { ref address } => Self::set_gateway {
                        address: core::clone::Clone::clone(address),
                    },
                    Self::set_converter {
                        ref id_hash,
                        ref converter,
                    } => Self::set_converter {
                        id_hash: core::clone::Clone::clone(id_hash),
                        converter: core::clone::Clone::clone(converter),
                    },
                }
            }
        }
    };
    const _: () = {
        impl<T: Config> core::cmp::Eq for Call<T> {}
    };
    const _: () = {
        impl<T: Config> core::cmp::PartialEq for Call<T> {
            fn eq(&self, other: &Self) -> bool {
                match (self, other) {
                    (Self::__Ignore(_0, _1), Self::__Ignore(_0_other, _1_other)) => {
                        true && _0 == _0_other && _1 == _1_other
                    }
                    (Self::set_gateway { address }, Self::set_gateway { address: _0 }) => {
                        true && address == _0
                    }
                    (
                        Self::set_converter { id_hash, converter },
                        Self::set_converter {
                            id_hash: _0,
                            converter: _1,
                        },
                    ) => true && id_hash == _0 && converter == _1,
                    (Self::__Ignore { .. }, Self::set_gateway { .. }) => false,
                    (Self::__Ignore { .. }, Self::set_converter { .. }) => false,
                    (Self::set_gateway { .. }, Self::__Ignore { .. }) => false,
                    (Self::set_gateway { .. }, Self::set_converter { .. }) => false,
                    (Self::set_converter { .. }, Self::__Ignore { .. }) => false,
                    (Self::set_converter { .. }, Self::set_gateway { .. }) => false,
                }
            }
        }
    };
    #[allow(deprecated)]
    const _: () = {
        #[allow(non_camel_case_types)]
        #[automatically_derived]
        impl<T: Config> ::codec::Encode for Call<T> {
            fn size_hint(&self) -> usize {
                1_usize
                    + match *self {
                        Call::set_gateway { ref address } => {
                            0_usize.saturating_add(::codec::Encode::size_hint(address))
                        }
                        Call::set_converter {
                            ref id_hash,
                            ref converter,
                        } => 0_usize
                            .saturating_add(::codec::Encode::size_hint(id_hash))
                            .saturating_add(::codec::Encode::size_hint(converter)),
                        _ => 0_usize,
                    }
            }
            fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
                &self,
                __codec_dest_edqy: &mut __CodecOutputEdqy,
            ) {
                match *self {
                    Call::set_gateway { ref address } => {
                        __codec_dest_edqy.push_byte(0u8 as ::core::primitive::u8);
                        ::codec::Encode::encode_to(address, __codec_dest_edqy);
                    }
                    Call::set_converter {
                        ref id_hash,
                        ref converter,
                    } => {
                        __codec_dest_edqy.push_byte(1u8 as ::core::primitive::u8);
                        ::codec::Encode::encode_to(id_hash, __codec_dest_edqy);
                        ::codec::Encode::encode_to(converter, __codec_dest_edqy);
                    }
                    _ => (),
                }
            }
        }
        #[automatically_derived]
        impl<T: Config> ::codec::EncodeLike for Call<T> {}
    };
    #[allow(deprecated)]
    const _: () = {
        #[allow(non_camel_case_types)]
        #[automatically_derived]
        impl<T: Config> ::codec::Decode for Call<T> {
            fn decode<__CodecInputEdqy: ::codec::Input>(
                __codec_input_edqy: &mut __CodecInputEdqy,
            ) -> ::core::result::Result<Self, ::codec::Error> {
                match __codec_input_edqy
                    .read_byte()
                    .map_err(|e| e.chain("Could not decode `Call`, failed to read variant byte"))?
                {
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy == 0u8 as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Call::<T>::set_gateway {
                                address: {
                                    let __codec_res_edqy =
                                        <H160 as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(e.chain(
                                                "Could not decode `Call::set_gateway::address`",
                                            ))
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy == 1u8 as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Call::<T>::set_converter {
                                id_hash: {
                                    let __codec_res_edqy =
                                        <H256 as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(e.chain(
                                                "Could not decode `Call::set_converter::id_hash`",
                                            ))
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                converter: {
                                    let __codec_res_edqy =
                                        <SourceConverter as ::codec::Decode>::decode(
                                            __codec_input_edqy,
                                        );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(e.chain(
                                                "Could not decode `Call::set_converter::converter`",
                                            ))
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    _ => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Err(<_ as ::core::convert::Into<_>>::into(
                                "Could not decode `Call`, variant doesn't exist",
                            ))
                        })();
                    }
                }
            }
        }
    };
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        impl<T: Config> ::scale_info::TypeInfo for Call<T>
        where
            frame_support::sp_std::marker::PhantomData<(T,)>: ::scale_info::TypeInfo + 'static,
            T: Config + 'static,
        {
            type Identity = Self;
            fn type_info() -> ::scale_info::Type {
                ::scale_info::Type::builder()
                    .path(::scale_info::Path::new(
                        "Call",
                        "axelar_gateway_precompile::pallet",
                    ))
                    .type_params(<[_]>::into_vec(
                        #[rustc_box]
                        ::alloc::boxed::Box::new([::scale_info::TypeParameter::new(
                            "T",
                            ::core::option::Option::None,
                        )]),
                    ))
                    .docs_always(&[
                        "Contains one variant per dispatchable that can be called by an extrinsic.",
                    ])
                    .variant(
                        ::scale_info::build::Variants::new()
                            .variant("set_gateway", |v| {
                                v.index(0u8 as ::core::primitive::u8).fields(
                                    ::scale_info::build::Fields::named().field(|f| {
                                        f.ty::<H160>().name("address").type_name("H160")
                                    }),
                                )
                            })
                            .variant("set_converter", |v| {
                                v.index(1u8 as ::core::primitive::u8).fields(
                                    ::scale_info::build::Fields::named()
                                        .field(|f| f.ty::<H256>().name("id_hash").type_name("H256"))
                                        .field(|f| {
                                            f.ty::<SourceConverter>()
                                                .name("converter")
                                                .type_name("SourceConverter")
                                        }),
                                )
                            }),
                    )
            }
        };
    };
    impl<T: Config> Call<T> {
        ///Create a call with the variant `set_gateway`.
        pub fn new_call_variant_set_gateway(address: H160) -> Self {
            Self::set_gateway { address }
        }
        ///Create a call with the variant `set_converter`.
        pub fn new_call_variant_set_converter(id_hash: H256, converter: SourceConverter) -> Self {
            Self::set_converter { id_hash, converter }
        }
    }
    impl<T: Config> frame_support::dispatch::GetDispatchInfo for Call<T> {
        fn get_dispatch_info(&self) -> frame_support::dispatch::DispatchInfo {
            match *self {
                Self::set_gateway { ref address } => {
                    let __pallet_base_weight = <T as Config>::WeightInfo::set_gateway();
                    let __pallet_weight =
                        <dyn frame_support::dispatch::WeighData<(&H160,)>>::weigh_data(
                            &__pallet_base_weight,
                            (address,),
                        );
                    let __pallet_class = < dyn frame_support :: dispatch :: ClassifyDispatch < (& H160 ,) > > :: classify_dispatch (& __pallet_base_weight , (address ,)) ;
                    let __pallet_pays_fee =
                        <dyn frame_support::dispatch::PaysFee<(&H160,)>>::pays_fee(
                            &__pallet_base_weight,
                            (address,),
                        );
                    frame_support::dispatch::DispatchInfo {
                        weight: __pallet_weight,
                        class: __pallet_class,
                        pays_fee: __pallet_pays_fee,
                    }
                }
                Self::set_converter {
                    ref id_hash,
                    ref converter,
                } => {
                    let __pallet_base_weight = <T as Config>::WeightInfo::set_converter();
                    let __pallet_weight = <dyn frame_support::dispatch::WeighData<(
                        &H256,
                        &SourceConverter,
                    )>>::weigh_data(
                        &__pallet_base_weight, (id_hash, converter)
                    );
                    let __pallet_class = <dyn frame_support::dispatch::ClassifyDispatch<(
                        &H256,
                        &SourceConverter,
                    )>>::classify_dispatch(
                        &__pallet_base_weight, (id_hash, converter)
                    );
                    let __pallet_pays_fee = <dyn frame_support::dispatch::PaysFee<(
                        &H256,
                        &SourceConverter,
                    )>>::pays_fee(
                        &__pallet_base_weight, (id_hash, converter)
                    );
                    frame_support::dispatch::DispatchInfo {
                        weight: __pallet_weight,
                        class: __pallet_class,
                        pays_fee: __pallet_pays_fee,
                    }
                }
                Self::__Ignore(_, _) => ::core::panicking::panic_fmt(format_args!(
                    "internal error: entered unreachable code: {0}",
                    format_args!("__Ignore cannot be used")
                )),
            }
        }
    }
    #[allow(deprecated)]
    impl<T: Config> frame_support::weights::GetDispatchInfo for Call<T> {}
    impl<T: Config> frame_support::dispatch::GetCallName for Call<T> {
        fn get_call_name(&self) -> &'static str {
            match *self {
                Self::set_gateway { .. } => "set_gateway",
                Self::set_converter { .. } => "set_converter",
                Self::__Ignore(_, _) => ::core::panicking::panic_fmt(format_args!(
                    "internal error: entered unreachable code: {0}",
                    format_args!("__PhantomItem cannot be used.")
                )),
            }
        }
        fn get_call_names() -> &'static [&'static str] {
            &["set_gateway", "set_converter"]
        }
    }
    impl<T: Config> frame_support::traits::UnfilteredDispatchable for Call<T> {
        type RuntimeOrigin = frame_system::pallet_prelude::OriginFor<T>;
        fn dispatch_bypass_filter(
            self,
            origin: Self::RuntimeOrigin,
        ) -> frame_support::dispatch::DispatchResultWithPostInfo {
            match self {
                Self::set_gateway { address } => {
                    let __within_span__ = {
                        use ::tracing::__macro_support::Callsite as _;
                        static CALLSITE: ::tracing::callsite::DefaultCallsite = {
                            static META: ::tracing::Metadata<'static> = {
                                :: tracing_core :: metadata :: Metadata :: new ("set_gateway" , "axelar_gateway_precompile::pallet" , :: tracing :: Level :: TRACE , Some ("pallets/liquidity-pools-gateway/axelar-gateway-precompile/src/lib.rs") , Some (89u32) , Some ("axelar_gateway_precompile::pallet") , :: tracing_core :: field :: FieldSet :: new (& [] , :: tracing_core :: callsite :: Identifier (& CALLSITE)) , :: tracing :: metadata :: Kind :: SPAN)
                            };
                            ::tracing::callsite::DefaultCallsite::new(&META)
                        };
                        let mut interest = ::tracing::subscriber::Interest::never();
                        if ::tracing::Level::TRACE <= ::tracing::level_filters::STATIC_MAX_LEVEL
                            && ::tracing::Level::TRACE
                                <= ::tracing::level_filters::LevelFilter::current()
                            && {
                                interest = CALLSITE.interest();
                                !interest.is_never()
                            }
                            && ::tracing::__macro_support::__is_enabled(
                                CALLSITE.metadata(),
                                interest,
                            )
                        {
                            let meta = CALLSITE.metadata();
                            ::tracing::Span::new(meta, &{ meta.fields().value_set(&[]) })
                        } else {
                            let span =
                                ::tracing::__macro_support::__disabled_span(CALLSITE.metadata());
                            {};
                            span
                        }
                    };
                    let __tracing_guard__ = __within_span__.enter();
                    <Pallet<T>>::set_gateway(origin, address)
                        .map(Into::into)
                        .map_err(Into::into)
                }
                Self::set_converter { id_hash, converter } => {
                    let __within_span__ = {
                        use ::tracing::__macro_support::Callsite as _;
                        static CALLSITE: ::tracing::callsite::DefaultCallsite = {
                            static META: ::tracing::Metadata<'static> = {
                                :: tracing_core :: metadata :: Metadata :: new ("set_converter" , "axelar_gateway_precompile::pallet" , :: tracing :: Level :: TRACE , Some ("pallets/liquidity-pools-gateway/axelar-gateway-precompile/src/lib.rs") , Some (89u32) , Some ("axelar_gateway_precompile::pallet") , :: tracing_core :: field :: FieldSet :: new (& [] , :: tracing_core :: callsite :: Identifier (& CALLSITE)) , :: tracing :: metadata :: Kind :: SPAN)
                            };
                            ::tracing::callsite::DefaultCallsite::new(&META)
                        };
                        let mut interest = ::tracing::subscriber::Interest::never();
                        if ::tracing::Level::TRACE <= ::tracing::level_filters::STATIC_MAX_LEVEL
                            && ::tracing::Level::TRACE
                                <= ::tracing::level_filters::LevelFilter::current()
                            && {
                                interest = CALLSITE.interest();
                                !interest.is_never()
                            }
                            && ::tracing::__macro_support::__is_enabled(
                                CALLSITE.metadata(),
                                interest,
                            )
                        {
                            let meta = CALLSITE.metadata();
                            ::tracing::Span::new(meta, &{ meta.fields().value_set(&[]) })
                        } else {
                            let span =
                                ::tracing::__macro_support::__disabled_span(CALLSITE.metadata());
                            {};
                            span
                        }
                    };
                    let __tracing_guard__ = __within_span__.enter();
                    <Pallet<T>>::set_converter(origin, id_hash, converter)
                        .map(Into::into)
                        .map_err(Into::into)
                }
                Self::__Ignore(_, _) => {
                    let _ = origin;
                    ::core::panicking::panic_fmt(format_args!(
                        "internal error: entered unreachable code: {0}",
                        format_args!("__PhantomItem cannot be used.")
                    ));
                }
            }
        }
    }
    impl<T: Config> frame_support::dispatch::Callable<T> for Pallet<T> {
        type RuntimeCall = Call<T>;
    }
    impl<T: Config> Pallet<T> {
        #[doc(hidden)]
        pub fn call_functions() -> frame_support::metadata::PalletCallMetadata {
            frame_support::scale_info::meta_type::<Call<T>>().into()
        }
    }
    impl<T: Config> frame_support::sp_std::fmt::Debug for Error<T> {
        fn fmt(
            &self,
            f: &mut frame_support::sp_std::fmt::Formatter<'_>,
        ) -> frame_support::sp_std::fmt::Result {
            f.write_str(self.as_str())
        }
    }
    impl<T: Config> Error<T> {
        #[doc(hidden)]
        pub fn as_str(&self) -> &'static str {
            match &self {
                Self::__Ignore(_, _) => ::core::panicking::panic_fmt(format_args!(
                    "internal error: entered unreachable code: {0}",
                    format_args!("`__Ignore` can never be constructed")
                )),
                Self::NoConverterForSource => "NoConverterForSource",
                Self::AccountBytesMismatchForDomain => "AccountBytesMismatchForDomain",
            }
        }
    }
    impl<T: Config> From<Error<T>> for &'static str {
        fn from(err: Error<T>) -> &'static str {
            err.as_str()
        }
    }
    impl<T: Config> From<Error<T>> for frame_support::sp_runtime::DispatchError {
        fn from(err: Error<T>) -> Self {
            use frame_support::codec::Encode;
            let index = < < T as frame_system :: Config > :: PalletInfo as frame_support :: traits :: PalletInfo > :: index :: < Pallet < T > > () . expect ("Every active module has an index in the runtime; qed") as u8 ;
            let mut encoded = err.encode();
            encoded.resize(frame_support::MAX_MODULE_ERROR_ENCODED_SIZE, 0);
            frame_support :: sp_runtime :: DispatchError :: Module (frame_support :: sp_runtime :: ModuleError { index , error : TryInto :: try_into (encoded) . expect ("encoded error is resized to be equal to the maximum encoded error size; qed") , message : Some (err . as_str ()) , })
        }
    }
    pub use __tt_error_token_1 as tt_error_token;
    #[doc(hidden)]
    pub mod __substrate_event_check {
        #[doc(hidden)]
        pub use __is_event_part_defined_2 as is_event_part_defined;
    }
    impl<T: Config> Pallet<T> {
        pub(super) fn deposit_event(event: Event<T>) {
            let event = <<T as Config>::RuntimeEvent as From<Event<T>>>::from(event);
            let event = <<T as Config>::RuntimeEvent as Into<
                <T as frame_system::Config>::RuntimeEvent,
            >>::into(event);
            <frame_system::Pallet<T>>::deposit_event(event)
        }
    }
    impl<T: Config> From<Event<T>> for () {
        fn from(_: Event<T>) {}
    }
    impl<T: Config> Pallet<T> {
        #[doc(hidden)]
        pub fn storage_metadata() -> frame_support::metadata::PalletStorageMetadata {
            frame_support :: metadata :: PalletStorageMetadata { prefix : < < T as frame_system :: Config > :: PalletInfo as frame_support :: traits :: PalletInfo > :: name :: < Pallet < T > > () . expect ("No name found for the pallet in the runtime! This usually means that the pallet wasn't added to `construct_runtime!`.") , entries : { # [allow (unused_mut)] let mut entries = :: alloc :: vec :: Vec :: new () ; { < AxelarGatewayContract < T > as frame_support :: storage :: StorageEntryMetadataBuilder > :: build_metadata (:: alloc :: vec :: Vec :: new () , & mut entries) ; } { < SourceConversion < T > as frame_support :: storage :: StorageEntryMetadataBuilder > :: build_metadata (< [_] > :: into_vec (# [rustc_box] :: alloc :: boxed :: Box :: new ([" `SourceConversion` is a `hash_of(Vec<u8>)` where the `Vec<u8>` is the" , " blake256-hash of the source-chain identifier used by the Axelar network."])) , & mut entries) ; } entries } , }
        }
    }
    #[doc(hidden)]
    pub struct _GeneratedPrefixForStorageAxelarGatewayContract<T>(core::marker::PhantomData<(T,)>);
    impl<T: Config> frame_support::traits::StorageInstance
        for _GeneratedPrefixForStorageAxelarGatewayContract<T>
    {
        fn pallet_prefix() -> &'static str {
            < < T as frame_system :: Config > :: PalletInfo as frame_support :: traits :: PalletInfo > :: name :: < Pallet < T > > () . expect ("No name found for the pallet in the runtime! This usually means that the pallet wasn't added to `construct_runtime!`.")
        }
        const STORAGE_PREFIX: &'static str = "AxelarGatewayContract";
    }
    #[doc(hidden)]
    pub struct _GeneratedPrefixForStorageSourceConversion<T>(core::marker::PhantomData<(T,)>);
    impl<T: Config> frame_support::traits::StorageInstance
        for _GeneratedPrefixForStorageSourceConversion<T>
    {
        fn pallet_prefix() -> &'static str {
            < < T as frame_system :: Config > :: PalletInfo as frame_support :: traits :: PalletInfo > :: name :: < Pallet < T > > () . expect ("No name found for the pallet in the runtime! This usually means that the pallet wasn't added to `construct_runtime!`.")
        }
        const STORAGE_PREFIX: &'static str = "SourceConversion";
    }
    #[doc(hidden)]
    pub mod __substrate_inherent_check {
        #[doc(hidden)]
        pub use __is_inherent_part_defined_3 as is_inherent_part_defined;
    }
    /// Hidden instance generated to be internally used when module is used without
    /// instance.
    #[doc(hidden)]
    pub type __InherentHiddenInstance = ();
    pub(super) trait Store {
        type AxelarGatewayContract;
        type SourceConversion;
    }
    impl<T: Config> Store for Pallet<T> {
        type AxelarGatewayContract = AxelarGatewayContract<T>;
        type SourceConversion = SourceConversion<T>;
    }
    impl<T: Config> frame_support::traits::Hooks<<T as frame_system::Config>::BlockNumber>
        for Pallet<T>
    {
    }
    impl<T: Config> frame_support::traits::OnFinalize<<T as frame_system::Config>::BlockNumber>
        for Pallet<T>
    {
        fn on_finalize(n: <T as frame_system::Config>::BlockNumber) {
            let __within_span__ = {
                use ::tracing::__macro_support::Callsite as _;
                static CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        :: tracing_core :: metadata :: Metadata :: new ("on_finalize" , "axelar_gateway_precompile::pallet" , :: tracing :: Level :: TRACE , Some ("pallets/liquidity-pools-gateway/axelar-gateway-precompile/src/lib.rs") , Some (89u32) , Some ("axelar_gateway_precompile::pallet") , :: tracing_core :: field :: FieldSet :: new (& [] , :: tracing_core :: callsite :: Identifier (& CALLSITE)) , :: tracing :: metadata :: Kind :: SPAN)
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let mut interest = ::tracing::subscriber::Interest::never();
                if ::tracing::Level::TRACE <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::TRACE <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        interest = CALLSITE.interest();
                        !interest.is_never()
                    }
                    && ::tracing::__macro_support::__is_enabled(CALLSITE.metadata(), interest)
                {
                    let meta = CALLSITE.metadata();
                    ::tracing::Span::new(meta, &{ meta.fields().value_set(&[]) })
                } else {
                    let span = ::tracing::__macro_support::__disabled_span(CALLSITE.metadata());
                    {};
                    span
                }
            };
            let __tracing_guard__ = __within_span__.enter();
            < Self as frame_support :: traits :: Hooks < < T as frame_system :: Config > :: BlockNumber > > :: on_finalize (n)
        }
    }
    impl<T: Config> frame_support::traits::OnIdle<<T as frame_system::Config>::BlockNumber>
        for Pallet<T>
    {
        fn on_idle(
            n: <T as frame_system::Config>::BlockNumber,
            remaining_weight: frame_support::weights::Weight,
        ) -> frame_support::weights::Weight {
            < Self as frame_support :: traits :: Hooks < < T as frame_system :: Config > :: BlockNumber > > :: on_idle (n , remaining_weight)
        }
    }
    impl<T: Config> frame_support::traits::OnInitialize<<T as frame_system::Config>::BlockNumber>
        for Pallet<T>
    {
        fn on_initialize(
            n: <T as frame_system::Config>::BlockNumber,
        ) -> frame_support::weights::Weight {
            let __within_span__ = {
                use ::tracing::__macro_support::Callsite as _;
                static CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        :: tracing_core :: metadata :: Metadata :: new ("on_initialize" , "axelar_gateway_precompile::pallet" , :: tracing :: Level :: TRACE , Some ("pallets/liquidity-pools-gateway/axelar-gateway-precompile/src/lib.rs") , Some (89u32) , Some ("axelar_gateway_precompile::pallet") , :: tracing_core :: field :: FieldSet :: new (& [] , :: tracing_core :: callsite :: Identifier (& CALLSITE)) , :: tracing :: metadata :: Kind :: SPAN)
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let mut interest = ::tracing::subscriber::Interest::never();
                if ::tracing::Level::TRACE <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::TRACE <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        interest = CALLSITE.interest();
                        !interest.is_never()
                    }
                    && ::tracing::__macro_support::__is_enabled(CALLSITE.metadata(), interest)
                {
                    let meta = CALLSITE.metadata();
                    ::tracing::Span::new(meta, &{ meta.fields().value_set(&[]) })
                } else {
                    let span = ::tracing::__macro_support::__disabled_span(CALLSITE.metadata());
                    {};
                    span
                }
            };
            let __tracing_guard__ = __within_span__.enter();
            < Self as frame_support :: traits :: Hooks < < T as frame_system :: Config > :: BlockNumber > > :: on_initialize (n)
        }
    }
    impl<T: Config> frame_support::traits::OnRuntimeUpgrade for Pallet<T> {
        fn on_runtime_upgrade() -> frame_support::weights::Weight {
            let __within_span__ = {
                use ::tracing::__macro_support::Callsite as _;
                static CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        :: tracing_core :: metadata :: Metadata :: new ("on_runtime_update" , "axelar_gateway_precompile::pallet" , :: tracing :: Level :: TRACE , Some ("pallets/liquidity-pools-gateway/axelar-gateway-precompile/src/lib.rs") , Some (89u32) , Some ("axelar_gateway_precompile::pallet") , :: tracing_core :: field :: FieldSet :: new (& [] , :: tracing_core :: callsite :: Identifier (& CALLSITE)) , :: tracing :: metadata :: Kind :: SPAN)
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let mut interest = ::tracing::subscriber::Interest::never();
                if ::tracing::Level::TRACE <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::TRACE <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        interest = CALLSITE.interest();
                        !interest.is_never()
                    }
                    && ::tracing::__macro_support::__is_enabled(CALLSITE.metadata(), interest)
                {
                    let meta = CALLSITE.metadata();
                    ::tracing::Span::new(meta, &{ meta.fields().value_set(&[]) })
                } else {
                    let span = ::tracing::__macro_support::__disabled_span(CALLSITE.metadata());
                    {};
                    span
                }
            };
            let __tracing_guard__ = __within_span__.enter();
            let pallet_name = < < T as frame_system :: Config > :: PalletInfo as frame_support :: traits :: PalletInfo > :: name :: < Self > () . unwrap_or ("<unknown pallet name>") ;
            {
                let lvl = ::log::Level::Debug;
                if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                    ::log::__private_api::log(
                        format_args!("✅ no migration for {0}", pallet_name),
                        lvl,
                        &(
                            frame_support::LOG_TARGET,
                            "axelar_gateway_precompile::pallet",
                            "pallets/liquidity-pools-gateway/axelar-gateway-precompile/src/lib.rs",
                        ),
                        89u32,
                        ::log::__private_api::Option::None,
                    );
                }
            };
            < Self as frame_support :: traits :: Hooks < < T as frame_system :: Config > :: BlockNumber > > :: on_runtime_upgrade ()
        }
    }
    impl<T: Config> frame_support::traits::OffchainWorker<<T as frame_system::Config>::BlockNumber>
        for Pallet<T>
    {
        fn offchain_worker(n: <T as frame_system::Config>::BlockNumber) {
            < Self as frame_support :: traits :: Hooks < < T as frame_system :: Config > :: BlockNumber > > :: offchain_worker (n)
        }
    }
    impl<T: Config> frame_support::traits::IntegrityTest for Pallet<T> {
        fn integrity_test() {
            < Self as frame_support :: traits :: Hooks < < T as frame_system :: Config > :: BlockNumber > > :: integrity_test ()
        }
    }
    #[cfg(feature = "std")]
    impl<T: Config> frame_support::sp_runtime::BuildModuleGenesisStorage<T, ()> for GenesisConfig<T> {
        fn build_module_genesis_storage(
            &self,
            storage: &mut frame_support::sp_runtime::Storage,
        ) -> std::result::Result<(), std::string::String> {
            frame_support::BasicExternalities::execute_with_storage(storage, || {
                <Self as frame_support::traits::GenesisBuild<T>>::build(self);
                Ok(())
            })
        }
    }
    #[doc(hidden)]
    pub mod __substrate_genesis_config_check {
        #[doc(hidden)]
        pub use __is_genesis_config_defined_4 as is_genesis_config_defined;
        #[doc(hidden)]
        pub use __is_std_macro_defined_for_genesis_4 as is_std_enabled_for_genesis;
    }
    #[doc(hidden)]
    pub mod __substrate_origin_check {
        #[doc(hidden)]
        pub use __is_origin_part_defined_5 as is_origin_part_defined;
    }
    #[doc(hidden)]
    pub mod __substrate_validate_unsigned_check {
        #[doc(hidden)]
        pub use __is_validate_unsigned_part_defined_6 as is_validate_unsigned_part_defined;
    }
    pub use __tt_default_parts_7 as tt_default_parts;
}
impl<T: Config> cfg_traits::TryConvert<(Vec<u8>, Vec<u8>), DomainAddress> for Pallet<T> {
    type Error = DispatchError;
    fn try_convert(origin: (Vec<u8>, Vec<u8>)) -> Result<DomainAddress, DispatchError> {
        let (source_chain, source_address) = origin;
        let domain_converter =
            SourceConversion::<T>::get(H256::from(Blake2_256::hash(&source_chain)))
                .ok_or(Error::<T>::NoConverterForSource)?;
        domain_converter
            .try_convert(&source_address)
            .ok_or(Error::<T>::AccountBytesMismatchForDomain.into())
    }
}
impl<T: Config> Pallet<T>
where
    T: frame_system::Config,
    <T as frame_system::Config>::RuntimeOrigin: From<pallet_liquidity_pools_gateway::GatewayOrigin>,
{
    fn execute(
        handle: &mut impl PrecompileHandle,
        command_id: H256,
        source_chain: String<MAX_SOURCE_CHAIN_BYTES>,
        source_address: String<MAX_SOURCE_ADDRESS_BYTES>,
        payload: Bytes<MAX_PAYLOAD_BYTES>,
    ) -> EvmResult {
        let payload_hash = H256::from(sp_io::hashing::keccak_256(payload.as_bytes()));
        let key = H256::from(sp_io::hashing::keccak_256(&ethabi::encode(&[
            Token::FixedBytes(PREFIX_CONTRACT_CALL_APPROVED.into()),
            Token::FixedBytes(command_id.as_bytes().into()),
            Token::String(source_chain.clone().try_into().map_err(|_| {
                RevertReason::read_out_of_bounds("utf-8 encoding failing".to_string())
            })?),
            Token::String(source_address.clone().try_into().map_err(|_| {
                RevertReason::read_out_of_bounds("utf-8 encoding failing".to_string())
            })?),
            Token::Address(handle.context().address),
            Token::FixedBytes(payload_hash.as_bytes().into()),
        ])));
        let msg = BoundedVec::<
            u8,
            <T as pallet_liquidity_pools_gateway::Config>::MaxIncomingMessageSize,
        >::try_from(payload.as_bytes().to_vec())
        .map_err(|_| PrecompileFailure::Error {
            exit_status: ExitError::Other("payload conversion".into()),
        })?;
        Self::execute_call(key, || {
            let domain_converter =
                SourceConversion::<T>::get(H256::from(Blake2_256::hash(source_chain.as_bytes())))
                    .ok_or(Error::<T>::NoConverterForSource)?;
            let domain_address = domain_converter
                .try_convert(source_address.as_bytes())
                .ok_or(Error::<T>::AccountBytesMismatchForDomain)?;
            pallet_liquidity_pools_gateway::Pallet::<T>::process_msg(
                pallet_liquidity_pools_gateway::GatewayOrigin::Domain(domain_address).into(),
                msg,
            )
        })
    }
    fn execute_with_token(
        _handle: &mut impl PrecompileHandle,
        _command_id: H256,
        _source_chain: String<MAX_SOURCE_CHAIN_BYTES>,
        _source_address: String<MAX_SOURCE_ADDRESS_BYTES>,
        _payload: Bytes<MAX_PAYLOAD_BYTES>,
        _token_symbol: String<MAX_TOKEN_SYMBOL_BYTES>,
        _amount: U256,
    ) -> EvmResult {
        Ok(())
    }
    fn execute_call(key: H256, f: impl FnOnce() -> DispatchResult) -> EvmResult {
        let gateway = AxelarGatewayContract::<T>::get();
        let valid = Self::get_validate_call(gateway, key);
        if valid {
            Self::set_validate_call(gateway, key, false);
            match f().map(|_| ()).map_err(TryDispatchError::Substrate) {
                Err(e) => {
                    Self::set_validate_call(gateway, key, true);
                    Err(e.into())
                }
                Ok(()) => Ok(()),
            }
        } else {
            Err(RevertReason::Custom("Call not validated".to_string()).into())
        }
    }
    fn get_validate_call(from: H160, key: H256) -> bool {
        Self::h256_to_bool(pallet_evm::AccountStorages::<T>::get(
            from,
            Self::get_index_validate_call(key),
        ))
    }
    fn set_validate_call(from: H160, key: H256, valid: bool) {
        pallet_evm::AccountStorages::<T>::set(
            from,
            Self::get_index_validate_call(key),
            Self::bool_to_h256(valid),
        )
    }
    fn get_index_validate_call(key: H256) -> H256 {
        let slot = U256::from(4);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(key.as_bytes());
        let mut be_bytes: [u8; 32] = [0u8; 32];
        slot.to_big_endian(&mut be_bytes);
        bytes.extend_from_slice(&be_bytes);
        H256::from(sp_io::hashing::keccak_256(&bytes))
    }
    fn h256_to_bool(value: H256) -> bool {
        let first = value.0[0];
        first == 1
    }
    fn bool_to_h256(value: bool) -> H256 {
        let mut bytes: [u8; 32] = [0u8; 32];
        if value {
            bytes[0] = 1;
        }
        H256::from(bytes)
    }
}
#[allow(non_camel_case_types)]
pub enum PalletCall<T>
where
    T: frame_system::Config,
    <T as frame_system::Config>::RuntimeOrigin: From<pallet_liquidity_pools_gateway::GatewayOrigin>,
{
    execute {
        command_id: H256,
        source_chain: String<MAX_SOURCE_CHAIN_BYTES>,
        source_address: String<MAX_SOURCE_ADDRESS_BYTES>,
        payload: Bytes<MAX_PAYLOAD_BYTES>,
    },
    execute_with_token {
        _command_id: H256,
        _source_chain: String<MAX_SOURCE_CHAIN_BYTES>,
        _source_address: String<MAX_SOURCE_ADDRESS_BYTES>,
        _payload: Bytes<MAX_PAYLOAD_BYTES>,
        _token_symbol: String<MAX_TOKEN_SYMBOL_BYTES>,
        _amount: U256,
    },
    #[doc(hidden)]
    __phantom(
        ::core::marker::PhantomData<(T)>,
        ::core::convert::Infallible,
    ),
}
impl<T: Config> PalletCall<T>
where
    T: frame_system::Config,
    <T as frame_system::Config>::RuntimeOrigin: From<pallet_liquidity_pools_gateway::GatewayOrigin>,
{
    pub fn parse_call_data(
        handle: &mut impl PrecompileHandle,
    ) -> ::precompile_utils::EvmResult<Self> {
        use ::precompile_utils::solidity::revert::RevertReason;
        let input = handle.input();
        let selector = input.get(0..4).map(|s| {
            let mut buffer = [0u8; 4];
            buffer.copy_from_slice(s);
            u32::from_be_bytes(buffer)
        });
        match selector {
            Some(446214880u32) => Self::_parse_execute_with_token(handle),
            Some(1226180184u32) => Self::_parse_execute(handle),
            Some(_) => Err(RevertReason::UnknownSelector.into()),
            None => Err(RevertReason::read_out_of_bounds("selector").into()),
        }
    }
    fn _parse_execute(handle: &mut impl PrecompileHandle) -> ::precompile_utils::EvmResult<Self> {
        use ::precompile_utils::solidity::revert::InjectBacktrace;
        use ::precompile_utils::solidity::modifier::FunctionModifier;
        use ::precompile_utils::evm::handle::PrecompileHandleExt;
        handle.check_function_modifier(FunctionModifier::NonPayable)?;
        let mut input = handle.read_after_selector()?;
        input.expect_arguments(4usize)?;
        Ok(Self::execute {
            command_id: input.read().in_field("commandId")?,
            source_chain: input.read().in_field("sourceChain")?,
            source_address: input.read().in_field("sourceAddress")?,
            payload: input.read().in_field("payload")?,
        })
    }
    fn _parse_execute_with_token(
        handle: &mut impl PrecompileHandle,
    ) -> ::precompile_utils::EvmResult<Self> {
        use ::precompile_utils::solidity::revert::InjectBacktrace;
        use ::precompile_utils::solidity::modifier::FunctionModifier;
        use ::precompile_utils::evm::handle::PrecompileHandleExt;
        handle.check_function_modifier(FunctionModifier::NonPayable)?;
        let mut input = handle.read_after_selector()?;
        input.expect_arguments(6usize)?;
        Ok(Self::execute_with_token {
            _command_id: input.read().in_field("commandId")?,
            _source_chain: input.read().in_field("sourceChain")?,
            _source_address: input.read().in_field("sourceAddress")?,
            _payload: input.read().in_field("payload")?,
            _token_symbol: input.read().in_field("tokenSymbol")?,
            _amount: input.read().in_field("amount")?,
        })
    }
    pub fn execute(
        self,
        handle: &mut impl PrecompileHandle,
    ) -> ::precompile_utils::EvmResult<::fp_evm::PrecompileOutput> {
        use ::precompile_utils::solidity::codec::Writer;
        use ::fp_evm::{PrecompileOutput, ExitSucceed};
        let output = match self {
            Self::execute {
                command_id,
                source_chain,
                source_address,
                payload,
            } => {
                let output =
                    <Pallet<T>>::execute(handle, command_id, source_chain, source_address, payload);
                ::precompile_utils::solidity::encode_return_value(output?)
            }
            Self::execute_with_token {
                _command_id,
                _source_chain,
                _source_address,
                _payload,
                _token_symbol,
                _amount,
            } => {
                let output = <Pallet<T>>::execute_with_token(
                    handle,
                    _command_id,
                    _source_chain,
                    _source_address,
                    _payload,
                    _token_symbol,
                    _amount,
                );
                ::precompile_utils::solidity::encode_return_value(output?)
            }
            Self::__phantom(_, _) => {
                ::core::panicking::panic_fmt(format_args!("__phantom variant should not be used"))
            }
        };
        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output,
        })
    }
    pub fn supports_selector(selector: u32) -> bool {
        match selector {
            446214880u32 => true,
            1226180184u32 => true,
            _ => false,
        }
    }
    pub fn selectors() -> &'static [u32] {
        &[446214880u32, 1226180184u32]
    }
    pub fn execute_selectors() -> &'static [u32] {
        &[1226180184u32]
    }
    pub fn execute_with_token_selectors() -> &'static [u32] {
        &[446214880u32]
    }
    pub fn encode(self) -> ::sp_std::vec::Vec<u8> {
        use ::precompile_utils::solidity::codec::Writer;
        match self {
            Self::execute {
                command_id,
                source_chain,
                source_address,
                payload,
            } => Writer::new_with_selector(1226180184u32)
                .write(command_id)
                .write(source_chain)
                .write(source_address)
                .write(payload)
                .build(),
            Self::execute_with_token {
                _command_id,
                _source_chain,
                _source_address,
                _payload,
                _token_symbol,
                _amount,
            } => Writer::new_with_selector(446214880u32)
                .write(_command_id)
                .write(_source_chain)
                .write(_source_address)
                .write(_payload)
                .write(_token_symbol)
                .write(_amount)
                .build(),
            Self::__phantom(_, _) => {
                ::core::panicking::panic_fmt(format_args!("__phantom variant should not be used"))
            }
        }
    }
}
impl<T: Config> From<PalletCall<T>> for ::sp_std::vec::Vec<u8>
where
    T: frame_system::Config,
    <T as frame_system::Config>::RuntimeOrigin: From<pallet_liquidity_pools_gateway::GatewayOrigin>,
{
    fn from(a: PalletCall<T>) -> ::sp_std::vec::Vec<u8> {
        a.encode()
    }
}
impl<T: Config> ::fp_evm::Precompile for Pallet<T>
where
    T: frame_system::Config,
    <T as frame_system::Config>::RuntimeOrigin: From<pallet_liquidity_pools_gateway::GatewayOrigin>,
{
    fn execute(
        handle: &mut impl PrecompileHandle,
    ) -> ::precompile_utils::EvmResult<::fp_evm::PrecompileOutput> {
        <PalletCall<T>>::parse_call_data(handle)?.execute(handle)
    }
}
#[allow(non_snake_case)]
pub(crate) fn __Pallet_test_solidity_signatures_inner() {
    use ::precompile_utils::solidity::Codec;
    match (
        &"(bytes32,string,string,bytes)",
        &<(
            H256,
            String<MAX_SOURCE_CHAIN_BYTES>,
            String<MAX_SOURCE_ADDRESS_BYTES>,
            Bytes<MAX_PAYLOAD_BYTES>,
        ) as Codec>::signature(),
    ) {
        (left_val, right_val) => {
            if !(*left_val == *right_val) {
                let kind = ::core::panicking::AssertKind::Eq;
                :: core :: panicking :: assert_failed (kind , & * left_val , & * right_val , :: core :: option :: Option :: Some (format_args ! ("{0} function signature doesn\'t match (left: attribute, right: computed from Rust types)" , "execute"))) ;
            }
        }
    };
    match (
        &"(bytes32,string,string,bytes,string,uint256)",
        &<(
            H256,
            String<MAX_SOURCE_CHAIN_BYTES>,
            String<MAX_SOURCE_ADDRESS_BYTES>,
            Bytes<MAX_PAYLOAD_BYTES>,
            String<MAX_TOKEN_SYMBOL_BYTES>,
            U256,
        ) as Codec>::signature(),
    ) {
        (left_val, right_val) => {
            if !(*left_val == *right_val) {
                let kind = ::core::panicking::AssertKind::Eq;
                :: core :: panicking :: assert_failed (kind , & * left_val , & * right_val , :: core :: option :: Option :: Some (format_args ! ("{0} function signature doesn\'t match (left: attribute, right: computed from Rust types)" , "execute_with_token"))) ;
            }
        }
    };
}
