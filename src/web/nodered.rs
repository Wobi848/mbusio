//! Schnittstelle für Node-RED (Konzept Node-RED): Baum zur Auswahl und laufende Werte als
//! Server-Sent Events. Nur lesend; der Bus gehört dem Plan von mbusio.

use std::collections::VecDeque;
use std::convert::Infallible;
use std::time::Duration;

use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::{self, Stream};

use crate::datapoint::{LiveState, LiveValue, Scope};

use super::*;

/// Aufbau der Antworten; Node-RED prüft das und meldet eine zu alte mbusio-Version
const VERSION: u32 = 1;

pub(super) async fn nodered_tree(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    let projects = st.db.lock().unwrap().live_tree()?;
    Ok(Json(json!({ "version": VERSION, "projects": projects })))
}

fn value_event(v: &LiveValue) -> Event {
    Event::default()
        .event("value")
        .data(serde_json::to_string(v).unwrap_or_default())
}

/// Zustand eines offenen Stroms
struct LiveStream {
    st: AppState,
    scope: Scope,
    changes: tokio::sync::watch::Receiver<u64>,
    stop: tokio::sync::watch::Receiver<bool>,
    last: LiveState,
    pending: VecDeque<Event>,
}

impl LiveStream {
    /// Nächstes Ereignis; None = mbusio fährt herunter
    async fn next(&mut self) -> Option<Event> {
        loop {
            if let Some(e) = self.pending.pop_front() {
                return Some(e);
            }
            tokio::select! {
                r = self.changes.changed() => r.ok()?,
                _ = self.stop.wait_for(|s| *s) => return None,
            }
            let now = self.st.db.lock().unwrap().live_state(self.scope);
            // Projekt gelöscht o.ä.: Strom endet, Node-RED verbindet neu und meldet den Fehler
            let now = now.ok()?;
            let (changed, values) = now.since(&self.last);
            if changed {
                self.pending
                    .push_back(Event::default().event("changed").data("{}"));
            }
            self.pending.extend(values.into_iter().map(value_event));
            self.last = now;
        }
    }
}

