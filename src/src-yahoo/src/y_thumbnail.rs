use serde::Deserialize;

use crate::YThumbnailResolution;

#[derive(Debug, Deserialize)]
pub struct YThumbnail {
    pub resolutions: Vec<YThumbnailResolution>,
}
