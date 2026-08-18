pub trait GcRequest {
    fn url(&self) -> reqwest::Url;
}
