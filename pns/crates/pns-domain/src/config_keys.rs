//! Config key names quoted in error and doctor messages, held once so a
//! schema rename cannot land without every quoting message moving with it.
//!
//! NAMING A CONFIG KEY INSIDE A MESSAGE IS A COMMITMENT, the same one
//! [`crate::failure::PHONE_TOKEN`] makes: a test in `pns-adapters`, which can
//! see both these constants and the live schema, asserts they agree, so a
//! rename breaks the build rather than the message.

/// The `[plugins.lights]` key holding the hue bridge's address.
pub const LIGHTS_BRIDGE_HOST: &str = "bridge_host";

/// The `[plugins.lights]` key holding the hue API key.
pub const LIGHTS_API_KEY: &str = "api_key";

/// The `[plugins.github]` key holding the classic personal access token.
pub const GITHUB_PERSONAL_ACCESS_TOKEN: &str = "personal_access_token";
