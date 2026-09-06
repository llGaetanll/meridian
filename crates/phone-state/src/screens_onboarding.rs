//! First run, turned into screens.
//!
//! Every sentence the flow prints is here, so that three platforms drawing
//! this road are not three chances to word the same promise differently.
//! `sketches/mobile/onboarding.html` carries the reasoning behind each one.

use std::cmp::Ordering;

use api_common::Timestamp;

use crate::StateOnboarding;
use crate::ViewCodeCell;
use crate::ViewField;
use crate::ViewIconRow;
use crate::ViewOnboarding;
use crate::ViewResend;
use crate::ViewRowTone;
use crate::ViewRun;
use crate::ViewTally;
use crate::fmt;
use crate::state_onboarding::CODE_LENGTH;
use crate::state_onboarding::Fault;
use crate::state_onboarding::RESEND_AFTER;
use crate::state_onboarding::Step;
use server_wire::SyncStage;

const STEP_ACCOUNT: &str = "Step 1 of 3 · Account";
const STEP_BANK: &str = "Step 2 of 3 · Bank";
const STEP_SYNC: &str = "Step 3 of 3 · Sync";

/// How many stages the progress bar is a fraction of: the bank, then the three
/// of [`SyncStage`].
const SYNC_STAGES: f64 = 4.0;

pub(crate) fn onboarding(flow: &StateOnboarding, now: Timestamp) -> ViewOnboarding {
    match flow.step() {
        Step::Welcome => welcome(),
        Step::Email => email(flow),
        Step::Code => code(flow, now),
        Step::Bank => bank(flow),
        Step::Sync => sync(flow),
        Step::Ready => ready(flow),
        Step::Failed => failed(flow),
    }
}

fn welcome() -> ViewOnboarding {
    ViewOnboarding::Welcome {
        title: "Every account,\none ledger.".to_string(),
        body: plain("Meridian reads your banks and puts what you spend, hold and owe on one page."),
        action: "Continue with email".to_string(),

        // Not a link yet: there is no page behind either name, and a tap that
        // opens nothing is worse than a sentence that does not offer to.
        legal: "By continuing you agree to our Terms and Privacy Policy.".to_string(),
    }
}

fn email(flow: &StateOnboarding) -> ViewOnboarding {
    ViewOnboarding::Email {
        step: STEP_ACCOUNT.to_string(),
        title: "What's your email?".to_string(),

        // There is no password anywhere in this flow, and this is where
        // someone would otherwise wonder about it.
        body: plain("We'll send a six-digit code. Nothing to make up and nothing to forget."),

        field: ViewField {
            icon: "mail".to_string(),
            value: flow.email().to_string(),
            placeholder: "you@example.com".to_string(),
            fault: match flow.fault() {
                Some(Fault::Address) => {
                    Some("That doesn't look like an email address.".to_string())
                }
                _ => None,
            },
        },

        action: "Continue".to_string(),
    }
}

fn code(flow: &StateOnboarding, now: Timestamp) -> ViewOnboarding {
    let digits: Vec<char> = flow.code().chars().collect();

    // Every box goes wrong at once: a rejected code is one wrong value, not
    // six wrong digits.
    let bad = flow.fault().is_some();

    let cells = (0..CODE_LENGTH)
        .map(|i| ViewCodeCell {
            digit: digits.get(i).map(char::to_string).unwrap_or_default(),
            live: !bad && i == digits.len(),
            bad,
        })
        .collect();

    ViewOnboarding::Code {
        step: STEP_ACCOUNT.to_string(),
        title: "Check your email".to_string(),

        // The address is on screen and one tap from being fixed. A typo in the
        // step before is otherwise a dead end.
        body: vec![run("We sent a code to "), strong(flow.email()), run(".")],

        cells,

        fault: match flow.fault() {
            Some(Fault::CodeWrong) => Some("That code is not right.".to_string()),
            _ => None,
        },

        action: (flow.code().len() == CODE_LENGTH).then(|| "Continue".to_string()),

        resend: resend(flow, now),
    }
}

