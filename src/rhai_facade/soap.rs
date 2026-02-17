use crate::rhai_facade_validation::{
    ensure_no_nul, ensure_not_blank, normalize_timeout_ms, RhaiResult,
};
use quick_xml::events::Event;
use rhai::{Dynamic, Engine, Map};
use std::time::Duration;

pub fn register(engine: &mut Engine) {
    engine.register_fn("soap_envelope", soap_envelope);
    engine.register_fn("soap_call", soap_call);
}

fn soap_envelope(body_xml: &str) -> RhaiResult<String> {
    ensure_no_nul("body_xml", body_xml)?;
    Ok(format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Body>
    {body_xml}
  </soap:Body>
</soap:Envelope>"#
    ))
}

fn soap_call(url: &str, action: &str, body_xml: &str, timeout_ms: i64) -> RhaiResult<Map> {
    let url = ensure_not_blank("url", url)?;
    ensure_no_nul("action", action)?;
    ensure_no_nul("body_xml", body_xml)?;
    let timeout_ms = normalize_timeout_ms(timeout_ms)?.unwrap_or(10_000);

    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(timeout_ms)))
        .http_status_as_error(false)
        .build();
    let agent: ureq::Agent = config.into();

    let request = http::Request::builder()
        .method("POST")
        .uri(url.as_str())
        .header("Content-Type", "text/xml; charset=utf-8")
        .header("SOAPAction", action)
        .body(body_xml.to_string())
        .map_err(|err| format!("failed to build SOAP request: {err}"))?;

    match agent.run(request) {
        Ok(mut response) => {
            let status = response.status().as_u16() as i64;
            let body = response
                .body_mut()
                .read_to_string()
                .map_err(|err| format!("failed to read SOAP response: {err}"))?;
            let fault_text = extract_fault_text(&body);

            let mut out = Map::new();
            out.insert("ok".into(), Dynamic::from((200..400).contains(&status)));
            out.insert("status".into(), Dynamic::from(status));
            out.insert("body".into(), Dynamic::from(body));
            out.insert("fault".into(), Dynamic::from(fault_text.is_some()));
            out.insert(
                "fault_text".into(),
                Dynamic::from(fault_text.unwrap_or_default()),
            );
            Ok(out)
        }
        Err(err) => {
            let mut out = Map::new();
            out.insert("ok".into(), Dynamic::from(false));
            out.insert("status".into(), Dynamic::from(0_i64));
            out.insert("body".into(), Dynamic::from(String::new()));
            out.insert("fault".into(), Dynamic::from(false));
            out.insert("fault_text".into(), Dynamic::from(String::new()));
            out.insert("error".into(), Dynamic::from(err.to_string()));
            Ok(out)
        }
    }
}

fn extract_fault_text(xml: &str) -> Option<String> {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut in_fault = false;
    let mut current_tag = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                current_tag = String::from_utf8_lossy(start.name().as_ref())
                    .to_ascii_lowercase()
                    .to_string();
                if current_tag.ends_with("fault") {
                    in_fault = true;
                }
            }
            Ok(Event::End(end)) => {
                let tag = String::from_utf8_lossy(end.name().as_ref())
                    .to_ascii_lowercase()
                    .to_string();
                if tag.ends_with("fault") {
                    in_fault = false;
                }
                current_tag.clear();
            }
            Ok(Event::Text(text)) => {
                if !in_fault {
                    continue;
                }
                let text = text.decode().ok()?.trim().to_string();
                if text.is_empty() {
                    continue;
                }
                if current_tag.ends_with("faultstring")
                    || current_tag.ends_with("reason")
                    || current_tag.ends_with("text")
                {
                    return Some(text);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{extract_fault_text, soap_call};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn extracts_fault_text_from_faultstring() {
        let xml = r#"<Envelope><Body><Fault><faultstring>bad request</faultstring></Fault></Body></Envelope>"#;
        assert_eq!(extract_fault_text(xml).as_deref(), Some("bad request"));
    }

    #[test]
    fn soap_call_reports_fault_text() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept connection");
            let mut buf = [0_u8; 2048];
            let _ = stream.read(&mut buf);
            let body = r#"<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body><soap:Fault><faultstring>denied</faultstring></soap:Fault></soap:Body></soap:Envelope>"#;
            let response = format!(
                "HTTP/1.1 500 Internal Server Error\r\nContent-Length: {}\r\nContent-Type: text/xml\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let out =
            soap_call(&format!("http://{addr}/"), "urn:test", "<Ping/>", 5_000).expect("soap");
        handle.join().expect("join server thread");

        let fault = out
            .get("fault")
            .and_then(|value| value.clone().try_cast::<bool>())
            .unwrap_or(false);
        let fault_text = out
            .get("fault_text")
            .and_then(|value| value.clone().try_cast::<String>())
            .unwrap_or_default();
        assert!(fault, "expected SOAP fault marker: {out:?}");
        assert_eq!(fault_text, "denied");
    }
}
