use web_sys::HtmlDivElement;

/// The three empty wells of a card, in the document and ready to take a frame
/// each.
pub struct CardWells {
    pub number: HtmlDivElement,
    pub expiry: HtmlDivElement,
    pub cvc: HtmlDivElement,
}
