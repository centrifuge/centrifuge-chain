# Foreign Investments (diagrams)

## Architecture
`pallet-foreign-investment` is a pallet without extrinsics that acts as a glue connecting investments and orders to liquidity pools though a bunch of traits:

```plantuml
@startuml
skinparam roundcorner 10

note "NOTE: orange boxes are traits" as N1

skinparam component {
  BackgroundColor<<utility>> #Business
  BackgroundColor #Motivation
}

skinparam rectangle {
  BackgroundColor #Strategy
}

[pallet-foreign-investments] <<utility>> as fi
[pallet-liquidity-pools] as lp
[pallet-investments] as investments
[pallet-order-book] as orders

rectangle Investments
rectangle ForeignInvestments
rectangle ForeignInvestmentsHooks
rectangle TokenSwaps
rectangle "NotificationStatusHook (collected)" as collected_hook
rectangle "NotificationStatusHook (fulfilled)" as fulfilled_hook

lp .down.|> ForeignInvestmentsHooks : implements
lp -down-> ForeignInvestments : uses
fi .up.|> ForeignInvestments : implements
fi -up-> ForeignInvestmentsHooks : uses
fi .down.|> fulfilled_hook : implements
fi .down.|> collected_hook : implements
fi -down-> Investments : uses
fi -down-> TokenSwaps : uses

orders .up.|> TokenSwaps : implements
orders -up-> fulfilled_hook : uses
investments .up.|> Investments : implements
investments -up-> collected_hook : uses

@enduml
```

## Actions
The following diagrams shows the sequence from the `pallet-foreign-investments` point of view and which LP messages are sent/received.

### Investments
```plantuml
@startuml
skinparam sequenceArrowThickness 2
skinparam roundcorner 20
skinparam sequence {
  LifeLineBackgroundColor #Business
}

actor Solidity
participant LiquidityPools as LP
participant ForeignInvestments as FI
participant Investments
participant OrderBook

== increase ==
Solidity -[#Green]> LP : DepositRequest
activate LP
LP -> FI ++ : increase_foreign_investment()
FI -> FI : increase()
activate FI #Strategy
FI -> OrderBook ++ : create_or_increase_swap()
FI <-- OrderBook --
deactivate FI
alt "if same currencies"
    FI -> FI : post_increase_swap()
    activate FI #Strategy
    FI -> Investments ++ : increase_invesment()
    FI <-- Investments --
    deactivate FI
end
LP <-- FI --
deactivate LP

== cancel ==
Solidity -[#Green]> LP : CancelDepositRequest
activate LP
LP -> FI ++ : cancel_foreign_investment()
FI -> FI : cancel()
activate FI #Strategy
FI -> Investments ++ : cancel_invesment()
FI <-- Investments --
FI -> OrderBook ++ : create_swap()
FI <-- OrderBook --
deactivate FI
alt "if any previous pending foreign to pool swap or if same currencies"
    FI -> FI : post_cancel_swap()
    activate FI #Strategy
    LP <- FI ++ #Strategy : fulfill_cancel_investment()
    Solidity <[#Blue]- LP : FulfilledCancelDepositRequest
    LP --> FI --
    deactivate FI
end
deactivate LP
LP <-- FI --

== fulfill a foreign to pool swap ==
hnote over OrderBook : Order partially fulfilled
FI <- OrderBook ++ : fulfill()
FI -> FI : post_increase_swap()
activate FI #Strategy
FI -> Investments ++ : increase_invesment()
FI <-- Investments --
deactivate FI
FI --> OrderBook --

== fulfill a pool to foreign swap ==
hnote over OrderBook : Order partially fulfilled
FI <- OrderBook ++ : fulfill()
FI -> FI : post_cancel_swap()
activate FI #Strategy
note right of LP : Called only when the\nswap is fully fulfilled
LP <- FI ++ #Strategy : fulfill_cancel_investment()
Solidity <[#Blue]- LP : FulfilledCancelDepositRequest
LP --> FI --
deactivate FI
FI --> OrderBook --

== collect ==
hnote over Investments : Epoch close.\nInvestment partially\ncollected
FI <- Investments ++ : collect()
FI -> FI : post_collect()
activate FI #Strategy
LP <- FI ++ #Strategy : fulfill_collect_investment()
Solidity <[#Blue]- LP : FulfilledDepositRequest
LP --> FI --
deactivate FI
FI --> Investments --

@enduml
```

### Redemptions
```plantuml
@startuml
skinparam sequenceArrowThickness 2
skinparam roundcorner 20
skinparam sequence {
  LifeLineBackgroundColor #Business
}

actor Solidity
participant LiquidityPools as LP
participant ForeignInvestments as FI
participant Investments
participant OrderBook

== increase ==
Solidity -[#Green]> LP : RedeemRequest
activate LP
LP -> FI ++ : increase_foreign_redemption()
FI -> FI : increase()
activate FI #Strategy
FI -> Investments ++ : increase_redemption()
FI <-- Investments --
deactivate FI
LP <-- FI --
deactivate LP

== cancel ==
Solidity -[#Green]> LP : CancelRedeemRequest
activate LP
LP -> FI ++ : cancel_foreign_redemption()
FI -> FI : cancel()
activate FI #Strategy
FI -> Investments ++ : cancel_redeemption()
FI <-- Investments --
deactivate FI
LP <-- FI --
Solidity <[#Blue]- LP : FulfilledCancelRedeemRequest
deactivate LP

== collect ==
hnote over Investments : Epoch close.\nRedemption partially\ncollected
FI <- Investments ++ : collect()
FI -> FI : post_collect_and_swap()
activate FI #Strategy
FI -> OrderBook ++ : create_or_increase_swap()
FI <-- OrderBook --
deactivate FI

alt "if same currencies"
    FI -> FI : post_swap()
    activate FI #Strategy
    LP <- FI ++ #Strategy : fulfill_collect_redemption()
    Solidity <[#Blue]- LP : FulfilledRedeemRequest
    LP --> FI --
    deactivate FI
end

FI --> Investments --

== fulfill a pool to foreign swap ==
hnote over OrderBook : Order partially fulfilled
FI <- OrderBook ++ : fulfill()
FI -> FI : post_swap()
activate FI #Strategy
note right of LP : Called only when the\nswap is fully fulfilled
LP <- FI ++ #Strategy : fulfill_collect_redemption()
Solidity <[#Blue]- LP : FulfilledRedeemRequest
LP --> FI --
deactivate FI
FI --> OrderBook --

@enduml
```
