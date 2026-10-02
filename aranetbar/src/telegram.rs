//! Sends alert messages through a Telegram bot, off the main thread.

use std::time::Duration;

/// Posts `text` to `chat_id`, then calls `done` from a background thread.
pub fn send(token: &str, chat_id: &str, text: &str, done: impl FnOnce(Result<(), String>) + Send + 'static) {
    let url = format!("https://api.telegram.org/bot{token}/sendMessage");
    let chat_id = chat_id.to_string();
    let text = text.to_string();
    let spawned = std::thread::Builder::new().name("telegram".into()).spawn(move || done(post(&url, &chat_id, &text)));
    if let Err(e) = spawned {
        eprintln!("telegram: {e}");
    }
}

fn post(url: &str, chat_id: &str, text: &str) -> Result<(), String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .into();
    let mut resp = agent
        .post(url)
        .send_form([("chat_id", chat_id), ("text", text)])
        .map_err(|e| format!("couldn't reach Telegram ({e})"))?;
    let body = resp.body_mut().read_to_string().unwrap_or_default();
    parse(resp.status().as_u16(), &body)
}

/// Telegram replies `{"ok":false,"description":"…"}` on failure.
fn parse(status: u16, body: &str) -> Result<(), String> {
    let json: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    if json["ok"].as_bool() == Some(true) {
        return Ok(());
    }
    let why = json["description"].as_str().map(str::to_string).unwrap_or_else(|| format!("HTTP {status}"));
    Err(match status {
        401 | 404 => format!("bot token rejected ({why})"),
        _ => why,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_telegram_replies() {
        assert_eq!(parse(200, r#"{"ok":true,"result":{}}"#), Ok(()));
        assert_eq!(parse(400, r#"{"ok":false,"description":"Bad Request: chat not found"}"#), Err("Bad Request: chat not found".into()));
        assert_eq!(parse(401, r#"{"ok":false,"description":"Unauthorized"}"#), Err("bot token rejected (Unauthorized)".into()));
        assert_eq!(parse(502, "<html>"), Err("HTTP 502".into()));
    }
}
