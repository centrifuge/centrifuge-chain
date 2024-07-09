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
The following diagrams shows the sequence from the `pallet-foreign-investments` point of view.

### Investments
```plantuml
@startuml
skinparam sequenceArrowThickness 2
skinparam roundcorner 20

actor LiquidityPools as LP
participant ForeignInvestments as FI
participant Investments
participant OrderBook

== increase ==
LP -> FI ++ #Business : increase_foreign_investment()
FI -> FI : increase()
activate FI #Strategy
FI -> OrderBook ++ #Motivation : create_or_increase_swap()
FI <-- OrderBook --
deactivate FI
alt "if same currencies"
    FI -> FI : post_increase_swap()
    activate FI #Strategy
    FI -> Investments ++ #Motivation : increase_invesment()
    FI <-- Investments --
    deactivate FI
end
LP <-- FI --

== cancel ==
LP -> FI ++ #Business : cancel_foreign_investment()
FI -> FI : cancel()
activate FI #Strategy
FI -> Investments ++ #Motivation : cancel_invesment()
FI <-- Investments --
FI -> OrderBook ++ #Motivation : create_swap()
FI <-- OrderBook --
deactivate FI
alt "if any previous pending foreign to pool swap or if same currencies"
    FI -> FI : post_cancel_swap()
    activate FI #Strategy
    LP <- FI ++ #Motivation : fulfill_cancel_investment()
    LP --> FI --
    deactivate FI
end
LP <-- FI --

== fulfill a foreign to pool swap ==
hnote over OrderBook : Order partially fulfilled
FI <- OrderBook ++ #Business : fulfill()
FI -> FI : post_increase_swap()
activate FI #Strategy
FI -> Investments ++ #Motivation : increase_invesment()
FI <-- Investments --
deactivate FI
FI --> OrderBook --

== fulfill a pool to foreign swap ==
hnote over OrderBook : Order partially fulfilled
FI <- OrderBook ++ #Business : fulfill()
FI -> FI : post_cancel_swap()
activate FI #Strategy
note right of LP : Call only when the\nswap is fully fulfilled
LP <- FI ++ #Motivation : fulfill_cancel_investment()
LP --> FI --
deactivate FI
FI --> OrderBook --

== collect ==
hnote over Investments : Epoch close.\nInvestment partially\ncollected
FI <- Investments ++ #Business : collect()
FI -> FI : post_collect()
activate FI #Strategy
LP <- FI ++ #Motivation : fulfill_collect_investment()
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

actor LiquidityPools as LP
participant ForeignInvestments as FI
participant Investments
participant OrderBook

== increase ==
LP -> FI ++ #Business : increase_foreign_redemption()
FI -> FI : increase()
activate FI #Strategy
FI -> Investments ++ #Motivation : increase_redemption()
FI <-- Investments --
deactivate FI
LP <-- FI --

== cancel ==
LP -> FI ++ #Business : cancel_foreign_redemption()
FI -> FI : cancel()
activate FI #Strategy
FI -> Investments ++ #Motivation : cancel_redeemption()
FI <-- Investments --
deactivate FI
LP <-- FI --

== collect ==
hnote over Investments : Epoch close.\nRedemption partially\ncollected
FI <- Investments ++ #Business : collect()
FI -> FI : post_collect_and_swap()
activate FI #Strategy
FI -> OrderBook ++ #Motivation : create_or_increase_swap()
FI <-- OrderBook --
deactivate FI

alt "if same currencies"
    FI -> FI : post_swap()
    activate FI #Strategy
    LP <- FI ++ #Motivation : fulfill_collect_redemption()
    LP --> FI --
    deactivate FI
end

FI --> Investments --

== fulfill a pool to foreign swap ==
hnote over OrderBook : Order partially fulfilled
FI <- OrderBook ++ #Business : fulfill()
FI -> FI : post_swap()
activate FI #Strategy
note right of LP : Call only when the\nswap is fully fulfilled
LP <- FI ++ #Motivation : fulfill_collect_redemption()
LP --> FI --
deactivate FI
FI --> OrderBook --

@enduml
```

### Miscelaneous
- Pallet color used: [here]( https://www.plantuml.com/plantuml/uml/LP312i8m38RlUuf2Ny09tgUUF0Y2WkUmXQsiJKkJ2Njx9rirvlRz-KX26XR8CWLVyUWeGOPWWgEp1-QdwsGmzVwWUXGxP4octgam0urRM6Li1QZtQ8ufUTU2k4XcAQjOMQU97I6pMSiMLieb98y1ITPPzf-LU8tYNj-5nlvOIRTXvkKCNnOMLifTCWZsSr4AA-M1xK3HnqsoFwuQfExpq3S0)
