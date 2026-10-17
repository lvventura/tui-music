use crate::errors::{AppError, AppResult};

/// Authentication cookies, in the canonical order used for the stored header.
pub const AUTH_COOKIES: &[&str] = &[
    "SAPISID",
    "__Secure-1PAPISID",
    "__Secure-3PAPISID",
    "SID",
    "__Secure-1PSID",
    "__Secure-3PSID",
    "HSID",
    "SSID",
    "APISID",
    "LOGIN_INFO",
    "PREF",
    "VISITOR_INFO1_LIVE",
    "YSC",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookieHeader {
    pairs: Vec<(String, String)>,
}

impl CookieHeader {
    /// Parses a pasted `Cookie:` header. A leading `cookie:` label is ignored.
    pub fn parse(input: &str) -> AppResult<Self> {
        let mut trimmed = input.trim();
        if let Some(rest) = trimmed.strip_prefix("Cookie:") {
            trimmed = rest;
        } else if let Some(rest) = trimmed.strip_prefix("cookie:") {
            trimmed = rest;
        }
        if trimmed.trim().is_empty() {
            return Err(AppError::Provider("la cabecera Cookie está vacía".into()));
        }
        let mut pairs = Vec::new();
        for part in trimmed.split(';') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let Some((name, value)) = part.split_once('=') else {
                return Err(AppError::Provider(
                    "fragmento de cookie sin '=' encontrado".into(),
                ));
            };
            let name = name.trim();
            let value = value.trim();
            if name.is_empty() {
                continue;
            }
            pairs.push((name.to_owned(), value.to_owned()));
        }
        Self::from_pairs(pairs, true)
    }

    /// Parses a Netscape `cookies.txt` export (as produced by browser
    /// extensions or yt-dlp). Comment lines and short rows are skipped.
    pub fn parse_netscape(input: &str) -> AppResult<Self> {
        let mut pairs = Vec::new();
        for line in input.lines() {
            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() < 7 {
                continue;
            }
            let name = fields[5].trim();
            let value = fields[6].trim();
            if name.is_empty() || value.is_empty() {
                continue;
            }
            pairs.push((name.to_owned(), value.to_owned()));
        }
        Self::from_pairs(pairs, true)
    }

    pub fn from_pairs(pairs: Vec<(String, String)>, keep_all: bool) -> AppResult<Self> {
        let mut filtered: Vec<(String, String)> = Vec::new();
        for (name, value) in pairs {
            if name.is_empty() || value.is_empty() {
                continue;
            }
            if !keep_all && !AUTH_COOKIES.contains(&name.as_str()) {
                continue;
            }
            if filtered.iter().any(|(existing, _)| existing == &name) {
                continue;
            }
            filtered.push((name, value));
        }
        let header = Self { pairs: filtered };
        if header.sapisid().is_none() {
            return Err(AppError::Provider(
                "la cabecera Cookie no contiene SAPISID".into(),
            ));
        }
        Ok(header)
    }

    pub fn sapisid(&self) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(name, value)| name == "SAPISID" && !value.is_empty())
            .map(|(_, value)| value.as_str())
    }

    /// Canonical header: known auth cookies first, then the rest, always
    /// terminated with `;` (ytmapi-rs splits SAPISID on the trailing `;`).
    pub fn to_header_string(&self) -> String {
        let mut ordered: Vec<&(String, String)> = Vec::with_capacity(self.pairs.len());
        for known in AUTH_COOKIES {
            if let Some(pair) = self.pairs.iter().find(|(name, _)| name == known) {
                ordered.push(pair);
            }
        }
        for pair in &self.pairs {
            if !AUTH_COOKIES.contains(&pair.0.as_str()) {
                ordered.push(pair);
            }
        }
        let mut output = ordered
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("; ");
        output.push(';');
        output
    }

    /// Netscape cookie file body. Python's `http.cookiejar` asserts that the
    /// `domain_specified` field equals whether the domain starts with a dot,
    /// so the domain must be `.youtube.com` with `TRUE`.
    pub fn to_netscape(&self, expiry: i64) -> String {
        let mut output = String::from("# Netscape HTTP Cookie File\n");
        for (name, value) in &self.pairs {
            output.push_str(&format!(
                ".youtube.com\tTRUE\t/\tTRUE\t{expiry}\t{name}\t{value}\n"
            ));
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::CookieHeader;

    fn value<'a>(header: &'a CookieHeader, name: &str) -> Option<&'a str> {
        header
            .pairs
            .iter()
            .find(|(candidate, _)| candidate == name)
            .map(|(_, value)| value.as_str())
    }

    #[test]
    fn a_header_without_sapisid_is_rejected() {
        let error = CookieHeader::parse("SID=abc; HSID=def;").unwrap_err();
        assert!(error.to_string().contains("SAPISID"));
    }

    #[test]
    fn a_header_requires_a_trailing_semicolon_when_serialized() {
        let header = CookieHeader::parse("SID=abc; SAPISID=secret; HSID=def").unwrap();
        let serialized = header.to_header_string();
        assert!(serialized.ends_with(';'));
        assert_eq!(header.sapisid(), Some("secret"));
    }

    #[test]
    fn known_cookies_are_ordered_first_and_deduplicated() {
        let header =
            CookieHeader::parse("ZZZ=1; SID=abc; SAPISID=secret; SAPISID=duplicate;").unwrap();
        let serialized = header.to_header_string();
        assert_eq!(serialized, "SAPISID=secret; SID=abc; ZZZ=1;");
    }

    #[test]
    fn a_cookie_label_prefix_is_ignored() {
        let header = CookieHeader::parse("Cookie: SAPISID=secret; SID=abc").unwrap();
        assert_eq!(header.sapisid(), Some("secret"));
        assert_eq!(value(&header, "SID"), Some("abc"));
    }

    #[test]
    fn values_containing_equals_are_kept_intact() {
        let header = CookieHeader::parse("SAPISID=sec=ret; LOGIN_INFO=a=b=c").unwrap();
        assert_eq!(header.sapisid(), Some("sec=ret"));
        assert_eq!(value(&header, "LOGIN_INFO"), Some("a=b=c"));
    }

    #[test]
    fn netscape_exports_are_parsed_back() {
        let file = "# Netscape HTTP Cookie File\n\
                    .youtube.com\tTRUE\t/\tTRUE\t0\tSAPISID\tsecret\n\
                    .youtube.com\tTRUE\t/\tTRUE\t0\tSID\tabc\n\
                    # a comment\n\
                    junk line\n";
        let header = CookieHeader::parse_netscape(file).unwrap();
        assert_eq!(header.sapisid(), Some("secret"));
        assert_eq!(value(&header, "SID"), Some("abc"));
    }

    #[test]
    fn netscape_export_contains_every_pair() {
        let header = CookieHeader::parse("SAPISID=secret; SID=abc").unwrap();
        let netscape = header.to_netscape(0);
        assert!(netscape.contains(".youtube.com\tTRUE\t/\tTRUE\t0\tSAPISID\tsecret"));
        assert!(netscape.contains("SID\tabc"));
    }

    #[test]
    fn filtering_drops_non_auth_cookies() {
        let header = CookieHeader::from_pairs(
            vec![
                ("SAPISID".into(), "secret".into()),
                ("SID".into(), "abc".into()),
                ("random".into(), "nope".into()),
            ],
            false,
        )
        .unwrap();
        assert_eq!(value(&header, "SID"), Some("abc"));
        assert_eq!(value(&header, "random"), None);
    }
}
