//! Navigation that does not wait for the full `load` event.
//!
//! `chromiumoxide::Page::goto` is held until the main frame AND every child
//! frame fire `load` (30s cap). One hanging image, iframe or analytics beacon
//! turns a usable page into "Request timed out." half a minute later. This
//! helper assigns `location` and polls the document state instead.

use anyhow::{Context, Result};
use chromiumoxide::Page;
use std::time::Duration;

/// Start a navigation and return once the new document is `interactive` or
/// `complete`. If `settle_timeout` elapses first the page is left loading and
/// `Ok(())` is returned. Chrome error pages (DNS, refused, blocked port, TLS)
/// become an `Err` that names the target.
pub async fn navigate_until_interactive(
    page: &Page,
    target: &str,
    settle_timeout: Duration,
) -> Result<()> {
    let url_json = serde_json::to_string(target).context("invalid url")?;
    let start =
        format!("(() => {{ window.__agentzNav = 1; window.location.assign({url_json}); }})()");
    // The old context may be torn down mid-call; that is the navigation working.
    let _ = page.evaluate(start.as_str()).await;

    let deadline = tokio::time::Instant::now() + settle_timeout;
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if let Ok(v) = page
            .evaluate(
                "[location.href, document.readyState, window.__agentzNav === 1, ((document.body && document.body.innerText || '').match(/ERR_[A-Z_0-9]+/) || [''])[0]]",
            )
            .await
        {
            if let Ok(arr) = v.into_value::<Vec<serde_json::Value>>() {
                let href = arr.first().and_then(|x| x.as_str()).unwrap_or("");
                let state = arr.get(1).and_then(|x| x.as_str()).unwrap_or("");
                let old_doc = arr.get(2).and_then(|x| x.as_bool()).unwrap_or(false);
                let text = arr.get(3).and_then(|x| x.as_str()).unwrap_or("");
                if href.starts_with("chrome-error://") {
                    if text.is_empty() {
                        anyhow::bail!(
                            "navigation failed: {target} could not be reached (connection refused, DNS failure, blocked port or TLS error)"
                        );
                    }
                    anyhow::bail!("navigation failed: net::{text} ({target})");
                }
                // Marker gone = a new document replaced ours; marker kept with
                // the target URL = same-document (hash) navigation.
                let arrived = !old_doc || href == target;
                if arrived && (state == "interactive" || state == "complete") {
                    return Ok(());
                }
            }
        }
        if tokio::time::Instant::now() >= deadline {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manager::{create_browser_manager, BrowserOptions};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Local server: /fast answers at once, /slow-img and /slow-iframe embed a
    /// resource that never completes.
    #[tokio::test]
    #[ignore = "requires local Chromium"]
    async fn slow_subresources_do_not_block_navigation_live() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            while let Ok((mut sock, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0u8; 2048];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let path = req
                        .lines()
                        .next()
                        .and_then(|l| l.split_whitespace().nth(1))
                        .unwrap_or("/")
                        .to_string();
                    let body = match path.as_str() {
                        "/fast" => "<h1>FAST</h1>",
                        "/slow-img" => "<h1>IMG</h1><img src=\"/hang\">",
                        "/slow-iframe" => "<h1>FRAME</h1><iframe src=\"/hang\"></iframe>",
                        "/hang" => {
                            tokio::time::sleep(Duration::from_secs(120)).await;
                            return;
                        }
                        _ => "",
                    };
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                });
            }
        });

        let mgr = create_browser_manager(BrowserOptions::default());
        let page = mgr.lock().await.get_or_create_page("nav-test").await.unwrap();
        for path in ["fast", "slow-img", "slow-iframe"] {
            let url = format!("http://{addr}/{path}");
            let t0 = std::time::Instant::now();
            navigate_until_interactive(&page, &url, Duration::from_secs(12))
                .await
                .unwrap_or_else(|e| panic!("{path}: {e:#}"));
            let dt = t0.elapsed();
            assert!(dt < Duration::from_secs(8), "{path} took {dt:?}");
        }
        let err = navigate_until_interactive(&page, "http://127.0.0.1:1/", Duration::from_secs(12))
            .await
            .unwrap_err();
        assert!(format!("{err:#}").contains("navigation failed"), "{err:#}");

        mgr.lock().await.close().await;
        server.abort();
    }
}
