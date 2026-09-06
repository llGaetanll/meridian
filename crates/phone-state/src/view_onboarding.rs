//! First run: the flow that ends with a bank connected.
//!
//! One value per screen, in the order they are drawn. Plaid's own page is not
//! among them — the bank sign-in happens on `secure.plaid.com`, in a web
//! session the app opens and does not draw.

/// Where the flow is, and everything that screen prints.
///
/// An enum rather than one record of optional fields, for the same reason
/// [`ViewScreen`](crate::ViewScreen) is one: the view draws the arm it is
/// handed and holds no "which step am I on" of its own.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum ViewOnboarding {
    /// One door, not two: an address that exists signs in, one that does not
    /// signs up.
    Welcome {
        title: String,
        body: Vec<ViewRun>,
        /// `"Continue with email"`.
        action: String,
        legal: String,
    },

    /// The address, and nothing else on the screen.
    Email {
        step: String,
        title: String,
        body: Vec<ViewRun>,
        field: ViewField,
        /// `"Continue"`.
        action: String,
    },

    /// Six digits and a Continue that is only there once all six are in.
    Code {
        step: String,
        title: String,
        body: Vec<ViewRun>,
        cells: Vec<ViewCodeCell>,
        /// `"That code is not right."` One sentence for every way a code can
        /// fail, because the server answers with one.
        fault: Option<String>,
        /// `Some("Continue")` once the code is complete — absent rather than
        /// present-and-dim, because a dim button still invites a press.
        action: Option<String>,
        resend: ViewResend,
    },

    /// What connecting a bank means, and the button that does it. No skip:
    /// every screen in the app is a view of transactions.
    Bank {
        step: String,
        title: String,
        body: Vec<ViewRun>,
        rows: Vec<ViewIconRow>,
        action: String,
        /// Whether the button is waiting on Plaid's page to appear.
        opening: bool,
        legal: String,
    },

    /// The one slow screen in the app, and it happens once per bank.
    Sync {
        step: String,
        title: String,
        body: Vec<ViewRun>,
        /// How far along, 0..1 — geometry rather than a percentage to print,
        /// which is the exception `docs/FFI.md` carves out for a drawn figure.
        progress: f64,
        rows: Vec<ViewIconRow>,
    },

    /// The end of the flow: counts rather than a tick and a slogan.
    Ready {
        title: String,
        body: Vec<ViewRun>,
        tally: Vec<ViewTally>,
        /// `"Add another bank"`.
        another: String,
        /// `"Open Meridian"`.
        open: String,
    },

    /// The sheet closed without a connection — the common ending, not a rare
    /// one. Nothing was created.
    Failed {
        step: String,
        title: String,
        body: Vec<ViewRun>,
        rows: Vec<ViewIconRow>,
        action: String,
        /// The same wait as [`Bank`](ViewOnboarding::Bank)'s.
        opening: bool,
        /// The way out, worded — `Some` only once a bank has connected.
        open: Option<String>,
    },
}

/// A stretch of body copy, and whether it is set strong.
///
/// A list of runs rather than one string with markup in it, so no platform
/// has to learn to parse the same markup to keep the emphasis.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewRun {
    pub text: String,
    pub strong: bool,
}

/// Something to type into.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewField {
    /// An icon name the view resolves in its own set.
    pub icon: String,

    /// What is in it. Empty draws the placeholder.
    pub value: String,

    pub placeholder: String,

    /// What the server, or the machine, made of the value.
    pub fault: Option<String>,
}

/// One box of a six-digit code.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewCodeCell {
    /// The digit, or empty.
    pub digit: String,

    /// The box that takes the next digit. It carries the caret.
    pub live: bool,

    /// Set on every box at once — a rejected code is one wrong value, not six.
    pub bad: bool,
}

/// Whether a new code can be asked for yet.
///
/// Two states rather than a countdown string that is sometimes empty: a line
/// to read and a button to press are not the same control.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum ViewResend {
    /// `"Resend in 0:42"`.
    Waiting {
        text: String,
        /// `"Wrong address?"` — without it a typo in the step before is a dead
        /// end, because the code never arrives and nothing says why.
        wrong_address: String,
    },

    /// `"Send a new code"`.
    Ready { label: String },
}

/// An icon, a line, a line under it, and something on the right — the one row
/// every list in this flow is made of.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewIconRow {
    /// An icon name the view resolves in its own set.
    pub icon: String,

    pub tone: ViewRowTone,

    pub title: String,

    pub sub: Option<String>,

    /// `"8"`, `"48 so far"`, `"waiting"`.
    pub value: Option<String>,
}

/// What a row's icon means, which is the only thing that colours it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ViewRowTone {
    /// A statement, not a step.
    Plain,
    Done,
    /// Happening now.
    Running,
    /// Not reached yet.
    Waiting,
}

/// One figure on the last screen, and what it is a figure for.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewTally {
    /// `"48"`.
    pub value: String,

    /// `"transactions, back to 23 May"`.
    pub label: String,
}
