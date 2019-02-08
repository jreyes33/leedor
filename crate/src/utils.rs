use crate::error::Result;
use cfg_if::cfg_if;
use url::Url;

cfg_if! {
    if #[cfg(feature = "console_error_panic_hook")] {
        pub use console_error_panic_hook::set_once as set_panic_hook;
    } else {
        #[inline]
        pub fn set_panic_hook() {}
    }
}

pub fn parse_relative_url(href: &str) -> Result<Url> {
    let base_url = Url::parse("https://leedor.jreyes.org")?;
    let url = Url::options().base_url(Some(&base_url)).parse(&href)?;
    Ok(url)
}
