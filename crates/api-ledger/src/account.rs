//! Accounts and the connections they arrive through.
//!
//! An account is not a ledger concept — the ledger is trades, and a trade does
//! not care which login it came through. But the Balances screen is entirely
//! about where money sits, so the accounts live here beside the entries rather
//! than in a second crate that would only ever be used with this one.

use api_common::Price;

/// One bank login. Plaid calls it an Item; the user calls it a connection.
#[derive(Debug, Clone, PartialEq)]
pub struct Connection {
    pub id: String,

    /// The institution's name, e.g. "First Platypus Bank".
    pub name: String,

    /// What its avatar shows when there is no logo: the initials of the name.
    pub initials: String,

    /// Whether this connection is still syncing, and why not if it is not.
    pub status: ConnectionStatus,

    pub accounts: Vec<Account>,
}

impl Connection {
    /// What is held here, ignoring what is owed. See [`Account::is_debt`].
    pub fn held(&self) -> Price {
        self.accounts
            .iter()
            .filter(|a| !a.is_debt())
            .map(|a| a.balance)
            .sum()
    }

    /// What is owed here, as a negative number.
    pub fn owed(&self) -> Price {
        self.accounts
            .iter()
            .filter(|a| a.is_debt())
            .map(|a| -a.balance)
            .sum()
    }

    pub fn has_debts(&self) -> bool {
        self.accounts.iter().any(|a| a.is_debt())
    }
}

/// Whether a connection is working.
///
/// `/transactions/sync` answers `ITEM_LOGIN_REQUIRED` once a bank's
/// credentials go stale, and only Link's update mode clears it. That is a
/// state the user has to act on, so it is modelled rather than logged.
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    /// Syncing normally, as of this instant. The wording of "2m ago" is the
    /// core's job, because only it is handed a clock.
    Ok { synced_at: api_common::Timestamp },

    /// The user has to sign in again. Nothing else will fix it.
    LoginRequired,
}

/// One account inside a connection.
#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    pub id: String,

    /// e.g. "Plaid Checking".
    pub name: String,

    /// What kind of account, already worded: "Checking", "401(k)", "HELOC".
    pub kind: String,

    /// The last four digits, without the dot the view puts in front.
    pub mask: Option<String>,

    /// The current balance, always positive as the source reports it. Whether
    /// that is money held or money owed is [`Account::is_debt`]'s answer, not
    /// the sign's.
    pub balance: Price,

    /// Spendable now, where it differs from the balance.
    pub available: Option<Price>,

    /// A credit line's ceiling.
    pub limit: Option<Price>,

    /// Interest, where the account pays any.
    pub apy: Option<f64>,

    pub group: AccountGroup,
}

impl Account {
    /// Whether this balance is money owed rather than money held.
    ///
    /// Plaid reports a card balance and a mortgage balance both as a positive
    /// `current` on a `credit` or `loan` account, so the account's type is the
    /// only thing that says which way it points. Getting this wrong adds a
    /// mortgage to someone's net worth.
    pub fn is_debt(&self) -> bool {
        matches!(self.group, AccountGroup::Credit | AccountGroup::Loan)
    }
}

/// Plaid's top-level account `type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountGroup {
    Depository,
    Investment,
    Credit,
    Loan,
    Other,
}

impl AccountGroup {
    pub fn from_plaid(kind: &str) -> Self {
        match kind {
            "depository" => AccountGroup::Depository,
            "investment" | "brokerage" => AccountGroup::Investment,
            "credit" => AccountGroup::Credit,
            "loan" => AccountGroup::Loan,
            _ => AccountGroup::Other,
        }
    }
}

/// Plaid's `subtype`, worded for a person.
///
/// Plaid sends `"401k"` and `"home equity"`; a screen should say `"401(k)"`
/// and `"HELOC"`. One place decides that, and it is here rather than in three
/// view layers.
pub fn account_kind_label(subtype: Option<&str>, group: AccountGroup) -> String {
    let subtype = match subtype {
        Some(subtype) => subtype,
        None => {
            return match group {
                AccountGroup::Depository => "Cash".to_string(),
                AccountGroup::Investment => "Investment".to_string(),
                AccountGroup::Credit => "Credit".to_string(),
                AccountGroup::Loan => "Loan".to_string(),
                AccountGroup::Other => "Account".to_string(),
            };
        }
    };

    match subtype {
        "checking" => "Checking".to_string(),
        "savings" => "Savings".to_string(),
        "cd" => "CD".to_string(),
        "money market" => "Money market".to_string(),
        "credit card" => "Credit".to_string(),
        "hsa" => "HSA".to_string(),
        "cash management" => "Cash management".to_string(),
        "401k" => "401(k)".to_string(),
        "ira" => "IRA".to_string(),
        "roth" => "Roth IRA".to_string(),
        "student" => "Student loan".to_string(),
        "mortgage" => "Mortgage".to_string(),
        "auto" => "Auto loan".to_string(),
        "home equity" => "HELOC".to_string(),
        // Unknown subtypes are shown rather than hidden: "Sep Ira" reads
        // better than nothing, and nothing would look like a bug.
        other => title_case(other),
    }
}

/// The initials shown where a connection has no logo. Two letters at most.
pub fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter(|w| !w.is_empty())
        .take(2)
        .filter_map(|w| w.chars().next())
        .flat_map(|c| c.to_uppercase())
        .collect()
}

fn title_case(s: &str) -> String {
    s.split(' ')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(group: AccountGroup, balance: Price) -> Account {
        Account {
            id: "a".into(),
            name: "Account".into(),
            kind: "Checking".into(),
            mask: None,
            balance,
            available: None,
            limit: None,
            apy: None,
            group,
        }
    }

    /// The failure this guards against is a mortgage counted as savings.
    #[test]
    fn debts_are_decided_by_the_account_type_not_the_sign() {
        assert!(account(AccountGroup::Loan, 56_302.06).is_debt());
        assert!(account(AccountGroup::Credit, 410.0).is_debt());
        assert!(!account(AccountGroup::Depository, 110.0).is_debt());
        assert!(!account(AccountGroup::Investment, 23_631.98).is_debt());
    }

    #[test]
    fn a_connection_totals_held_and_owed_separately() {
        let conn = Connection {
            id: "i".into(),
            name: "First Platypus Bank".into(),
            initials: "FP".into(),
            status: ConnectionStatus::Ok { synced_at: 0 },
            accounts: vec![
                account(AccountGroup::Depository, 110.0),
                account(AccountGroup::Investment, 890.0),
                account(AccountGroup::Loan, 500.0),
            ],
        };

        assert_eq!(conn.held(), 1_000.0);
        assert_eq!(conn.owed(), -500.0);
    }

    #[test]
    fn subtypes_are_worded_for_a_person() {
        assert_eq!(
            account_kind_label(Some("401k"), AccountGroup::Investment),
            "401(k)"
        );
        assert_eq!(
            account_kind_label(Some("home equity"), AccountGroup::Loan),
            "HELOC"
        );
        assert_eq!(
            account_kind_label(Some("sep ira"), AccountGroup::Investment),
            "Sep Ira"
        );
        assert_eq!(account_kind_label(None, AccountGroup::Credit), "Credit");
    }

    #[test]
    fn initials_take_two_words() {
        assert_eq!(initials("First Platypus Bank"), "FP");
        assert_eq!(initials("Houndstooth"), "H");
        assert_eq!(initials(""), "");
    }
}
