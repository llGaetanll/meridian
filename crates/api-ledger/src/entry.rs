//! What a transaction carries besides its numbers.
//!
//! [`Tx`](crate::Tx) is the arithmetic: what moved, how much, when. That is
//! the right shape for computing positions and the wrong shape for drawing a
//! row — a row says "McDonald's, Food & drink, pending", none of which is
//! arithmetic.
//!
//! So an [`Entry`] is a `Tx` plus the description that came with it. The
//! ledger stays one list, the way `docs/ABOUT.md` argues it must, and the
//! parts a bank feed knows about ride along instead of forcing a second model.

use api_common::Timestamp;

use crate::Tx;

/// One line of the ledger: the movement, and what is known about it.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Stable across syncs. Plaid's `transaction_id` where there is one, so a
    /// modified transaction replaces its old self rather than joining it.
    pub id: String,

    /// The movement itself.
    pub tx: Tx,

    /// What to call this row. A cleaned-up merchant where the source knew one,
    /// the raw bank description otherwise.
    pub label: String,

    /// Which account it happened in, when the source says.
    pub account_id: Option<String>,

    /// A merchant logo, where one exists. Shown as-is; never a stand-in glyph.
    pub logo_url: Option<String>,

    /// What kind of spending this is.
    pub category: Category,

    /// Authorised but not settled. A pending entry is provisional: the amount
    /// can change and the whole row can be replaced.
    pub pending: bool,

    /// The id of the pending entry this one replaces, if it replaces one.
    ///
    /// Plaid never updates a pending transaction in place — a posted one
    /// arrives with a new id pointing back at the old. Carrying that pointer
    /// is what lets [`Ledger::apply`](crate::Ledger::apply) drop the
    /// duplicate; see `docs/PLAID.md`.
    pub supersedes: Option<String>,
}

impl Entry {
    /// A bare entry around a transaction, for the text ledger format and for
    /// tests. Anything from a bank source knows more than this and builds the
    /// struct directly.
    pub fn from_tx(id: String, tx: Tx) -> Self {
        Self {
            id,
            tx,
            label: String::new(),
            account_id: None,
            logo_url: None,
            category: Category::Unknown,
            pending: false,
            supersedes: None,
        }
    }

    /// When it happened.
    pub fn ts(&self) -> Timestamp {
        match &self.tx {
            Tx::Trade { ts, .. } | Tx::Transfer { ts, .. } => *ts,
        }
    }

    /// What moved, signed in ledger convention: negative left the account.
    ///
    /// A trade's amount is what it cost, so the sold side is the answer. The
    /// bought side is the thing acquired, which is not a number a spending row
    /// wants to show.
    pub fn amount(&self) -> f64 {
        match &self.tx {
            Tx::Trade { sell, .. } => sell.qty,
            Tx::Transfer { pos, .. } => pos.qty,
        }
    }

    /// Money in rather than out.
    pub fn is_inflow(&self) -> bool {
        self.amount() > 0.0
    }
}

/// A spending category.
///
/// These are Plaid's sixteen `personal_finance_category.primary` values. They
/// are modelled as an enum rather than kept as strings because the app groups,
/// filters and colours by them everywhere, and a typo in a string would be a
/// silently empty filter rather than a compile error.
///
/// [`Category::Unknown`] is the escape hatch: a source that reports something
/// we do not know about still produces a usable row. It is deliberately not a
/// catch-all for *our* bugs — every value Plaid documents has an arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    Income,
    TransferIn,
    TransferOut,
    LoanPayments,
    BankFees,
    Entertainment,
    FoodAndDrink,
    GeneralMerchandise,
    HomeImprovement,
    Medical,
    PersonalCare,
    GeneralServices,
    GovernmentAndNonProfit,
    Transportation,
    Travel,
    RentAndUtilities,
    Unknown,
}

impl Category {
    /// Every category the app knows, in the order the filter grid lays them
    /// out. `Unknown` is absent: nothing is filed under it on purpose, so
    /// offering it as a filter would usually mean an empty chip.
    pub const ALL: [Category; 16] = [
        Category::Transportation,
        Category::HomeImprovement,
        Category::FoodAndDrink,
        Category::GovernmentAndNonProfit,
        Category::Travel,
        Category::TransferIn,
        Category::BankFees,
        Category::LoanPayments,
        Category::GeneralServices,
        Category::TransferOut,
        Category::Income,
        Category::Medical,
        Category::GeneralMerchandise,
        Category::RentAndUtilities,
        Category::PersonalCare,
        Category::Entertainment,
    ];

    /// Plaid's `primary` value, e.g. `"FOOD_AND_DRINK"`.
    ///
    /// Anything unrecognised becomes [`Category::Unknown`] rather than an
    /// error: a new Plaid taxonomy value should degrade one row, not fail a
    /// whole sync.
    pub fn from_plaid(primary: &str) -> Self {
        match primary {
            "INCOME" => Category::Income,
            "TRANSFER_IN" => Category::TransferIn,
            "TRANSFER_OUT" => Category::TransferOut,
            "LOAN_PAYMENTS" => Category::LoanPayments,
            "BANK_FEES" => Category::BankFees,
            "ENTERTAINMENT" => Category::Entertainment,
            "FOOD_AND_DRINK" => Category::FoodAndDrink,
            "GENERAL_MERCHANDISE" => Category::GeneralMerchandise,
            "HOME_IMPROVEMENT" => Category::HomeImprovement,
            "MEDICAL" => Category::Medical,
            "PERSONAL_CARE" => Category::PersonalCare,
            "GENERAL_SERVICES" => Category::GeneralServices,
            "GOVERNMENT_AND_NON_PROFIT" => Category::GovernmentAndNonProfit,
            "TRANSPORTATION" => Category::Transportation,
            "TRAVEL" => Category::Travel,
            "RENT_AND_UTILITIES" => Category::RentAndUtilities,
            _ => Category::Unknown,
        }
    }

