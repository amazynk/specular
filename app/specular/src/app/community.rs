//! Actions that open a web page in the system browser (the Specular Discord invite, and
//! the pages of the other apps by the same people). The browser fetches the page; the app
//! sends nothing and adds no tracking parameters.

use specular_input::Action;

/// The web address `a` opens in the browser, if it opens one.
pub(super) fn browser_link(a: Action) -> Option<&'static str> {
    match a {
        Action::JoinDiscord => Some(specular_core::links::DISCORD_INVITE),
        Action::GetPagebara => Some(specular_core::links::PAGEBARA),
        Action::GetDozed => Some(specular_core::links::DOZED),
        _ => None,
    }
}

impl super::App {
    /// Opens `a`'s web page in the browser through the app's one way of doing so; false
    /// when `a` opens none.
    pub(super) fn open_browser_link(&mut self, a: Action) -> bool {
        let Some(url) = browser_link(a) else {
            return false;
        };
        super::ai::open_url(url.to_owned());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Join the Specular Discord (what the toolbar's Discord button, the palette entry and
    /// About's button run; specular-widgets tests that they do) opens exactly the invite
    /// constant, unchanged: no query added, nothing else.
    #[test]
    fn discord_button_opens_exactly_the_invite() {
        assert_eq!(
            browser_link(Action::JoinDiscord),
            Some(specular_core::links::DISCORD_INVITE)
        );
        // No other action opens it.
        for &a in Action::LISTED {
            if a != Action::JoinDiscord {
                assert_ne!(
                    browser_link(a),
                    Some(specular_core::links::DISCORD_INVITE),
                    "{a:?}"
                );
            }
        }
    }

    /// "Get Pagebara" and "Get Dozed" (About's buttons, the palette, the macOS Help menu)
    /// open those apps' pages, unchanged, and no other action opens a web page at all.
    #[test]
    fn the_other_apps_open_exactly_their_pages() {
        assert_eq!(
            browser_link(Action::GetPagebara),
            Some(specular_core::links::PAGEBARA)
        );
        assert_eq!(
            browser_link(Action::GetDozed),
            Some(specular_core::links::DOZED)
        );
        for &a in Action::LISTED {
            if !matches!(
                a,
                Action::JoinDiscord | Action::GetPagebara | Action::GetDozed
            ) {
                assert_eq!(browser_link(a), None, "{a:?}");
            }
        }
    }
}
