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