    /// What the source calls it, and what gets written down.
    ///
    /// The exact inverse of [`Category::from_plaid`], so a category can go into
    /// a database and come back as itself. `Unknown` has no Plaid value of its
    /// own, so it takes its own name — it is a value we assign, not one Plaid
    /// ever sends.
    pub fn as_plaid(&self) -> &'static str {
        match self {
            Category::Income => "INCOME",
            Category::TransferIn => "TRANSFER_IN",
            Category::TransferOut => "TRANSFER_OUT",
            Category::LoanPayments => "LOAN_PAYMENTS",
            Category::BankFees => "BANK_FEES",
            Category::Entertainment => "ENTERTAINMENT",
            Category::FoodAndDrink => "FOOD_AND_DRINK",
            Category::GeneralMerchandise => "GENERAL_MERCHANDISE",
            Category::HomeImprovement => "HOME_IMPROVEMENT",
            Category::Medical => "MEDICAL",
            Category::PersonalCare => "PERSONAL_CARE",
            Category::GeneralServices => "GENERAL_SERVICES",
            Category::GovernmentAndNonProfit => "GOVERNMENT_AND_NON_PROFIT",
            Category::Transportation => "TRANSPORTATION",
            Category::Travel => "TRAVEL",
            Category::RentAndUtilities => "RENT_AND_UTILITIES",
            Category::Unknown => "UNKNOWN",
        }
    }

    /// What a row calls it. Sentence case, because it is read as a label and
    /// not as a constant.
    pub fn label(&self) -> &'static str {
        match self {
            Category::Income => "Income",
            Category::TransferIn => "Transfer in",
            Category::TransferOut => "Transfer out",
            Category::LoanPayments => "Loan payments",
            Category::BankFees => "Bank fees",
            Category::Entertainment => "Entertainment",
            Category::FoodAndDrink => "Food & drink",
            Category::GeneralMerchandise => "General merchandise",
            Category::HomeImprovement => "Home improvement",
            Category::Medical => "Medical",
            Category::PersonalCare => "Personal care",
            Category::GeneralServices => "General services",
            Category::GovernmentAndNonProfit => "Government & non-profit",
            Category::Transportation => "Transportation",
            Category::Travel => "Travel",
            Category::RentAndUtilities => "Rent & utilities",
            Category::Unknown => "Uncategorised",
        }
    }

    /// The CSS class the phone's stylesheet colours this with.
    ///
    /// The mapping lives here rather than in the view because all three front
    /// ends need the same one, and `docs/FFI.md` is explicit that nothing
    /// above the boundary should be working out what a row looks like. Swift
    /// and Kotlin map this token to their own palettes.
    pub fn token(&self) -> &'static str {
        match self {
            Category::FoodAndDrink => "c-food",
            Category::Transportation => "c-transport",
            Category::Travel => "c-travel",
            Category::GeneralMerchandise => "c-merch",
            Category::LoanPayments => "c-loans",
            Category::TransferIn | Category::TransferOut => "c-transfer",
            Category::PersonalCare => "c-personal",
            Category::RentAndUtilities => "c-utilities",
            Category::Income => "c-income",
            Category::Entertainment => "c-entertain",
            Category::GeneralServices => "c-services",
            Category::Medical => "c-medical",
            Category::BankFees => "c-fees",
            Category::HomeImprovement => "c-home",
            Category::GovernmentAndNonProfit => "c-govt",
            Category::Unknown => "c-transfer",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_plaid_primary_maps_to_something_that_is_not_unknown() {
        // The sixteen documented values. If Plaid adds a seventeenth this
        // test keeps passing — `ALL` is what would need updating, and the
        // filter grid is what would show the gap.
        for primary in [
            "INCOME",
            "TRANSFER_IN",
            "TRANSFER_OUT",
            "LOAN_PAYMENTS",
            "BANK_FEES",
            "ENTERTAINMENT",
            "FOOD_AND_DRINK",
            "GENERAL_MERCHANDISE",
            "HOME_IMPROVEMENT",
            "MEDICAL",
            "PERSONAL_CARE",
            "GENERAL_SERVICES",
            "GOVERNMENT_AND_NON_PROFIT",
            "TRANSPORTATION",
            "TRAVEL",
            "RENT_AND_UTILITIES",
        ] {
            assert_ne!(
                Category::from_plaid(primary),
                Category::Unknown,
                "{primary} should have its own arm",
            );
        }
    }

    /// The inverse pair has to round-trip, or a category written down comes
    /// back as something else — and `Unknown` is the arm worth pinning, since
    /// it is the one with no Plaid value behind it.
    #[test]
    fn a_category_survives_being_written_down() {
        for category in Category::ALL {
            assert_eq!(Category::from_plaid(category.as_plaid()), category);
        }

        assert_eq!(
            Category::from_plaid(Category::Unknown.as_plaid()),
            Category::Unknown,
        );
    }

    #[test]
    fn an_unrecognised_category_degrades_rather_than_failing() {
        assert_eq!(Category::from_plaid("CRYPTOCURRENCY"), Category::Unknown);
        assert_eq!(Category::from_plaid(""), Category::Unknown);
    }

    /// `ALL` is what the filter grid renders, so a duplicate would draw twice
    /// and a gap would make a category unfilterable.
    #[test]
    fn all_holds_each_category_once() {
        let mut seen = Category::ALL.to_vec();
        seen.sort();
        seen.dedup();

        assert_eq!(seen.len(), Category::ALL.len());
        assert!(!Category::ALL.contains(&Category::Unknown));
    }
}
