//! Web addresses the app opens in the system browser. Opening one is the browser's fetch,
//! never the app's: nothing here is downloaded, and no tracking parameters are added.

/// The Specular community on Discord (the toolbar's Discord button, the palette and the
/// About box open it). A page on the project website that forwards to the current invite, so
/// the invite can change without a new release.
pub const DISCORD_INVITE: &str = "https://amazynk.com/discord/";

/// The page of Pagebara, the PDF viewer and editor by the same people ("Get Pagebara" in
/// About, the palette and the macOS Help menu open it). Its downloads are on that page: the
/// app does not download another app.
pub const PAGEBARA: &str = "https://amazynk.com/pagebara/";

/// The page of Dozed, which freezes the Linux desktop apps that are not being looked at
/// ("Get Dozed", in the same places).
pub const DOZED: &str = "https://amazynk.com/dozed/";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discord_invite_is_a_plain_https_link() {
        assert!(DISCORD_INVITE.starts_with("https://"));
        // No query or fragment: nothing that could track who clicked.
        assert!(!DISCORD_INVITE.contains(['?', '#']));
    }

    /// The other apps' links are their pages on the same website, and as plain.
    #[test]
    fn the_other_apps_pages_are_plain_https_links() {
        for (url, app) in [(PAGEBARA, "pagebara"), (DOZED, "dozed")] {
            assert_eq!(url, format!("https://amazynk.com/{app}/"));
            assert!(!url.contains(['?', '#']));
        }
    }
}
