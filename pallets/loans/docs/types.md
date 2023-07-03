```plantuml
@startuml

set namespaceSeparator ::
hide methods

enum Maturity {
    Fixed: Moment
}

enum CalendarEvent {
    End
}

enum ReferenceDate{
    CalendarDate: CalendarEvent,
    OriginationDate
}

ReferenceDate *--> CalendarEvent

enum InterestPayments {
    None
    Monthly: ReferenceDate
    SemiAnnually: ReferenceDate
}

InterestPayments *--> ReferenceDate

enum PayDownSchedule {
    None
}

class RepaymentSchedule {
    maturity: Maturity
    interest_payments: InterestPayments
    pay_down_schedule: PayDownSchedule
}

RepaymentSchedule *--> Maturity
RepaymentSchedule *--> PayDownSchedule
RepaymentSchedule *-----> InterestPayments

enum BorrowRestrictions {
    NoWrittenOff
    FullOnce
}

enum RepayRestrictions {
    None
    FullOnce
}

class LoanRestrictions {
    borrows: BorrowRestrictions
    repayments: RepayRestrictions
}

LoanRestrictions *--> BorrowRestrictions
LoanRestrictions *--> RepayRestrictions

enum CompoundingCadence {
    Secondly: ReferenceDate
}

CompoundingCadence *--> ReferenceDate

enum InterestRate {
    Fixed: Rate, CompoundingCadence
}

InterestRate *--> CompoundingCadence

package portfolio {
    class PortfolioValuation {
        value: Balance
        last_updated: Moment
        values: Vec<Tuple<LoanId, Balance>>
    }
}

package valuation {
    class DiscountedCashFlows {
        probability_of_default: Rate
        loss_given_default: Rate
        discount_rate: Rate
    }

    ValuationMethod *--> DiscountedCashFlows

    enum ValuationMethod {
        DiscountedCashFlows: DiscountedCashFlows
        OutstandingDebt
    }
}

package policy {
    class WriteOffStatus {
        percentage: Rate
        penalty: Rate
    }

    enum WriteOffTrigger {
        PrincipalOverdueDays,
        PriceOutdated,
    }

    class WriteOffRule {
        triggers: Vec<WriteOffTrigger>
        status: WriteOffStatus
    }

    WriteOffRule *--> WriteOffTrigger
    WriteOffRule *--> WriteOffStatus
}

package pricing {
    package internal {
        enum MaxBorrowAmount {
            UpToTotalBorrows::advance_rate: Rate
            UpToOutstandingDebt::advance_rate: Rate
        }

        class InternalPricing {
            collateral_value: Balance
            valuation_method: ValuationMethod
            max_borrow_amount: MaxBorrowAmount
            interest_rate: InterestRate
        }

        InternalPricing *--> MaxBorrowAmount
        InternalPricing *-d--> valuation::ValuationMethod
        InternalPricing *-r-> InterestRate

        class InternalActivePricing {
            info: InternalPricing
            write_off_penalty: Rate,
            principal: Balance,
            normalized_accrued_debt: Balance
        }

        InternalActivePricing *-r-> InternalPricing
    }

    package external {
        class ExternalPricing {
            price_id: Price,
            max_borrow_quantity: Balance,
            notional: Rate,
            interest_rate: InterestRate
        }

        ExternalPricing *-r-> InterestRate

        class ExternalActivePricing {
            info: ExternalPricing
            outstanding_quantity: Balance,
            normalized_notional: Balance
        }
    }

    ExternalActivePricing *-r-> ExternalPricing

    enum Pricing {
        Internal: InternalPricing
        External: ExternalPricing
    }

    enum ActivePricing {
        Internal: InternalActivePricing
        External: ExternalActivePricing
    }

    Pricing *--> InternalPricing
    Pricing *--> ExternalPricing

    ActivePricing *----> InternalActivePricing
    ActivePricing *--> ExternalActivePricing
}

package loan {
    class LoanInfo {
        schedule: RepaymentSchedule
        collateral: Asset
        restrictions: LoanRestrictions
        pricing: Pricing
    }

    class CreatedLoan {
        info: LoanInfo
        borrower: AccountId
    }

    class ActiveLoan {
        loan_id: LoanId
        borrower: AccountId
        schedule: RepaymentSchedule
        collateral: Asset
        restrictions: LoanRestrictions
        pricing: ActivePricing
        write_off_percentage: Rate
        origination_date: Moment
        total_borrowed: Balance
        total_repaid_principal: Balance
        total_repaid_interest: Balance
        total_repaid_unchecked: Balance
    }

    class ClosedLoan {
        closed_at: BlockNumber
        info: LoanInfo
        total_borrowed: Balance
        total_repaid: Balance
    }

    LoanInfo *--> RepaymentSchedule
    LoanInfo *-r-> LoanRestrictions
    LoanInfo *--> pricing::Pricing

    CreatedLoan *--> LoanInfo
    ActiveLoan *--> pricing::ActivePricing
    ActiveLoan *-d--> RepaymentSchedule
    ActiveLoan *-r-> LoanRestrictions
    ClosedLoan *--> LoanInfo
}

class Storage <<(P, orange)>> {
    CreatedLoan: Map<PoolId, LoanId, CreatedLoan>
    ActiveLoans: Map<PoolId, Vec<Tuple<LoanId, ActiveLoan>>>
    ClosedLoan: Map<PoolId, LoanId, ClosedLoan>
    LastLoanId: Map<PoolId, LoanId>
    WriteOffPolicy: Map<PoolId, Vec<WriteOffRule>>
    PortfolioValuation: Map<PoolId, PortfolioValuation>
}

Storage *--> "n" CreatedLoan
Storage *--> "n" ActiveLoan
Storage *--> "n" ClosedLoan
Storage *-u-> "n" WriteOffRule
Storage *-u-> "n" PortfolioValuation

@enduml
```