/// The way out of a code that did not work, whatever was wrong with it. A
/// countdown that gives way to a button, rather than a button that does
/// nothing for the first minute.
fn resend(flow: &StateOnboarding, now: Timestamp) -> ViewResend {
    let Some(sent) = flow.sent_at() else {
        return ViewResend::Ready {
            label: "Send a new code".to_string(),
        };
    };

    let waited = now.since(sent);

    if waited >= RESEND_AFTER {
        return ViewResend::Ready {
            label: "Send a new code".to_string(),
        };
    }

    ViewResend::Waiting {
        text: format!(
            "Resend in {}",
            fmt::countdown(RESEND_AFTER.saturating_sub(waited)),
        ),
        wrong_address: "Wrong address?".to_string(),
    }
}

/// The screen the whole flow exists for. The four lines are true ones,
/// including the third: Meridian stores your transactions, and an app that
/// implies otherwise has lied on the screen where it asks to be trusted.
fn bank(flow: &StateOnboarding) -> ViewOnboarding {
    ViewOnboarding::Bank {
        step: STEP_BANK.to_string(),
        title: "Just so you know".to_string(),
        body: vec![
            run("Meridian uses "),
            strong("Plaid"),
            run(" to connect. You sign in on your bank's own page."),
        ],

        rows: vec![
            fact(
                "lock",
                "Your bank password goes to Plaid",
                "Never to Meridian. We are handed a key to read with, not your sign-in.",
            ),
            fact(
                "tick-circle",
                "Read-only",
                "Balances, transactions and recurring bills. Nothing can move money.",
            ),
            fact(
                "ledger",
                "Meridian keeps your transactions",
                "They are stored on our server. That is what the ledger is made of.",
            ),
            fact(
                "trash",
                "Removing a bank deletes them",
                "One tap in Account, and its history goes with it.",
            ),
        ],

        action: "Connect a bank".to_string(),
        opening: flow.opening(),

        // Named here rather than in a settings screen: this is where the data
        // starts moving.
        legal: "Plaid handles the connection under its end-user privacy policy.".to_string(),
    }
}

/// The only slow screen in the app, and it happens once per bank.
/// `/transactions/sync` pages the whole history the first time
/// (`docs/PLAID.md`), so a bar beats a spinner that says nothing for a minute.
fn sync(flow: &StateOnboarding) -> ViewOnboarding {
    let sync = flow.sync();

    ViewOnboarding::Sync {
        step: STEP_SYNC.to_string(),
        title: "Reading your history".to_string(),
        body: plain("First time only. Every refresh after this one takes a moment."),

        // The stage that is running counts half: a bar that only moved between
        // stages would sit still through the longest one.
        progress: (f64::from(stage_index(sync.stage)) + 1.5) / SYNC_STAGES,

        rows: vec![
            ViewIconRow {
                icon: "tick-circle".to_string(),
                tone: ViewRowTone::Done,
                title: flow.bank().unwrap_or("Your bank").to_string(),
                sub: None,
                value: Some("connected".to_string()),
            },
            stage_row("Accounts", SyncStage::Accounts, sync.stage, sync.accounts),
            stage_row(
                "Transactions",
                SyncStage::Transactions,
                sync.stage,
                sync.transactions,
            ),
            stage_row(
                "Recurring bills",
                SyncStage::Streams,
                sync.stage,
                sync.streams,
            ),
        ],
    }
}

/// Counts rather than a tick and a slogan: they are the first proof the thing
/// works, and they set the expectation for the Ledger the next tap opens.
///
/// No balance. Balances are covered by default so the app can be opened in
/// public, and putting the figure here would undo that on day one.
fn ready(flow: &StateOnboarding) -> ViewOnboarding {
    let sync = flow.sync();

    let bank = flow.bank().unwrap_or("Your bank");

    ViewOnboarding::Ready {
        title: "You're set.".to_string(),
        body: vec![strong(bank), run(" is connected and syncing.")],

        // The labels say what was counted and no more: anything about the
        // ledger's reach is a fact this screen has not fetched.
        tally: vec![
            tally(sync.accounts, "account", "accounts"),
            tally(sync.transactions, "transaction", "transactions"),
            tally(
                sync.streams,
                "recurring bill found",
                "recurring bills found",
            ),
        ],

        // Add another bank rather than Done: the second connection will never
        // be likelier than it is now.
        another: "Add another bank".to_string(),
        open: "Open Meridian".to_string(),
    }
}

