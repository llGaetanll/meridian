pub trait YRequest {
    /// Path and query relative to the API host, e.g.
    /// `v8/finance/chart/SPY?range=1d&interval=1m`.
    ///
    /// It is relative so the client can point it at a proxy: browsers refuse
    /// to call Yahoo directly (no CORS headers, and the User-Agent header they
    /// send cannot be overridden).
    fn path(&self) -> String;
}
