//! Signing in to encoredecks.com, and checking that a saved session is still valid.
//!
//! Signing in sends the email and password once and gets back a session cookie, valid for a
//! year. Only the session is kept (see [`super::credentials`]); the password is wiped.

use anyhow::{Context, Result, anyhow, bail};
use zeroize::Zeroizing;

use super::{LOGIN_API, LOGOUT_API, SITE, USER_PAGE};
use crate::text::percent_decode;

const SESSION_COOKIE: &str = "encoresesssionid";

/// An Encore Decks account linked to the tool: its name and login session.
///
/// `Zeroizing<String>` is a `String` whose memory is overwritten with zeros when it is
/// dropped, so the session does not linger in memory after use.
#[derive(Clone, PartialEq)]
pub struct Account {
    pub name: String,
    pub session: Zeroizing<String>,
}

/// Printing an account (in logs, in test failures) never shows the session.
impl std::fmt::Debug for Account {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Account")
            .field("name", &self.name)
            .field("session", &"<redacted>")
            .finish()
    }
}

/// The error returned when the linked account's session is no longer valid: the user must
/// link the account again. Callers recognise it with `error.is::<SessionExpired>()`.
#[derive(Debug)]
pub struct SessionExpired;

impl std::fmt::Display for SessionExpired {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the Encore Decks session has expired; link your account again in Settings"
        )
    }
}

impl std::error::Error for SessionExpired {}

/// The `Cookie` header value that carries `session`.
pub(super) fn cookie(session: &str) -> Zeroizing<String> {
    Zeroizing::new(format!("{SESSION_COOKIE}={session}"))
}

/// `encoresesssionid=s%3Aabc.def; Path=/; HttpOnly` -> `s%3Aabc.def`.
pub fn session_from_set_cookie(header: &str) -> Option<String> {
    let (key, value) = header.split(';').next()?.split_once('=')?;
    let value = value.trim();
    if key.trim() == SESSION_COOKIE && !value.is_empty() {
        Some(value.to_string())
    } else {
        None
    }
}

/// `/user/Some%20Name` -> `Some Name`; `/login` (not logged in) -> `None`.
pub fn name_from_user_redirect(location: &str) -> Option<String> {
    let encoded = location
        .trim()
        .trim_start_matches(SITE)
        .strip_prefix("/user/")?
        .split(['/', '?', '#'])
        .next()
        .filter(|name| !name.is_empty())?;
    Some(percent_decode(encoded))
}

/// The account name of a session, or `None` when the session is not logged in.
pub fn account_name(session: &str) -> Result<Option<String>> {
    let response = ureq::get(USER_PAGE)
        .config()
        .max_redirects(0)
        .http_status_as_error(false)
        .build()
        .header("Cookie", cookie(session).as_str())
        .call()
        .context("checking the Encore Decks session")?;
    Ok(response
        .headers()
        .get("location")
        .and_then(|location| location.to_str().ok())
        .and_then(name_from_user_redirect))
}

/// `{"username":…,"password":…}`, written straight into a buffer that is wiped when dropped
/// and sized up front (JSON escaping at most sextuples a character), so it never reallocates
/// and leaves no copy of the password behind.
fn login_body(email: &str, password: &str) -> Result<Zeroizing<Vec<u8>>> {
    let mut body = Zeroizing::new(Vec::with_capacity(32 + 6 * (email.len() + password.len())));
    body.extend_from_slice(b"{\"username\":");
    serde_json::to_writer(&mut *body, email.trim())?;
    body.extend_from_slice(b",\"password\":");
    serde_json::to_writer(&mut *body, password)?;
    body.push(b'}');
    Ok(body)
}

/// Logs in with the account's email and password, over HTTPS only, and returns the session.
/// The password is sent once and not kept.
pub fn login(email: &str, password: &str) -> Result<Account> {
    let body = login_body(email, password)?;
    let response = ureq::post(LOGIN_API)
        .config()
        .http_status_as_error(false)
        // Never let a redirect carry the credentials anywhere else.
        .max_redirects(0)
        .build()
        .header("Content-Type", "application/json")
        .send(&body[..])
        .context("signing in to encoredecks")?;
    match response.status().as_u16() {
        200 => {}
        401 => bail!("wrong email or password"),
        code => bail!("encoredecks answered {code} to the sign-in"),
    }
    let session = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|header| header.to_str().ok())
        .find_map(session_from_set_cookie)
        .map(Zeroizing::new)
        .ok_or_else(|| anyhow!("encoredecks did not return a session"))?;
    let name = account_name(&session)?
        .ok_or_else(|| anyhow!("encoredecks did not keep the session open"))?;
    Ok(Account { name, session })
}

/// Ends the session on encoredecks.com. Best effort: the tool forgets it anyway.
pub fn logout(session: &str) {
    let _ = ureq::post(LOGOUT_API)
        .config()
        .max_redirects(0)
        .build()
        .header("Cookie", cookie(session).as_str())
        .send_empty();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn login_body_is_json() {
        let body = login_body(" me@x.y ", "p\"a\\ss\u{1}é").unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"username": "me@x.y", "password": "p\"a\\ss\u{1}é"})
        );
        assert!(body.capacity() >= body.len());
    }

    #[test]
    fn session_parsing() {
        assert_eq!(
            session_from_set_cookie(
                "encoresesssionid=s%3Aab.c%2Fd; Path=/; Expires=Wed, 29 Sep 2027; HttpOnly"
            )
            .as_deref(),
            Some("s%3Aab.c%2Fd")
        );
        assert_eq!(session_from_set_cookie("other=x; Path=/"), None);
        assert_eq!(session_from_set_cookie("encoresesssionid=; Path=/"), None);

        assert_eq!(
            name_from_user_redirect("/user/Benji").as_deref(),
            Some("Benji")
        );
        assert_eq!(
            name_from_user_redirect("https://www.encoredecks.com/user/A%20B%C3%A9").as_deref(),
            Some("A Bé")
        );
        assert_eq!(name_from_user_redirect("/login"), None);
        assert_eq!(name_from_user_redirect("/user/"), None);

        let account = Account {
            name: "Benji".into(),
            session: Zeroizing::new("s%3Asecret".into()),
        };
        let printed = format!("{account:?}");
        assert!(printed.contains("Benji") && !printed.contains("secret"));
    }

    /// Hits the network (read-only): `cargo test -- --ignored live_session_check`.
    #[test]
    #[ignore]
    fn live_session_check() {
        assert_eq!(account_name("s%3Anot-a-session").unwrap(), None);
        let err = login("nobody@example.invalid", "wrong").unwrap_err();
        assert!(err.to_string().contains("wrong email or password"));
    }
}
