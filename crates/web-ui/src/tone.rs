/// Whether a number reads as a gain, a loss, or neither. Purely presentational:
/// it maps onto the `up` / `down` color roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tone {
    Up,
    Down,
    #[default]
    Neutral,
}

impl Tone {
    pub fn of(value: f64) -> Self {
        if value > 0.0 {
            Self::Up
        } else if value < 0.0 {
            Self::Down
        } else {
            Self::Neutral
        }
    }

    /// Class name for text coloring. Neutral inherits the surrounding color.
    pub fn class(&self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Neutral => "",
        }
    }

    /// Class name for filled elements, where neutral still needs a background.
    pub fn filled_class(&self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Neutral => "neutral",
        }
    }

    pub fn css_var(&self) -> &'static str {
        match self {
            Self::Up => "var(--up)",
            Self::Down => "var(--down)",
            Self::Neutral => "var(--accent)",
        }
    }
}