/// The common ending, not a rare one: people close the sheet to go and find a
/// password, or their bank's own login fails.
///
/// It reads two ways because it is reached from two places: before the first
/// bank, Try again is the only honest button on it; after one, it is the end
/// of Add another bank and needs a way through to the app.
fn failed(flow: &StateOnboarding) -> ViewOnboarding {
    let linked = flow.linked();

    let bank = flow.bank().unwrap_or("Your bank");

    ViewOnboarding::Failed {
        step: STEP_BANK.to_string(),
        title: "That didn't finish".to_string(),

        body: plain(if linked {
            "The bank sign-in closed before it was done, so nothing was added."
        } else {
            "The bank sign-in closed before it was done, so no bank was connected."
        }),

        // There is no skip, so this row carries the reassurance instead — the
        // difference between insisting on a bank and trapping someone.
        rows: vec![if linked {
            fact(
                "tick-circle",
                &format!("{bank} is still connected"),
                "Nothing about it changed. This attempt is the only thing that did not finish.",
            )
        } else {
            fact(
                "tick-circle",
                "Your account is made",
                "Close the app if you need to. Signing in again comes back to here.",
            )
        }],

        action: "Try again".to_string(),
        opening: flow.opening(),

        open: linked.then(|| "Open Meridian".to_string()),
    }
}

/// One thing that is true about connecting a bank.
fn fact(icon: &str, title: &str, sub: &str) -> ViewIconRow {
    ViewIconRow {
        icon: icon.to_string(),
        tone: ViewRowTone::Plain,
        title: title.to_string(),
        sub: Some(sub.to_string()),
        value: None,
    }
}

/// One stage of the first sync, as it reads at this moment.
fn stage_row(title: &str, stage: SyncStage, running: SyncStage, count: u32) -> ViewIconRow {
    let (icon, tone, value) = match stage_index(stage).cmp(&stage_index(running)) {
        Ordering::Less => ("tick-circle", ViewRowTone::Done, count.to_string()),

        Ordering::Equal => (
            "spin",
            ViewRowTone::Running,
            // Nothing has landed yet is a different thing from none exist, and
            // "0 so far" reads as the second.
            if count == 0 {
                "reading".to_string()
            } else {
                format!("{count} so far")
            },
        ),

        Ordering::Greater => ("dot", ViewRowTone::Waiting, "waiting".to_string()),
    };

    ViewIconRow {
        icon: icon.to_string(),
        tone,
        title: title.to_string(),
        sub: None,
        value: Some(value),
    }
}

/// How far through the three stages, as a number the bar and rows order by.
fn stage_index(stage: SyncStage) -> u32 {
    match stage {
        SyncStage::Accounts => 0,
        SyncStage::Transactions => 1,
        SyncStage::Streams => 2,
    }
}

fn tally(count: u32, singular: &str, plural: &str) -> ViewTally {
    ViewTally {
        value: count.to_string(),
        label: if count == 1 { singular } else { plural }.to_string(),
    }
}

/// A body with no emphasis in it, which is most of them.
fn plain(text: &str) -> Vec<ViewRun> {
    vec![run(text)]
}

fn run(text: &str) -> ViewRun {
    ViewRun {
        text: text.to_string(),
        strong: false,
    }
}

fn strong(text: &str) -> ViewRun {
    ViewRun {
        text: text.to_string(),
        strong: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_onboarding::Progress;

    /// The sketch draws the bar at 62% with the transactions stage running,
    /// which is what "the running stage counts half" comes out at.
    #[test]
    fn the_bar_matches_the_sketch() {
        let mut flow = StateOnboarding::new();
        flow.bank_linked();
        flow.bank_named("First Platypus Bank");
        flow.sync_progressed(Progress {
            stage: SyncStage::Transactions,
            accounts: 8,
            transactions: 48,
            streams: 0,
        });

        let ViewOnboarding::Sync { progress, rows, .. } = onboarding(&flow, Timestamp::EPOCH)
        else {
            unreachable!("a linked bank puts the flow on the sync screen")
        };

        assert!((progress - 0.625).abs() < f64::EPSILON, "{progress}");

        assert_eq!(rows[1].tone, ViewRowTone::Done);
        assert_eq!(rows[1].value.as_deref(), Some("8"));
        assert_eq!(rows[2].tone, ViewRowTone::Running);
        assert_eq!(rows[2].value.as_deref(), Some("48 so far"));
        assert_eq!(rows[3].tone, ViewRowTone::Waiting);
        assert_eq!(rows[3].value.as_deref(), Some("waiting"));
    }
}
