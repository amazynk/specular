//! Web addresses the app opens in the system browser. Opening one is the browser's fetch,
//! never the app's: nothing here is downloaded, and no tracking parameters are added.

/// The Specular community on Discord (the toolbar's Discord button, the palette and the
/// About box open it). A page on the project website that forwards to the current invite, so
/// the invite can change without a new release.
pub const DISCORD_INVITE: &str = "https://amazynk.com/discord/";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discord_invite_is_a_plain_https_link() {
        assert!(DISCORD_INVITE.starts_with("https://"));
        // No query or fragment: nothing that could track who clicked.
        assert!(!DISCORD_INVITE.contains(['?', '#']));
    }
}
