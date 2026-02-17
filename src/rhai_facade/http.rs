use crate::rhai_facade_validation::{
    ensure_not_blank, map_to_strings, normalize_timeout_ms, RhaiResult,
};
use rhai::{Dynamic, Engine, Map};
use std::time::Duration;

pub fn register(engine: &mut Engine) {
    engine.register_fn("web_fetch", web_fetch);
    engine.register_fn("web_fetch_with", web_fetch_with);
}

fn web_fetch(url: &str) -> RhaiResult<Map> {
    web_fetch_with("GET", url, Map::new(), "".to_string(), 10_000)
}

fn web_fetch_with(
    method: &str,
    url: &str,
    headers: Map,
    body: String,
    timeout_ms: i64,
) -> RhaiResult<Map> {
    let method = ensure_not_blank("method", method)?;
    let url = ensure_not_blank("url", url)?;
    let headers = map_to_strings("headers", &headers)?;
    let timeout_ms = normalize_timeout_ms(timeout_ms)?.unwrap_or(10_000);

    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(timeout_ms)))
        .http_status_as_error(false)
        .build();
    let agent: ureq::Agent = config.into();

    let mut request = http::Request::builder().method(method.as_str()).uri(url.as_str());
    for (key, value) in headers {
        request = request.header(key, value);
    }
    let request = request
        .body(body)
        .map_err(|err| format!("failed to build request: {err}"))?;

    match agent.run(request) {
        Ok(mut response) => {
            let status = response.status().as_u16() as i64;
            let mut headers_out = Map::new();
            for (name, value) in response.headers() {
                headers_out.insert(
                    name.to_string().into(),
                    Dynamic::from(value.to_str().unwrap_or_default().to_string()),
                );
            }

            let body_text = response
                .body_mut()
                .read_to_string()
                .map_err(|err| format!("failed to read response body: {err}"))?;

            let mut out = Map::new();
            out.insert("ok".into(), Dynamic::from((200..400).contains(&status)));
            out.insert("status".into(), Dynamic::from(status));
            out.insert("url".into(), Dynamic::from(url));
            out.insert("method".into(), Dynamic::from(method));
            out.insert("headers".into(), Dynamic::from(headers_out));
            out.insert("body".into(), Dynamic::from(body_text));
            Ok(out)
        }
        Err(err) => {
            let mut out = Map::new();
            out.insert("ok".into(), Dynamic::from(false));
            out.insert("status".into(), Dynamic::from(0_i64));
            out.insert("url".into(), Dynamic::from(url));
            out.insert("method".into(), Dynamic::from(method));
            out.insert("headers".into(), Dynamic::from(Map::new()));
            out.insert("body".into(), Dynamic::from(String::new()));
            out.insert("error".into(), Dynamic::from(err.to_string()));
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::web_fetch;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn web_fetch_reads_local_http_response() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept connection");
            let mut buf = [0_u8; 1024];
            let _ = stream.read(&mut buf);
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nContent-Type: text/plain\r\n\r\nhello",
                )
                .expect("write response");
        });

        let out = web_fetch(&format!("http://{addr}/")).expect("fetch");
        handle.join().expect("join server thread");

        let ok = out
            .get("ok")
            .and_then(|value| value.clone().try_cast::<bool>())
            .unwrap_or(false);
        let status = out
            .get("status")
            .and_then(|value| value.clone().try_cast::<i64>())
            .unwrap_or_default();
        let body = out
            .get("body")
            .and_then(|value| value.clone().try_cast::<String>())
            .unwrap_or_default();
        assert!(ok, "expected successful response map: {out:?}");
        assert_eq!(status, 200);
        assert_eq!(body, "hello");
    }
}

