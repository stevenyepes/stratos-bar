//! Currency flag SVGs, vendored under `src-tauri/assets/flags/` and embedded in the
//! binary at compile time.
//!
//! These are deliberately *not* served through [`CachedIconResolver`]: that resolver
//! exists to hand out arbitrary icons from the user's on-disk icon themes, so every
//! read is containment-checked against `icon_roots`. Flags ship inside the binary
//! instead, have no filesystem presence at runtime, and are addressed by a fixed
//! country code rather than a path — so they get their own lookup with no I/O and no
//! containment check to get wrong.
//!
//! `include_bytes!` behind a `match` rather than an embed crate: the set is small,
//! fully enumerated by `countryMap` in `CurrencyResult.vue`, and this keeps the
//! dependency tree unchanged.

/// URL path prefix (after percent-decoding) that routes a `stratos-icon` request to
/// the embedded flags instead of the disk-backed icon resolver.
pub const FLAG_PATH_PREFIX: &str = "flags/";

/// Served for any code the frontend asks for that we do not vendor.
const UNKNOWN: &[u8] = include_bytes!("../../assets/flags/xx.svg");

macro_rules! flags {
    ($($code:literal),* $(,)?) => {
        /// Bytes of the flag for `code`, or the `xx` placeholder when `code` is not
        /// one we vendor.
        ///
        /// Infallible on purpose: an unmapped currency should render the placeholder,
        /// never a broken image, so callers have no not-found branch to handle.
        pub fn lookup(code: &str) -> &'static [u8] {
            match code {
                $($code => include_bytes!(concat!("../../assets/flags/", $code, ".svg")),)*
                _ => UNKNOWN,
            }
        }

        /// Every country code with a vendored flag. Test-only; the app path goes
        /// through [`lookup`].
        #[cfg(test)]
        const VENDORED: &[&str] = &[$($code),*];
    };
}

flags![
    "ar", "au", "br", "btc", "ca", "ch", "cl", "cn", "co", "eu", "gb", "hk", "id", "in", "jp",
    "kr", "mx", "my", "no", "nz", "pe", "ph", "ru", "se", "sg", "th", "tr", "us", "uy", "vn", "xx",
    "za",
];

/// Extracts the country code from a decoded `stratos-icon` request path of the form
/// `flags/<code>.svg`, or [`None`] if the path is not a flag request at all.
///
/// The code is returned verbatim and handed straight to [`lookup`], which falls back
/// to the placeholder — so a traversal attempt like `flags/../../etc/passwd.svg` is
/// simply an unknown code, not a filesystem read.
pub fn flag_code_from_path(decoded_path: &str) -> Option<&str> {
    decoded_path
        .strip_prefix(FLAG_PATH_PREFIX)
        .and_then(|rest| rest.strip_suffix(".svg"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_svg(bytes: &[u8]) -> bool {
        std::str::from_utf8(bytes)
            .map(|s| s.trim_start().starts_with("<svg") && s.contains("</svg>"))
            .unwrap_or(false)
    }

    #[test]
    fn every_vendored_code_resolves_to_its_own_svg() {
        for code in VENDORED {
            let bytes = lookup(code);
            assert!(
                is_svg(bytes),
                "{code}.svg is not a well-formed SVG document"
            );
            if *code != "xx" {
                assert_ne!(bytes, UNKNOWN, "{code} fell through to the placeholder");
            }
        }
    }

    #[test]
    fn currency_map_country_codes_are_all_vendored() {
        // Mirrors `countryMap` in src/components/CurrencyResult.vue.
        for code in [
            "us", "eu", "gb", "jp", "cn", "in", "ca", "au", "ch", "ru", "kr", "br", "mx", "sg",
            "hk", "nz", "za", "tr", "se", "no", "co", "ar", "cl", "pe", "uy", "ph", "id", "th",
            "my", "vn", "btc",
        ] {
            assert!(
                VENDORED.contains(&code),
                "{code} is used by CurrencyResult.vue but not vendored"
            );
        }
    }

    #[test]
    fn unknown_code_falls_back_to_placeholder() {
        assert_eq!(lookup("zz"), UNKNOWN);
        assert_eq!(lookup(""), UNKNOWN);
        assert_eq!(lookup("../../etc/passwd"), UNKNOWN);
    }

    #[test]
    fn flag_paths_are_recognised_and_other_paths_are_not() {
        assert_eq!(flag_code_from_path("flags/us.svg"), Some("us"));
        assert_eq!(flag_code_from_path("flags/xx.svg"), Some("xx"));
        assert_eq!(
            flag_code_from_path("/usr/share/icons/hicolor/48x48/apps/firefox.png"),
            None
        );
        assert_eq!(flag_code_from_path("flags/us.png"), None);
        assert_eq!(flag_code_from_path("notflags/us.svg"), None);
    }

    #[test]
    fn traversal_in_a_flag_path_resolves_to_the_placeholder_not_a_file_read() {
        let code = flag_code_from_path("flags/../../../../etc/passwd.svg").unwrap();
        assert_eq!(lookup(code), UNKNOWN);
    }
}
