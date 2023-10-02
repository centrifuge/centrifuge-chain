pub enum RuntimeKind {
	Development,
	Altair,
	Centrifuge,
}

// ---------------------------------

#[fudge::companion]
pub struct CentrifugeEnv {
	#[fudge::relaychain]
	pub relay: RelaychainBuilder<RelayBlock, RelayRtApi, RelayRt, RelayCidp, RelayDp>,
	#[fudge::parachain(PARA_ID)]
	pub parachain: ParachainBuilder<CentrifugeBlock, CentrifugeRtApi, CentrifugeCidp, CentrifugeDp>,
	nonce_manager: Arc<Mutex<NonceManager>>,
	pub events: Arc<Mutex<EventsStorage>>,
}

impl Environment for CentrifugeEnv {
	// TODO...
	// implement non-default methods of Environment trait
}

impl Config for centrifuge_runtime::Runtime {
	const KIND: RuntimeKind = RuntimeKind::Centrifuge;
}

// ---------------------------------

#[fudge::companion]
pub struct AltairEnv {
	#[fudge::relaychain]
	pub relay: RelaychainBuilder<RelayBlock, RelayRtApi, RelayRt, RelayCidp, RelayDp>,
	#[fudge::parachain(PARA_ID)]
	pub parachain: ParachainBuilder<AltairBlock, AltairRtApi, AltairCidp, AltairDp>,
	nonce_manager: Arc<Mutex<NonceManager>>,
	pub events: Arc<Mutex<EventsStorage>>,
}

impl Environment for AltairEnv {
	// TODO...
	// implement non-default methods of Environment trait
}

impl Config for altair_runtime::Runtime {
	const KIND: RuntimeKind = RuntimeKind::Altair;
}

// ---------------------------------

#[fudge::companion]
pub struct DevelopmentEnv {
	#[fudge::relaychain]
	pub relay: RelaychainBuilder<RelayBlock, RelayRtApi, RelayRt, RelayCidp, RelayDp>,
	#[fudge::parachain(PARA_ID)]
	pub parachain:
		ParachainBuilder<DevelopmentBlock, DevelopmentRtApi, DevelopmentCidp, DevelopmentDp>,
	nonce_manager: Arc<Mutex<NonceManager>>,
	pub events: Arc<Mutex<EventsStorage>>,
}

impl Environment for DevelopmentEnv {
	// TODO...
	// implement non-default methods of Environment trait
}

impl Config for development_runtime::Runtime {
	const KIND: RuntimeKind = RuntimeKind::Development;
}


}

