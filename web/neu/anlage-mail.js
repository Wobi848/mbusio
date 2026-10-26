// Bereich Anlage, Abrechnung: E-Mail (Konzept Export, Teil C). Reine Bausteine: Mailserver
// (einmal je Gerät, Passwort nie im Browser), Zustand der E-Mail je abgelegter Datei.
// Geschaltet und gespeichert wird in anlage-abrechnung.js.

/// Verschlüsselung und ihr üblicher Port
const SECURITY = { starttls: 587, tls: 465, none: 25 };

/// Felder: [name, art, extra]
const FIELDS = [
  ["host", "text", { ph: "smtp.example.org", mono: true }],
  ["port", "number", { mono: true }],
  ["security", "select"],
  ["insecure", "check"],
  ["username", "text"],
  ["password", "password"],
  ["from", "text", { ph: "mbusio@example.org", mono: true }],
];

function field([name, kind, x = {}], c, hasPassword) {
  const label = `<span>${esc(t("an_m_" + name))}</span>`;
  const cls = x.mono ? ' class="mono"' : "";
  if (kind === "check") return `<label class="check">${label}<input type="checkbox" name="${name}"${c[name] ? " checked" : ""}></label>`;
  if (kind === "select")
    return `<label>${label}<select name="${name}">${Object.keys(SECURITY).map((v) => `<option value="${v}"${v === c[name] ? " selected" : ""}>${esc(t("an_m_sec_" + v))}</option>`).join("")}</select></label>`;
  const ph = kind === "password" ? (hasPassword ? t("an_m_pass_saved") : "") : x.ph || "";
  const value = kind === "password" ? "" : esc(c[name] ?? "");
  return `<label>${label}<input type="${kind}" name="${name}" value="${value}"${cls}${ph ? ` placeholder="${esc(ph)}"` : ""}></label>`;
}

/// Mailserver als aufklappbares Feld; offen, solange Empfänger da sind, aber kein Server
export function mailServerHtml(mail, mailTo) {
  const c = mail.config;
  const open = mailTo && !c.host;
  return `<details class="more" data-mail${open ? " open" : ""}><summary>${esc(t("an_m_server"))}</summary>
    <form class="fields" data-mail-form>
      ${FIELDS.map((f) => field(f, c, mail.has_password)).join("")}
      <p class="sub hint">${esc(t("an_m_hint"))}</p>
      <div class="actions">
        <button type="submit">${esc(t("an_m_save"))}</button>
        <button type="button" class="quiet" data-mail-test${mailTo ? "" : ` disabled title="${esc(t("an_m_test_none"))}"`}>${esc(t("an_m_test"))}</button>
      </div>
    </form></details>`;
}

/// Werte aus dem Formular; leeres Passwort = gespeichertes behalten
export function mailFormValues(form) {
  const config = {};
  for (const [name, kind] of FIELDS) {
    const el = form.elements[name];
    config[name] = kind === "check" ? el.checked : kind === "number" ? Number(el.value) || 0 : el.value;
  }
  const password = config.password;
  delete config.password;
  return { config, password: password || undefined };
}

/// Andere Verschlüsselung: üblichen Port eintragen, ausser es steht ein eigener drin
export function securityChanged(select) {
  const port = select.form.elements.port;
  if (Object.values(SECURITY).includes(Number(port.value))) port.value = SECURITY[select.value];
}

/// Zustand der E-Mail an einer Datei: gesendet, nicht gesendet (Grund im Tooltip) mit „Senden“
export function mailStateHtml(f, canSend) {
  const m = f.mail;
  const send = canSend ? `<button type="button" class="quiet" data-b-send="${esc(f.name)}">${esc(t(m?.sent ? "an_m_send_again" : "an_m_send"))}</button>` : "";
  if (m?.sent) return `<span class="chip ok" title="${esc(t("an_m_sent_title", slotText(m.sent)))}">${esc(t("an_m_sent"))}</span>${send}`;
  if (m?.error) return `<span class="chip warn" title="${esc(m.error)}">${esc(t("an_m_not_sent"))}</span>${send}`;
  return `<span></span>${send}`;
}
