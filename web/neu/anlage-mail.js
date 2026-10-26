// Bereich Anlage, Abrechnung: Zustand der E-Mail je abgelegter Datei. Der Mailserver selbst ist
// ein Ausgang der Weitergabe (weitergabe-ausgaenge.js).

/// Zustand der E-Mail an einer Datei: gesendet, nicht gesendet (Grund im Tooltip) mit „Senden“
export function mailStateHtml(f, canSend) {
  const m = f.mail;
  const send = canSend ? `<button type="button" class="quiet" data-b-send="${esc(f.name)}">${esc(t(m?.sent ? "an_m_send_again" : "an_m_send"))}</button>` : "";
  if (m?.sent) return `<span class="chip ok" title="${esc(t("an_m_sent_title", slotText(m.sent)))}">${esc(t("an_m_sent"))}</span>${send}`;
  if (m?.error) return `<span class="chip warn" title="${esc(m.error)}">${esc(t("an_m_not_sent"))}</span>${send}`;
  return `<span></span>${send}`;
}
