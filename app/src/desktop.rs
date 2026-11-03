//! The connectors' view of the desktop: opening a link.

use cargoframe_connectors::Browser;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt as _;

/// Only web links, a local sign-in page and the Claude app's own link ever leave through here,
/// whatever a connector asks for.
pub fn openable(url: &str) -> bool {
    url.starts_with("https://")
        || url.starts_with("http://127.0.0.1")
        || url.starts_with("http://localhost")
        || url.starts_with("claude://claude.ai/")
}

pub struct TauriBrowser(pub AppHandle);

impl Browser for TauriBrowser {
    fn open(&self, url: &str) -> Result<(), String> {
        if !openable(url) {
            return Err("only web links and Claude links can be opened".into());
        }
        self.0
            .opener()
            .open_url(url, None::<&str>)
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::openable;

    #[test]
    fn only_expected_links_open() {
        assert!(openable("https://example.com/a?b=c"));
        assert!(openable("claude://claude.ai/new?q=hi"));
        assert!(openable("http://127.0.0.1:53682/"));
        for bad in [
            "http://example.com",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "ms-msdt:/id",
            "claude://other/",
            "",
        ] {
            assert!(!openable(bad), "{bad}");
        }
    }
}
