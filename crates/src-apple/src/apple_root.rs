//! The one certificate this server believes without being shown anything.

/// Apple Root CA G3, as the DER bytes Apple publishes.
///
/// Committed rather than fetched, because a trust anchor fetched at runtime is
/// one an attacker gets a say in. It came from
/// `https://www.apple.com/certificateauthority/AppleRootCA-G3.cer` and its
/// SHA-256 is `63343abfb89a6a03ebb57e9b3f5fa7be7c4f5c756f3017b3a8c488c3653e9179`;
/// the certificate is good until 30 April 2039.
pub const APPLE_ROOT_CA_G3: &[u8] = include_bytes!("../certs/AppleRootCA-G3.cer");
