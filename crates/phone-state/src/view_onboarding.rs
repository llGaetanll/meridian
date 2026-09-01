//! First run: the flow that ends with a bank connected.
//!
//! One value per screen, in the order they are drawn.
//! `sketches/mobile/onboarding.html` is the drawing these are the finished
//! form of. Plaid's own page is not among them — the bank sign-in happens on
//! `secure.plaid.com`, in a web session the app opens and does not draw — so
//! the flow has a screen either side of it and nothing for it.

/// Where the flow is, and everything that screen prints.
///
/// An enum rather than one record of optional fields, for the same reason
/// [`ViewScreen`](crate::ViewScreen) is one: the view draws the arm it is
/// handed and holds no "which step am I on" of its own. The repeated `step`,
/// `title` and `body` are the price of that, and a cheap one — a variant
/// carries exactly what its screen has and no arm has to check whether a field
/// applies to it.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum ViewOnboarding {
    /// One door, not two. An address that exists signs in and one that does
    /// not signs up, so there is nothing to choose here and no way to choose
    /// wrong.
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
        /// `"Continue"`. Under the field rather than on the bottom edge: the
        /// keyboard covers the bottom third the moment this opens.
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
        /// `Some("Continue")` once the code is complete, `None` before that.
        ///
        /// Absent rather than present-and-dim: there is nothing to submit yet,
        /// and a button that cannot be pressed still invites a press. It takes
        /// the place the resend was in, which is why the resend sits below it.
        action: Option<String>,
        resend: ViewResend,
    },

    /// What connecting a bank means, said plainly, and the button that does
    /// it. No skip: every screen in the app is a view of transactions, so an
    /// app that let someone past this would open on three empty tabs.
    Bank {
        step: String,
        title: String,
        body: Vec<ViewRun>,
        rows: Vec<ViewIconRow>,
        action: String,
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

    /// Counts rather than a tick and a slogan: they are the first proof the
    /// thing works, and they set the expectation for the Ledger the next tap
    /// opens.
    Ready {
        title: String,
        body: Vec<ViewRun>,
        tally: Vec<ViewTally>,
        /// `"Add another bank"` — most people have more than one, and the
        /// second connection will never be likelier than it is right now.
        another: String,
        /// `"Open Meridian"`.
        open: String,
    },

    /// The common ending, not a rare one: people close the sheet to go and
    /// find a password. Nothing was created, and saying so plainly is the
    /// whole job.
    Failed {
        step: String,
        title: String,
        body: Vec<ViewRun>,
        rows: Vec<ViewIconRow>,
        action: String,
    },
}

/// A stretch of body copy, and whether it is set strong.
///
/// The sketch's `<b>`: the address on the code screen, Plaid's name on the
/// bank screen. A list of runs rather than one string with markup in it,
/// because the alternative is every platform learning to parse the same
/// markup — and the alternative to *that* is losing the emphasis the sentence
/// was written around.
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

    /// What is in it. Empty means the placeholder shows instead.
    pub value: String,

    pub placeholder: String,

    /// What the server, or the machine, made of the value. `Some` is what
    /// draws the field wrong — never an empty field, which has not done
    /// anything yet.
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
/// Two states rather than a countdown string that is sometimes empty: while it
/// is counting there is a line to read, and once it is not there is a button
/// to press. They are not the same control.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum ViewResend {
    /// `"Resend in 0:42"`, beside the way back to a mistyped address.
    Waiting {
        text: String,
        /// `"Wrong address?"` — a typo in the step before is otherwise a dead
        /// end, because the code never arrives and nothing says why.
        wrong_address: String,
    },

    /// `"Send a new code"`.
    Ready { label: String },
}

/// An icon, a line, a line under it, and something on the right.
///
/// Every list in this flow is this one row: what connecting a bank means, what
/// the first sync has fetched, what is still true after a failure.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewIconRow {
    /// An icon name the view resolves in its own set.
    pub icon: String,

    pub tone: ViewRowTone,

    pub title: String,

    pub sub: Option<String>,

    /// `"8"`, `"48 so far"`, `"waiting"` — what the row has to say about
    /// itself, on the right.
    pub value: Option<String>,
}

/// What a row's icon means, which is the only thing that colours it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ViewRowTone {
    /// A statement, not a step. The bank screen's four facts.
    Plain,
    Done,
    /// Happening now, and the only row that spins.
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