/// Laufende Werte eines Projekts (optional `line`, `meter`): zuerst der aktuelle Stand, dann jede
/// neue Auslesung nach der Weitergabe-Art und jede geänderte Qualität; „changed“, wenn sich
/// Datenpunkte, Zähler oder Linien geändert haben
pub(super) async fn nodered_stream(
    State(st): State<AppState>,
    Query(scope): Query<Scope>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let (changes, first) = {
        let db = st.db.lock().unwrap();
        // zuerst abonnieren, dann den Stand holen: so geht keine Änderung dazwischen verloren
        (db.changes.subscribe(), db.live_state(scope)?)
    };
    let s = LiveStream {
        stop: st.stop.subscribe(),
        st: st.clone(),
        scope,
        changes,
        pending: first.values.values().map(value_event).collect(),
        last: first,
    };
    let events = stream::unfold(s, |mut s| async move {
        let e = s.next().await?;
        Some((Ok(e), s))
    });
    Ok(Sse::new(events).keep_alive(KeepAlive::new().interval(Duration::from_secs(30))))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    use crate::mbus::master::Target;
    use crate::testutil::{db_with_energy, with_energy};
    use crate::web::{router, AppState, Shared};

    /// mbusio-Webserver auf einem freien Port; liefert Zustand und Port
    async fn server(
        token: Option<&str>,
    ) -> (AppState, u16, i64, crate::mbus::master::MeterReading) {
        let (db, line, _, r) = db_with_energy();
        let st: AppState = Arc::new(Shared::new(token.map(String::from), db));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let app = router(st.clone());
        tokio::spawn(async move { axum::serve(listener, app).await });
        (st, port, line, r)
    }

    /// Anfrage schicken; `close`: Antwort ganz lesen (sonst bleibt die Verbindung offen)
    async fn request(port: u16, path: &str, token: Option<&str>) -> TcpStream {
        let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let auth = token
            .map(|t| format!("x-token: {t}\r\n"))
            .unwrap_or_default();
        let req = format!("GET {path} HTTP/1.1\r\nHost: test\r\n{auth}Connection: close\r\n\r\n");
        s.write_all(req.as_bytes()).await.unwrap();
        s
    }

    /// Ändernde Anfrage mit JSON wie die Oberfläche; liefert die Antwort
    async fn send(port: u16, method: &str, path: &str, body: &serde_json::Value) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let body = body.to_string();
        let req = format!(
            "{method} {path} HTTP/1.1\r\nHost: test\r\ncontent-type: application/json\r\ncontent-length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        s.write_all(req.as_bytes()).await.unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).await.unwrap();
        out
    }

    async fn whole(port: u16, path: &str, token: Option<&str>) -> String {
        let mut s = request(port, path, token).await;
        let mut out = String::new();
        s.read_to_string(&mut out).await.unwrap();
        out
    }

    /// Weiterlesen, bis `needle` im Gelesenen steht (höchstens 5 s)
    async fn until(s: &mut TcpStream, seen: &mut String, needle: &str) {
        let end = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut buf = [0u8; 4096];
        while !seen.contains(needle) {
            let n = tokio::time::timeout_at(end, s.read(&mut buf))
                .await
                .unwrap_or_else(|_| panic!("„{needle}“ kam nicht, bisher: {seen}"))
                .unwrap();
            assert!(n > 0, "Strom endete vor „{needle}“: {seen}");
            seen.push_str(&String::from_utf8_lossy(&buf[..n]));
        }
    }

    #[tokio::test]
    async fn tree_and_stream_follow_readings_and_end_on_shutdown() {
        let (st, port, line, r) = server(None).await;
        // Baum: Projekt › Gateway › Linie › Zähler › Datenpunkt mit Wert
        let tree = whole(port, "/api/nodered/tree", None).await;
        assert!(tree.starts_with("HTTP/1.1 200"), "{tree}");
        let body: serde_json::Value =
            serde_json::from_str(tree.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(body["version"], 1);
        let dp = &body["projects"][0]["gateways"][0]["lines"][0]["meters"][0]["datapoints"][0];
        assert_eq!(dp["no"], 1);
        assert_eq!(dp["value"], 123456.0);
        assert_eq!(dp["quality"], "good");
        // Strom: zuerst der Stand
        let mut s = request(port, "/api/nodered/stream?project=1", None).await;
        let mut seen = String::new();
        until(&mut s, &mut seen, "\"value\":123456.0").await;
        assert!(seen.contains("text/event-stream"), "{seen}");
        assert!(seen.contains("event: value"));
        // neue Auslesung: neuer Wert kommt
        st.db
            .lock()
            .unwrap()
            .record(
                line,
                &Target::Primary(1),
                Ok(&with_energy(&r, 123999.0)),
                false,
            )
            .unwrap();
        until(&mut s, &mut seen, "\"value\":123999.0").await;
        // Datenpunkt umbenannt (wie in der Oberfläche): „changed“, Node-RED holt den Baum neu
        let (mid, d) = {
            let db = st.db.lock().unwrap();
            let mid = db.meters(line).unwrap()[0].id;
            (mid, db.dp_by_meter(mid).unwrap().remove(0))
        };
        let mut v = serde_json::to_value(&d).unwrap();
        v["name"] = "Wohnung 1 · Energie".into();
        let r = send(port, "PUT", &format!("/api/datapoints/{}", d.id), &v).await;
        assert!(r.starts_with("HTTP/1.1 200"), "{r}");
        until(&mut s, &mut seen, "event: changed").await;
        // Zähler umbenannt: auch das kommt an (ändernde Anfragen melden sich immer)
        let before = seen.matches("event: changed").count();
        let mut m = serde_json::to_value(st.db.lock().unwrap().meter(mid).unwrap()).unwrap();
        m["name"] = "Whg 1 neu".into();
        let r = send(port, "PUT", &format!("/api/meters/{mid}"), &m).await;
        assert!(r.starts_with("HTTP/1.1 200"), "{r}");
        let end = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut buf = [0u8; 4096];
        while seen.matches("event: changed").count() == before {
            let n = tokio::time::timeout_at(end, s.read(&mut buf))
                .await
                .expect("changed nach Zähler")
                .unwrap();
            seen.push_str(&String::from_utf8_lossy(&buf[..n]));
        }
        // Herunterfahren: der Strom endet, statt den Server aufzuhalten
        st.stop.send_replace(true);
        let mut rest = Vec::new();
        let n = tokio::time::timeout(std::time::Duration::from_secs(2), s.read_to_end(&mut rest))
            .await
            .expect("Strom endet beim Herunterfahren")
            .unwrap();
        assert!(n < 4096);
    }

    #[tokio::test]
    async fn access_key_and_unknown_project() {
        let (_st, port, _, _) = server(Some("geheim")).await;
        assert!(whole(port, "/api/nodered/tree", None)
            .await
            .starts_with("HTTP/1.1 401"));
        assert!(whole(port, "/api/nodered/tree", Some("geheim"))
            .await
            .starts_with("HTTP/1.1 200"));
        let r = whole(port, "/api/nodered/stream?project=9", Some("geheim")).await;
        assert!(r.starts_with("HTTP/1.1 404"), "{r}");
        // auch sonst: was es nicht gibt, ist 404 (nicht 502 wie ein Busfehler)
        let r = whole(port, "/api/projects/99/outputs/mqtt/list", Some("geheim")).await;
        assert!(r.starts_with("HTTP/1.1 404"), "{r}");
    }
}
