//! Actions that open a web page in the system browser (the Specular Discord invite). The
//! browser fetches the page; the app sends nothing and adds no tracking parameters.

use specular_input::Action;

/// The web address `a` opens in the browser, if it opens one.
pub(super) fn browser_link(a: Action) -> Option<&'static str> {
    match a {
        Action::JoinDiscord => Some(specular_core::links::DISCORD_INVITE),
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
                assert_eq!(browser_link(a), None, "{a:?}");
            }
        }
    }
}
