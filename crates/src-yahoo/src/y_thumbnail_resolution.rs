use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct YThumbnailResolution {
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub tag: String,
}
