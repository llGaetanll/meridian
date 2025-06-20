pub trait YRequest {
    fn url(&self) -> reqwest::Url;
}
