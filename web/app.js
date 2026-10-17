"use strict";

// ------------------------------------------------------------ Sprache

const UI = {
  de: {
    disconnected: "Getrennt",
    nav_read: "Auslesen",
    nav_config: "Einstellen",
    nav_export: "Export",
    nav_log: "Log",
    connection: "Verbindung",
    port: "Port",
    reload_ports: "Ports neu laden",
    port_manual: "Oder manuell",
    baud: "Baudrate",
    timeout: "Timeout ms",
    timeout_hint: "0 = automatisch",
    connect: "Verbinden",
    disconnect: "Trennen",
    connection_hint:
      "8 Datenbits, gerade Parität, 1 Stopbit. Netzwerk-Gateways als tcp://IP:Port angeben (Baudrate dann am Gateway einstellen). „SIMULATOR“ startet einen virtuellen Bus zum Testen.",
    scan: "Scan",
    from: "Von Adresse",
    to: "Bis Adresse",
    retries: "Wiederholungen",
    scan_primary: "Primärscan (PA)",
    scan_secondary: "Sekundärscan (SA)",
    scan_primary_start: "Primärscan starten",
    scan_secondary_start: "Sekundärscan starten",
    scan_primary_hint:
      "Fragt jede Primäradresse im Bereich einzeln ab. Schnell, aber Zähler mit gleicher Adresse (z.B. alle auf 0) erscheinen nur als Kollision.",
    scan_secondary_hint:
      "Sucht über die ID der Zähler, unabhängig von der Primäradresse. Findet auch Zähler, die sich eine Adresse teilen. Dauert länger, kein Bereich nötig.",
    cancel: "Abbrechen",
    scan_bauds: "Baudraten:",
    scan_bauds_hint:
      "„Prüfen“ fragt in wenigen Sekunden alle Baudraten ab und markiert die, auf denen Geräte antworten (●). Gescannt wird nur auf den markierten Baudraten.",
    bauds_all: "Alle",
    busbar_view: "Ansehen",
    tab_project: "Projekt",
    scan_primary_short: "Jede Adresse einzeln abfragen. Schnell.",
    scan_secondary_short: "Über die ID suchen. Findet auch Zähler mit gleicher PA.",
    reset_short: "Kommunikation des Zählers zurücksetzen.",
    tab_scan: "Scan",
    tab_meters: "Zähler",
    tab_tools: "Werkzeuge",
    tab_sim: "Simulator",
    sim_title: "Simulator",
    sim_hint:
      "Virtuelle Zähler zum Üben, Testen und Vorführen, ohne Hardware. Eine Linie mit dem Anschluss „SIMULATOR:Name“ spricht mit der gewählten Simulation. Vorlagen lassen sich kopieren und dann frei ändern.",
    sim_choose: "Simulation",
    sim_template: "Vorlage",
    sim_use: "Für aktive Linie verwenden",
    sim_copy: "Kopieren",
    sim_new: "Neu (leer)",
    sim_delete: "Löschen",
    sim_line: (name, conn) => `Aktive Linie „${name}“: ${conn || "kein Anschluss"}`,
    sim_line_uses: "verwendet diese Simulation",
    sim_builtin_note: "Vorlage, nicht änderbar. Zum Anpassen „Kopieren“.",
    sim_name: "Name",
    sim_col_pa: "PA",
    sim_col_id: "ID",
    sim_col_man: "Hersteller",
    sim_col_medium: "Medium",
    sim_col_baud: "Baudrate",
    sim_col_fault: "Verhalten",
    sim_baud_bus: "wie Bus",
    sim_add: "+ Zähler",
    sim_add_many: "+ mehrere",
    sim_add_n: "Anzahl",
    sim_save: "Speichern",
    sim_saved: (n) => "Simulation gespeichert" + (n ? " · Linie neu verbunden" : ""),
    sim_used: (c) => `Aktive Linie verbunden mit ${c}`,
    sim_copy_name: (n) => `Kopie von ${n}`,
    sim_new_prompt: "Name der neuen Simulation:",
    sim_new_name: "Meine Anlage",
    confirm_sim_delete: (n) => `Simulation „${n}“ löschen?`,
    confirm_sim_discard: "Ungespeicherte Änderungen verwerfen?",
    sim_empty: "Noch keine Zähler. Mit „+ Zähler“ oder „+ mehrere“ anlegen.",
    sim_count: (n, faults) => `${n} Zähler` + (faults ? ` · ${faults} mit Fehler` : " · alle in Ordnung"),
    sim_dup_pa: (pa) => `PA ${pa} mehrfach: die Zähler antworten gleichzeitig (Kollision)`,
    sim_dup_id: (id) => `ID ${id} mehrfach: Sekundärsuche findet sie nicht getrennt`,
    sim_pa0: (n) => `${n} Zähler mit PA 0 (Werkszustand): nur über die Sekundäradresse erreichbar`,
    sim_m_heat: "Wärme",
    sim_m_cooling: "Kälte",
    sim_m_water: "Wasser",
    sim_m_cold_water: "Kaltwasser",
    sim_m_warm_water: "Warmwasser",
    sim_m_gas: "Gas",
    sim_m_electricity: "Strom",
    sim_m_heat_old: "Wärme alt (feste Struktur)",
    sim_f_none: "in Ordnung",
    sim_f_checksum: "Prüfsumme falsch",
    sim_f_truncated: "Telegramm abgeschnitten",
    sim_f_busy: "meldet „beschäftigt“",
    sim_f_wrong_address: "antwortet mit falscher Adresse",
    sim_f_flaky: "Wackelkontakt",
    sim_f_too_many: "nach 2 Auslesungen „zu viele“",
    help_toggle: "Erklärungen ein-/ausblenden",
    conn_change: "Ändern",
    conn_summary: (p, b) => `Verbunden: ${p} · ${b} Bd`,
    gw_sum: (n, issues) => `${n} Linie${n === 1 ? "" : "n"}` + (issues ? ` · ⚠ ${issues}` : ""),
    project: "Projekt",
    crumb_title: "Projekt › Gateway › Linie: hier wechseln",
    project_switch: "Projekt wechseln",
    project_new: "+ Neues Projekt",
    project_new_prompt: "Name des neuen Projekts (z.B. Objekt oder Adresse):",
    project_edit: "Projekt bearbeiten",
    project_delete: "Projekt löschen",
    confirm_project_delete: (n) => `Projekt „${n}“ mit allen Gateways, Linien, Zählern und Auslesungen löschen?\n\nDas lässt sich nicht rückgängig machen. Tipp: vorher „Exportieren“.`,
    project_imported: (n) => `Projekt „${n}“ importiert`,
    gw_add: "+ Gateway",
    gw_new: (n) => `Gateway ${n}`,
    move_up: "Nach oben",
    move_down: "Nach unten",
    ln_add: "+ Linie",
    ln_new: (n) => `Linie ${n}`,
    ln_use: "Wählen",
    ln_active: "Aktiv",
    ln_no_conn: "Anschluss noch nicht festgelegt",
    ln_info: (n, read) => `${n} Zähler` + (read ? ` · gelesen ${read}` : ""),
    plan: "Ausleseplan",
    plan_time: "Uhrzeit",
    plan_opt_off: "Aus (nur von Hand)",
    plan_opt_monthly: "Monatlich (am 1.)",
    plan_opt_daily: "Täglich",
    plan_opt_hourly: "Stündlich",
    plan_opt_15min: "Alle 15 min",
    plan_inherit_project: (x) => `Wie Projekt (${x})`,
    plan_inherit_line: (x) => `Wie Linie (${x})`,
    plan_off: "aus",
    plan_monthly: (time) => `monatlich am 1., ${time}`,
    plan_daily: (time) => `täglich ${time}`,
    plan_hourly: "stündlich",
    plan_15min: "alle 15 min",
    plan_hint_project:
      "Vorgabe für alle Linien des Projekts. Standard ist Aus: geplant ausgelesen wird nur, was man bewusst einschaltet. Batteriezähler nicht öfter als nötig auslesen, meist reicht 1× täglich.",
    plan_hint_line: "Gilt für alle Zähler der Linie, die keinen eigenen Plan haben.",
    plan_hint_meter: "Nur nötig, wenn dieser Zähler anders ausgelesen werden soll als die Linie.",
    plan_next: (x) => `nächster ${x}`,
    plan_meters_own: (n) => `${n} Zähler mit eigenem Plan`,
    dp_title: "Datenpunkte",
    dp_mqtt_on: "MQTT an",
    dp_mqtt_off: "MQTT aus",
    f_mqtt: "per MQTT weitergeben",
    f_topic: "Topic (nach dem Basis-Topic)",
    f_topic_ph: "wird beim Einschalten vorgeschlagen",
    mqtt_hint:
      "Gibt die Datenpunkte mit Häkchen „MQTT“ an einen Broker weiter (z.B. Leitsystem): ein Topic je Datenpunkt, Inhalt JSON mit Wert, Einheit, Zeit und Qualität, retained, QoS 1. Ist der Broker nicht erreichbar, sammelt mbusio die Werte bis 7 Tage und liefert sie in richtiger Reihenfolge nach.",
    mqtt_enabled: "MQTT einschalten",
    mqtt_host: "Broker (Adresse)",
    mqtt_port: "Port",
    mqtt_insecure: "Zertifikat nicht prüfen (selbst signiert)",
    mqtt_user: "Benutzer",
    mqtt_password: "Passwort",
    mqtt_pass_saved: "gespeichert (leer = behalten)",
    mqtt_client: "Client-ID",
    mqtt_base: "Basis-Topic",
    mqtt_time: (t) => `Uhrzeit vom Leitsystem übernehmen (Topic ${t})`,
    mqtt_ca: "Eigenes CA-Zertifikat (PEM, optional)",
    mqtt_example: (topic) => `Beispiel: ${topic} → {"value": 123456, "unit": "kWh", "time": "…Z", "quality": "good", …} · Status: online/offline`,
    mqtt_off: "aus",
    mqtt_connecting: "verbindet …",
    mqtt_connected: "verbunden",
    mqtt_error: "getrennt",
    mqtt_state_off: "MQTT ist aus. Es wird nichts gesammelt oder gesendet.",
    mqtt_state_connected: (b, since, sent, q) => `Verbunden mit ${b} seit ${since} · ${sent} gesendet · ${q} in der Warteschlange`,
    mqtt_state_connecting: (b) => `Verbinde mit ${b} …`,
    mqtt_state_error: (b, e, q) => `Keine Verbindung zu ${b}: ${e} · ${q} Nachrichten warten und werden nachgeliefert`,
    mqtt_saved: "MQTT-Einstellungen gespeichert",
    mqtt_busbar: (q) => `MQTT getrennt` + (q ? ` · ${q} warten` : ""),
    dp_hint:
      "Die Werte, die weitergegeben werden (sobald MQTT oder Modbus eingerichtet ist). Auswahl je Zähler über „Werte“ in der Zählerliste. Die Kennung beruht auf der Bedeutung des Werts und bleibt stabil, auch wenn ein Zähler nach einem Update die Reihenfolge ändert.",
    dp_empty: "Noch keine Datenpunkte. In der Zählerliste bei einem Zähler auf „Werte“ klicken und auswählen.",
    dp_csv_export: "CSV exportieren",
    dp_csv_import: "CSV importieren",
    dp_select_all: "Alle auswählen",
    th_no: "Nr",
    th_dp: "Datenpunkt",
    th_value: "Wert",
    th_quality: "Qualität",
    th_send: "Weitergabe",
    th_active: "Aktiv",
    q_good: "gut",
    q_stale: "veraltet",
    q_disturbed: "gestört",
    q_invalid: "ungültig",
    q_none: "kein Wert",
    q_stale_title: "Der letzte Wert ist älter als 3 Termine des Ausleseplans.",
    q_disturbed_title: "Der Zähler antwortet nicht.",
    q_decreased_title: "Zählerstand gesunken. Wurde der Zähler getauscht? Dann beim Zähler „Bearbeiten → Zähler getauscht…“ verwenden.",
    q_status_title: "Der Zähler meldet einen Fehler in seinem Status.",
    send_every: "bei jeder Auslesung",
    send_change: "nur bei Änderung",
    send_daily: "1× täglich",
    dp_selected: (n) => `${n} ausgewählt:`,
    dp_on: "Aktiv an",
    dp_off: "Aktiv aus",
    dp_delete: "Löschen",
    dp_send_to: "Weitergabe …",
    confirm_dp_delete: (n) => `${n} Datenpunkte löschen? Ihre Nummern werden nicht wieder vergeben.`,
    dp_values_btn: "Werte",
    dp_values_title: "Werte als Datenpunkte wählen",
    dp_pick_title: (name) => `Datenpunkte: ${name}`,
    dp_pick_read: (ts) => `Werte der Auslesung vom ${ts}`,
    dp_pick_none: "Noch keine Auslesung. Zähler zuerst auslesen, dann sind seine Werte hier wählbar.",
    dp_main: "Hauptwert",
    dp_missing: "fehlt in der letzten Auslesung",
    dp_tpl: (medium) => `Gleiche Auswahl für alle Zähler „${medium}“ dieser Linie`,
    dp_tpl_auto: "auch für Zähler, die später dazukommen",
    dp_saved: (a, r, o) => `Gespeichert: ${a} neu, ${r} entfernt` + (o ? `, ${o} bei anderen Zählern` : ""),
    dp_templates: "Vorlagen:",
    dp_tpl_n: (n) => (n === 1 ? "1 Wert" : `${n} Werte`),
    dp_tpl_auto_short: "auch neue Zähler",
    dp_tpl_remove: "Vorlage entfernen",
    confirm_tpl_remove: (m) => `Vorlage für „${m}“ entfernen? Vorhandene Datenpunkte bleiben.`,
    dp_form_title: "Datenpunkt bearbeiten",
    f_unit: "Einheit",
    f_factor: "Faktor",
    f_factor_hint: "Angezeigter Wert = Wert des Zählers × Faktor, z.B. 0.001 für Wh → kWh. Weitergabe wirkt, sobald MQTT oder Modbus eingerichtet ist.",
    f_send: "Weitergabe",
    f_active: "Aktiv (weitergeben)",
    dp_import_preview: (c, a, e) => `Vorschau: ${c} Änderungen, ${a} neue Datenpunkte, ${e} Fehler`,
    dp_import_apply: "Übernehmen",
    dp_import_cancel: "Verwerfen",
    dp_import_done: "CSV übernommen",
    dp_import_nothing: "Die Datei enthält keine Änderungen.",
    dp_import_errors: "Wegen der Fehler wird nichts übernommen. Datei korrigieren und erneut importieren.",
    dp_field_name: "Name",
    dp_field_unit: "Einheit",
    dp_field_factor: "Faktor",
    dp_field_send: "Weitergabe",
    dp_field_active: "Aktiv",
    dp_new: "neu",
    exchange_btn: "Zähler getauscht…",
    exchange_title: "Neuer Zähler am selben Platz: Zählerstände laufen nahtlos weiter.",
    exchange_prompt: "Adresse des neuen Zählers (Primäradresse, ID oder Sekundäradresse):",
    exchange_confirm: (o, n) =>
      `Zähler ${o} durch ${n} ersetzen?\n\nmbusio liest den neuen Zähler aus und führt die Zählerstände der Datenpunkte nahtlos weiter (alter Endstand → neuer Anfangsstand). Das steht danach im Bericht unter den Änderungen.`,
    exchange_done: "Zähler getauscht",
    ln_dp: (n) => `${n} Datenpunkte`,
    ln_dp_invalid: (n) => `${n} ungültig`,
    thin: "Verlauf",
    thin_off: "Alles behalten (Standard)",
    thin_days: (n) => `Ab ${n} Tagen 1 Wert/Tag`,
    thin_hint: "Nur bei häufigen Plänen nötig. Ohne diese Einstellung löscht mbusio nie Messwerte.",
    disk_low: (p) => `Speicher knapp: ${p} % frei`,
    disk_full: (p) => `Speicher voll: Messwerte werden nicht mehr gespeichert (${p} % frei)`,
    disk_title:
      "Platz schaffen: Messwerte exportieren und alte löschen, oder im Projekt „Verlauf“ ausdünnen. Gelesen und angezeigt wird weiterhin.",
    plan_last: (ok, n) => `zuletzt ${ok}/${n} gelesen`,
    plan_disturbed: (n) => `${n} gestört`,
    plan_paused: (n) => `${n} ausgesetzt`,
    plan_load: (p) => `Buslast ${p} %`,
    plan_load_title:
      "Die geplanten Auslesungen brauchen fast die ganze Buszeit dieser Linie. Seltener auslesen oder Zähler auf mehrere Linien verteilen.",
    plan_held: "pausiert, von Hand getrennt",
    plan_held_title: "Der Ausleseplan läuft weiter, sobald die Linie wieder verbunden wird.",
    plan_offline: "nicht verbunden",
    plan_line_down: "Linie gestört: kein Zähler hat geantwortet",
    plan_running: (label, d, n) => `Plan ${label} liest · ${d}/${n}`,
    plan_saved_warn: (w) => `Gespeichert. Achtung: ${w}`,
    plan_paused_disconnect: "Getrennt. Der Ausleseplan dieser Linie ist pausiert, bis wieder verbunden wird.",
    col_plan: "Plan",
    meter_disturbed: "gestört",
    meter_disturbed_title: (n) => `${n} Termine nacheinander nicht gelesen. Wird weiter versucht.`,
    meter_paused: (x) => `ausgesetzt bis ${x}`,
    meter_paused_title:
      "Der Zähler hat abgelehnt (beschäftigt oder zu viele Auslesungen). Nächster Versuch am nächsten Tag.",
    day_today: "heute",
    day_tomorrow: "morgen",
    time_unsure: "Uhrzeit nicht bestätigt",
    time_unsure_title:
      "Weder Zeitserver noch Uhr-Modul bestätigen die Uhrzeit. Werte werden trotzdem gelesen, aber als „Zeit unsicher“ gekennzeichnet. Klicken, um die Uhrzeit dieses Geräts zu übernehmen.",
    confirm_time: (x) => `Uhrzeit dieses Geräts übernehmen?\n\n${x}\n\nNeue Auslesungen gelten danach als zeitlich sicher (bis zum nächsten Neustart von mbusio).`,
    time_taken: (s) => (s ? `Uhrzeit übernommen (Korrektur ${s > 0 ? "+" : ""}${s} s)` : "Uhrzeit bestätigt"),
    reading_time_unsure: "Zeit unsicher",
    reading_time_unsure_title: "Beim Auslesen war die Uhrzeit von mbusio nicht bestätigt; der Zeitstempel kann falsch sein.",
    ln_issues: (n) => `${n} Auffälligkeit${n === 1 ? "" : "en"}`,
    ln_no_issues: "keine Auffälligkeiten",
    confirm_gw_delete: (n) => `Gateway „${n}“ mit allen Linien und Zählern löschen?`,
    confirm_ln_delete: (n) => `Linie „${n}“ mit allen Zählern und Auslesungen löschen?`,
    form_project: "Projekt",
    form_gateway: "Gateway",
    form_line: "Linie",
    f_name: "Name",
    f_customer: "Kunde",
    f_site: "Objekt / Anlage",
    f_technician: "Bearbeiter",
    f_order: "Auftrag",
    f_remarks: "Bemerkungen",
    f_address: "Adresse (z.B. IP)",
    f_note: "Notiz",
    f_connection: "Anschluss",
    f_connection_ph: "COM3, /dev/ttyUSB0, tcp://IP:Port, SIMULATOR",
    conn_line: (l) => `Verbindung gehört zur Linie „${l}“ und wird dort gespeichert.`,
    activated: (l) => `Linie „${l}“ aktiv`,
    activate_failed: (l, e) => `Linie „${l}“ aktiv, aber nicht verbunden: ${e}`,
    confirm_cancel_scan: "Scan wirklich abbrechen?\n\nDie bisher gefundenen Zähler bleiben in der Liste.",
    confirm_cancel_read: "Auslesen wirklich abbrechen?\n\nBereits gelesene Zähler behalten ihre Werte.",
    idle_ready: (p, b) => `Bereit · ${p} · ${b} Bd`,
    idle_off: "Nicht verbunden",
    idle_connect: "Verbinden",
    issues: (n) => `⚠ ${n} Auffälligkeit${n === 1 ? "" : "en"}`,
    issues_none: "✓ Keine Auffälligkeiten",
    dup_pa: "PA doppelt",
    other_baud: "Baudrate abweichend",
    other_baud_title: (b, bus) => `Zähler läuft mit ${b} Bd, der Bus mit ${bus} Bd. Empfehlung: auf ${bus} Bd umstellen.`,
    fix_baud: "Baudrate ändern",
    config_baud_note: (b) => `Zähler läuft mit ${b} Bd – Befehle werden mit ${b} Bd gesendet.`,
    remember_all: "Alle merken",
    remember_via: "Merken über",
    mode_sa_title: "Sekundäradresse: eindeutig und bleibt gleich, auch wenn die PA geändert wird (empfohlen)",
    mode_pa_title: "Primäradresse: wenn ein Leitsystem oder Gateway die Zähler über die PA abfragt",
    confirm_mode: (m, n) => `Linie auf „Merken über ${m}“ umstellen?\n\nDie ${n} Zähler der Liste werden mit umgestellt, soweit sie eindeutig sind.`,
    mode_done: (m, n) => `Merken über ${m} · ${n} Zähler umgestellt`,
    mode_kept: (n) => `${n} nicht umstellbar`,
    remember_skipped: (n) => `${n} nicht eindeutig, nicht gemerkt (zuerst „Alle beheben“)`,
    skip_pa_unknown: "Primäradresse unbekannt: nicht über PA merkbar",
    skip_pa_zero: "PA 0 ist der Werkszustand: zuerst „PA ändern“",
    skip_pa_dup: (pa) => `PA ${pa} ist doppelt: zuerst „PA ändern“`,
    skip_sa_unknown: "Sekundäradresse unbekannt: nicht über SA merkbar",
    skip_sa_dup: "Sekundäradresse nicht eindeutig (gleiche ID und Hersteller): Linie auf „Merken über PA“ umstellen",
    spot_pa: (id, a) => `Zähler ${id} bekommt die freie Adresse ${a}. Jetzt „Setzen“ drücken.`,
    spot_baud: (id, from, to) => `Zähler ${id} läuft mit ${from} Bd. Jetzt „Umstellen“ drücken, dann läuft er mit ${to} Bd wie der Bus.`,
    spot_done_pa: (id, a) => `✓ Erledigt: Zähler ${id} hat jetzt Adresse ${a}.`,
    spot_done_baud: (id, b) => `✓ Erledigt: Zähler ${id} läuft jetzt mit ${b} Bd.`,
    back_to_hits: "← zurück zur Trefferliste",
    remembered_all: (n, skipped) => `${n} Zähler übernommen` + (skipped ? `, ${skipped} schon in der Liste` : ""),
    dup_id: "ID + Hersteller doppelt",
    dup_id_info: "ID mehrfach",
    dup_id_info_title: "Kein Fehler: IDs sind nur je Hersteller eindeutig. mbusio unterscheidet die Zähler über die volle Sekundäradresse.",
    dup_pa_title: (pa, n) => `${n} Zähler stehen auf PA ${pa}. Über die ID neu adressieren.`,
    dup_id_title: "Gleiche ID und gleicher Hersteller: nicht unterscheidbar. Der Zähler muss vom Hersteller umprogrammiert werden.",
    fix_pa: "PA ändern",
    warn_title: "Auffälligkeiten",
    warn_dup_pa: (pa, b, ids) => `PA ${pa} (${b} Bd): ${ids.length} Zähler – ${ids.join(", ")}`,
    warn_dup_id: (id, n) => `ID ${id}: ${n} Zähler mit gleicher ID und gleichem Hersteller – vom Hersteller umprogrammieren lassen`,
    warn_collision_pa: (pa, b) => `PA ${pa} (${b} Bd): mehrere Zähler antworten gleichzeitig – Empfehlung: Sekundärscan, dann „PA ändern“`,
    warn_collision_id: (id) => `ID ${id}: mehrere Zähler mit derselben ID – mit dem Primärscan einzeln finden`,
    why: "Warum?",
    history: "Verlauf",
    history_title: (n) => `Verlauf: ${n}`,
    history_series: "Wert",
    history_few: "Für einen Verlauf braucht es mindestens zwei Auslesungen. „Alle auslesen“ später nochmal drücken.",
    history_none: "Noch keine Auslesung gespeichert.",
    history_first: "Erste",
    history_last: "Letzte",
    history_diff: "Differenz",
    history_per_day: "pro Tag",
    history_table: "Tabelle",
    history_chart: "Diagramm",
    history_csv: "CSV",
    history_close: "Schliessen",
    th_time: "Zeitpunkt",
    th_delta: "Δ zur vorherigen",
    chart_label: (n, v) => `Verlauf ${n}, letzter Wert ${v}`,
    sort_title: "Klicken zum Sortieren (auf, ab, wie gescannt)",
    fix_all: "Alle beheben",
    plan_title: (n) => `Plan: ${n} Änderung${n === 1 ? "" : "en"}`,
    plan_hint: "Häkchen entfernen bei allem, was nicht geändert werden soll. Jede Änderung wird am Zähler kontrolliert.",
    plan_manual: "Von Hand zu erledigen:",
    plan_manual_collision: (pa, b) => `PA ${pa} (${b} Bd): Kollision – zuerst Sekundärscan, dann sind die Zähler einzeln sichtbar`,
    plan_manual_nodata: (x) => `${x}: liefert keine Daten – „Warum?“ zeigt die Ursache`,
    plan_manual_sameid: (id) => `ID ${id}: gleiche ID und Hersteller – vom Hersteller umprogrammieren lassen`,
    plan_manual_id_collision: (id) => `ID ${id}: mehrere Zähler mit dieser ID – Primärscan zeigt sie einzeln (bei gleichem Hersteller: umprogrammieren lassen)`,
    plan_none: "Nichts lässt sich automatisch beheben.",
    plan_run: (n) => `Ausführen (${n})`,
    plan_close: "Schliessen",
    plan_busy_free: "Suche freie Primäradressen …",
    plan_running: (k, n) => `Behebe ${k}/${n} …`,
    plan_done: (ok, n) => `${ok} von ${n} Änderungen erledigt`,
    plan_baud: (id, a, b) => `${id}: Baudrate ${a} → ${b} Bd`,
    plan_pa: (id, a, b) => `${id}: Primäradresse ${a} → ${b}`,
    change_pa: (id, a, b) => `${id}: Primäradresse ${a} → ${b}`,
    change_baud: (id, a, b) => `${id}: Baudrate ${a} → ${b} Bd`,
    why_tip: "Was tun:",
    why_raw: "Empfangen",
    why_unknown: ["Keine genauere Ursache bekannt.", "Telegramm-Log öffnen und die Zeilen zu diesem Zähler ansehen."],
    why_collision: ["Mehrere Zähler antworten gleichzeitig auf diese Adresse.", "Sekundärscan starten, dann bei den doppelten Zählern „PA ändern“."],
    why_checksum: ["Die Prüfsumme stimmt nicht: Bytes wurden unterwegs verfälscht.", "Verkabelung, Klemmen und Abschirmung prüfen. Langsamere Baudrate versuchen (z.B. 2400 statt 9600). Zu viele Zähler oder zu lange Leitung für den Pegelwandler?"],
    why_stop: ["Das Endezeichen fehlt: das Telegramm ist gestört.", "Verkabelung und Abschirmung prüfen, langsamere Baudrate versuchen."],
    why_garbled: ["Unlesbare Antwort: Störung oder mehrere Zähler gleichzeitig.", "Mit dem Sekundärscan prüfen, ob mehrere Zähler auf der Adresse sind. Sonst Verkabelung prüfen."],
    why_truncated: ["Das Telegramm ist abgeschnitten: der Rest kam nicht an.", "Unter Verbindung den Timeout erhöhen (z.B. 1000 ms) und neu verbinden. Sonst Verkabelung prüfen."],
    why_no_reply: ["Der Zähler meldet sich, schickt aber keine Daten.", "Später nochmal versuchen. Manche Zähler brauchen nach dem Einschalten Zeit oder haben das Auslesen gesperrt (Hersteller-Doku)."],
    why_unexpected: ["Der Zähler antwortet mit einem unerwarteten Telegramm.", "Telegramm analysieren lassen. Evtl. kein Standard-M-Bus-Zähler."],
    why_wrong_address: (a) => [`Der Zähler antwortet mit Adresse ${a} statt mit der abgefragten.`, "Zähler ist falsch konfiguriert. Über die Sekundäradresse ansprechen und mit „PA ändern“ neu setzen."],
    why_app_error: (code, text) => [`Der Zähler meldet selbst einen Fehler: ${text} (Code ${code}).`, code === "08" ? "Später nochmal auslesen." : "Fehlercode in der Hersteller-Doku nachschlagen."],
    why_decode: ["Die Daten lassen sich nicht lesen (unbekanntes Format).", "Telegramm analysieren lassen. Evtl. herstellerspezifisches Format."],
    why_same_id: ["Mehrere Zähler haben dieselbe ID und sind über die ID allein nicht trennbar.", "Mit dem Primärscan einzeln finden. Bei gleichem Hersteller: vom Hersteller umprogrammieren lassen."],
    app_errors: { "00": "nicht näher bezeichnet", "01": "Funktion nicht unterstützt", "02": "Puffer zu lang", "03": "zu viele Datensätze", "04": "Datensatz vorzeitig beendet", "05": "mehr als 10 DIFE", "06": "mehr als 10 VIFE", "08": "Zähler beschäftigt", "09": "zu viele Auslesungen" },
    start_secondary: "Sekundärscan starten",
    warn_other_baud: (n, b) => `${n} Zähler nicht auf der eingestellten Baudrate (${b} Bd) – Empfehlung: „Baudrate ändern“`,
    warn_fix_hint: "„PA ändern“ sucht eine freie Adresse auf dem Bus und trägt sie mit der ID unter „Zähler einstellen“ ein.",
    job_done: (k, n) => `${k} fertig · ${n} Treffer`,
    job_cancelled: (k, n) => `${k} abgebrochen · ${n} Treffer`,
    busy_read: "Lese Zähler …",
    busy_set: "Setze Primäradresse …",
    busy_baud: "Stelle Baudrate um …",
    busy_detect: "Suche Baudrate …",
    busy_reset: "Sende Reset …",
    busy_raw: "Sende Telegramm …",
    busy_free: "Suche freie Primäradresse …",
    bauds_none: "Keine",
    bus_bauds: "Prüfen",
    pick_baud: "Für den Scan mindestens eine Baudrate markieren",
    estimate: (t) => `Primärscan dauert höchstens ca. ${t}`,
    bus_bauds_busy: "Prüfe 300 bis 38400 Bd …",
    bus_bauds_step: (b, k, n, found) => `Prüfe ${b} Bd … (${k}/${n})` + (found.length ? ` · Antwort auf ${found.join(", ")} Bd` : ""),
    bus_bauds_checking: "Prüfe …",
    theme_auto: "Design wie System",
    theme_light: "Hell",
    theme_dark: "Dunkel",
    bus_bauds_none: "Auf keiner Baudrate antwortet ein Gerät",
    bus_bauds_found: (l, b) => `Geräte antworten auf: ${l} Bd` + (l.split(", ").some((x) => Number(x) !== b) ? " – Achtung, nicht nur auf der eingestellten Baudrate!" : ""),
    duration: (s) => (s < 90 ? `${Math.ceil(s)} s` : `${Math.ceil(s / 60)} min`),
    col_baud: "Baud",
    col_primary: "PA",
    col_secondary: "Sekundäradresse",
    col_id: "ID",
    col_version: "Version",
    sa_raw: "Sekundäradresse als Hex (16-stellig)",
    col_manufacturer: "Hersteller",
    col_medium: "Medium",
    col_status: "Status",
    read_meter: "Zähler auslesen",
    address: "Adresse",
    address_ph: "Primär (0–250), ID (8) oder Sekundär (16)",
    address_ph_short: "Primär, ID oder Sekundär",
    read: "Auslesen",
    reading_busy: "Lese …",
    set_primary: "Primäradresse setzen",
    new_primary: "Neue Primäradresse",
    set: "Setzen",
    set_hint: "Zähler über Primäradresse, ID oder Sekundäradresse angeben. Stehen mehrere Zähler auf derselben Adresse (z.B. 0), die ID verwenden.",
    configure: "Zähler einstellen",
    meter: "Zähler",
    baud_title: "Baudrate",
    new_baud: "Neue Baudrate",
    switch: "Umstellen",
    detect_baud: "Baudrate suchen",
    baud_hint: "„Suchen“ probiert 300 bis 38400 Bd durch. Nach dem Umstellen antwortet der Zähler nur noch mit der neuen Baudrate; in der Zählerliste wird sie übernommen.",
    confirm_switch: (a, b) => `Zähler ${a} auf ${b} Bd umstellen?`,
    baud_switched: (b) => `Zähler läuft jetzt mit ${b} Bd`,
    baud_switched_list: (b) => `Zähler läuft jetzt mit ${b} Bd, in der Zählerliste übernommen`,
    detect_busy: "Suche …",
    detect_none: "Keine Antwort auf allen Baudraten",
    detect_found: (l) => `Antwortet mit: ${l} Bd`,
    reset_title: "Application Reset",
    reset_hint: "Setzt die Kommunikation des Zählers zurück (z.B. Auswahl der Telegramme). Zählerstände bleiben erhalten.",
    reset: "Reset senden",
    confirm_reset: (a) => `Application Reset an Zähler ${a} senden?`,
    reset_done: "Reset quittiert",
    meters: "Zählerliste",
    meters_hint: "Zähler merken, benennen und gesammelt auslesen. Die Liste wird auf dem Rechner gespeichert, auf dem mbusio läuft.",
    meters_file: (p) => `Datei: ${p}`,
    meters_empty: "Noch keine Zähler. In der Scan-Liste „Merken“ klicken oder „Zähler hinzufügen“.",
    meter_add: "Zähler hinzufügen",
    meters_read_all: "Alle auslesen",
    meters_clear: "Liste leeren",
    report: "Bericht (PDF)",
    report_title: "Bericht für den Kunden: drucken oder als PDF speichern",
    confirm_meters_clear: (n) => `Alle ${n} Zähler aus der Liste löschen?\n\nDas lässt sich nicht rückgängig machen. Tipp: vorher „Exportieren“.`,
    meters_cleared: (n) => `${n} Zähler gelöscht`,
    export: "Exportieren",
    import: "Importieren",
    meter_name: "Name",
    meter_name_ph: "z.B. Whg. 3 Warmwasser",
    meter_location: "Ort",
    meter_location_ph: "z.B. Keller links",
    meter_note: "Notiz",
    baud_bus: "wie Bus",
    save: "Speichern",
    edit: "Bearbeiten",
    remember: "Merken",
    remembered: "In Zählerliste übernommen",
    last_value: "Letzter Wert",
    meter_saved: "Gespeichert",
    confirm_delete_meter: (n) => `„${n}“ aus der Zählerliste entfernen?`,
    imported: (n) => `${n} Zähler importiert`,
    import_bad: "Datei enthält keine Zählerliste",
    job_read_all: "Alle auslesen",
    read_all_done: "Zählerliste ausgelesen",
    read_all_cancelled: "Auslesen der Zählerliste abgebrochen",
    raw_title: "Telegramm analysieren / senden",
    raw_hint: "Bytes eingeben oder oben eine TX/RX-Zeile anklicken. „Analysieren“ erklärt das Telegramm Byte für Byte, „Senden“ schickt es unverändert an den Bus (Experten). Beispiel: 10 40 01 41 16.",
    analyze: "Analysieren",
    an_problems: "Probleme",
    an_clean: "✓ Telegramm in Ordnung",
    an_bytes: "Bytes",
    an_field: "Feld",
    an_meaning: "Bedeutung",
    log_click: "Klicken zum Analysieren",
    why_analyze: "Telegramm analysieren",
    raw_bytes: "Bytes (Hex)",
    send: "Senden",
    raw_none: "Keine Antwort",
    raw_ack: "ACK (E5)",
    raw_collision: "Gestörte Antwort",
    raw_frame: "Telegramm",
    readings_export: "Messwerte & Export",
    readings_count: (n) => `Auslesung${n === 1 ? "" : "en"} gespeichert`,
    delete: "Löschen",
    telegram_log: "Telegramm-Log",
    clear_log: "Log leeren",
    follow: "mitscrollen",
    connected_to: (p) => `Verbunden mit ${p}`,
    job_primary: "Primärscan",
    job_secondary: "Sekundärscan",
    found: (n) => `${n} gefunden`,
    hits: (n) => `${n} Treffer`,
    st_ok: "OK",
    st_collision: "Kollision",
    st_no_data: "Keine Daten",
    meta_id: "ID",
    meta_manufacturer: "Hersteller",
    meta_medium: "Medium",
    meta_version: "Version",
    meta_primary: "Primäradresse",
    meta_secondary: "Sekundäradresse",
    meta_access: "Zugriffsnr.",
    meta_status: "Status",
    meta_time: "Zeitpunkt",
    th_desc: "Beschreibung",
    th_value: "Wert",
    th_unit: "Einheit",
    th_kind: "Art",
    storage: "Speicher",
    tariff: "Tarif",
    subunit: "Untereinheit",
    enter_address: "Adresse eingeben",
    enter_both: "Aktuelle und neue Adresse eingeben",
    confirm_set: (a, n) => `Primäradresse von ${a} auf ${n} ändern?`,
    address_set: (n) => `Neue Primäradresse ${n} gesetzt`,
    confirm_clear: "Alle gespeicherten Auslesungen löschen?",
    token_prompt: "Zugriffsschlüssel (Token) eingeben:",
  },
  en: {
    disconnected: "Disconnected",
    nav_read: "Read",
    nav_config: "Configure",
    nav_export: "Export",
    nav_log: "Log",
    connection: "Connection",
    port: "Port",
    reload_ports: "Reload ports",
    port_manual: "Or enter manually",
    baud: "Baud rate",
    timeout: "Timeout ms",
    timeout_hint: "0 = automatic",
    connect: "Connect",
    disconnect: "Disconnect",
    connection_hint:
      "8 data bits, even parity, 1 stop bit. Network gateways as tcp://IP:port (set the baud rate on the gateway). “SIMULATOR” starts a virtual bus for testing.",
    scan: "Scan",
    from: "From address",
    to: "To address",
    retries: "Retries",
    scan_primary: "Primary scan (PA)",
    scan_secondary: "Secondary scan (SA)",
    scan_primary_start: "Start primary scan",
    scan_secondary_start: "Start secondary scan",
    scan_primary_hint:
      "Polls every primary address in the range. Fast, but meters sharing an address (e.g. all on 0) only show up as a collision.",
    scan_secondary_hint:
      "Searches by meter ID, independent of the primary address. Also finds meters that share an address. Takes longer, no range needed.",
    cancel: "Cancel",
    scan_bauds: "Baud rates:",
    scan_bauds_hint:
      "“Check” polls all baud rates in a few seconds and selects those where devices answer (●). The scan only runs at the selected baud rates.",
    bauds_all: "All",
    busbar_view: "View",
    tab_project: "Project",
    scan_primary_short: "Poll every address one by one. Fast.",
    scan_secondary_short: "Search by ID. Also finds meters sharing a PA.",
    reset_short: "Reset the meter's communication.",
    tab_scan: "Scan",
    tab_meters: "Meters",
    tab_tools: "Tools",
    tab_sim: "Simulator",
    sim_title: "Simulator",
    sim_hint:
      "Virtual meters for practice, testing and demos, no hardware needed. A line with the connection “SIMULATOR:name” talks to the chosen simulation. Templates can be copied and then changed freely.",
    sim_choose: "Simulation",
    sim_template: "template",
    sim_use: "Use for active line",
    sim_copy: "Copy",
    sim_new: "New (empty)",
    sim_delete: "Delete",
    sim_line: (name, conn) => `Active line “${name}”: ${conn || "no connection"}`,
    sim_line_uses: "uses this simulation",
    sim_builtin_note: "Template, cannot be changed. “Copy” it to adapt it.",
    sim_name: "Name",
    sim_col_pa: "PA",
    sim_col_id: "ID",
    sim_col_man: "Manufacturer",
    sim_col_medium: "Medium",
    sim_col_baud: "Baud rate",
    sim_col_fault: "Behaviour",
    sim_baud_bus: "like bus",
    sim_add: "+ Meter",
    sim_add_many: "+ several",
    sim_add_n: "Count",
    sim_save: "Save",
    sim_saved: (n) => "Simulation saved" + (n ? " · line reconnected" : ""),
    sim_used: (c) => `Active line connected to ${c}`,
    sim_copy_name: (n) => `Copy of ${n}`,
    sim_new_prompt: "Name of the new simulation:",
    sim_new_name: "My installation",
    confirm_sim_delete: (n) => `Delete simulation “${n}”?`,
    confirm_sim_discard: "Discard unsaved changes?",
    sim_empty: "No meters yet. Add some with “+ Meter” or “+ several”.",
    sim_count: (n, faults) => `${n} meters` + (faults ? ` · ${faults} with a fault` : " · all OK"),
    sim_dup_pa: (pa) => `PA ${pa} used more than once: the meters answer at the same time (collision)`,
    sim_dup_id: (id) => `ID ${id} used more than once: the secondary scan cannot tell them apart`,
    sim_pa0: (n) => `${n} meters with PA 0 (factory default): reachable only by secondary address`,
    sim_m_heat: "Heat",
    sim_m_cooling: "Cooling",
    sim_m_water: "Water",
    sim_m_cold_water: "Cold water",
    sim_m_warm_water: "Warm water",
    sim_m_gas: "Gas",
    sim_m_electricity: "Electricity",
    sim_m_heat_old: "Heat, old (fixed structure)",
    sim_f_none: "OK",
    sim_f_checksum: "wrong checksum",
    sim_f_truncated: "truncated telegram",
    sim_f_busy: "reports “busy”",
    sim_f_wrong_address: "answers with wrong address",
    sim_f_flaky: "loose contact",
    sim_f_too_many: "“too many” after 2 reads",
    help_toggle: "Show/hide explanations",
    conn_change: "Change",
    conn_summary: (p, b) => `Connected: ${p} · ${b} Bd`,
    gw_sum: (n, issues) => `${n} line${n === 1 ? "" : "s"}` + (issues ? ` · ⚠ ${issues}` : ""),
    project: "Project",
    crumb_title: "Project › gateway › line: switch here",
    project_switch: "Switch project",
    project_new: "+ New project",
    project_new_prompt: "Name of the new project (e.g. building or address):",
    project_edit: "Edit project",
    project_delete: "Delete project",
    confirm_project_delete: (n) => `Delete project “${n}” with all gateways, lines, meters and readings?\n\nThis cannot be undone. Tip: “Export” first.`,
    project_imported: (n) => `Project “${n}” imported`,
    gw_add: "+ Gateway",
    gw_new: (n) => `Gateway ${n}`,
    move_up: "Move up",
    move_down: "Move down",
    ln_add: "+ Line",
    ln_new: (n) => `Line ${n}`,
    ln_use: "Select",
    ln_active: "Active",
    ln_no_conn: "Connection not set yet",
    ln_info: (n, read) => `${n} meters` + (read ? ` · read ${read}` : ""),
    plan: "Read plan",
    plan_time: "Time",
    plan_opt_off: "Off (manual only)",
    plan_opt_monthly: "Monthly (on the 1st)",
    plan_opt_daily: "Daily",
    plan_opt_hourly: "Hourly",
    plan_opt_15min: "Every 15 min",
    plan_inherit_project: (x) => `Like project (${x})`,
    plan_inherit_line: (x) => `Like line (${x})`,
    plan_off: "off",
    plan_monthly: (time) => `monthly on the 1st, ${time}`,
    plan_daily: (time) => `daily ${time}`,
    plan_hourly: "hourly",
    plan_15min: "every 15 min",
    plan_hint_project:
      "Default for all lines of the project. Off by default: only what you switch on deliberately is read on schedule. Don't read battery meters more often than needed; once a day is usually enough.",
    plan_hint_line: "Applies to all meters of the line that have no plan of their own.",
    plan_hint_meter: "Only needed if this meter should be read differently from its line.",
    plan_next: (x) => `next ${x}`,
    plan_meters_own: (n) => `${n} meters with their own plan`,
    dp_title: "Datapoints",
    dp_mqtt_on: "MQTT on",
    dp_mqtt_off: "MQTT off",
    f_mqtt: "send via MQTT",
    f_topic: "Topic (after the base topic)",
    f_topic_ph: "suggested when switched on",
    mqtt_hint:
      "Passes the datapoints ticked “MQTT” to a broker (e.g. a building management system): one topic per datapoint, JSON with value, unit, time and quality, retained, QoS 1. If the broker is unreachable, mbusio collects the values for up to 7 days and delivers them later in the right order.",
    mqtt_enabled: "Enable MQTT",
    mqtt_host: "Broker (address)",
    mqtt_port: "Port",
    mqtt_insecure: "Do not verify the certificate (self-signed)",
    mqtt_user: "User",
    mqtt_password: "Password",
    mqtt_pass_saved: "saved (empty = keep)",
    mqtt_client: "Client ID",
    mqtt_base: "Base topic",
    mqtt_time: (t) => `Take the time from the control system (topic ${t})`,
    mqtt_ca: "Own CA certificate (PEM, optional)",
    mqtt_example: (topic) => `Example: ${topic} → {"value": 123456, "unit": "kWh", "time": "…Z", "quality": "good", …} · status: online/offline`,
    mqtt_off: "off",
    mqtt_connecting: "connecting …",
    mqtt_connected: "connected",
    mqtt_error: "disconnected",
    mqtt_state_off: "MQTT is off. Nothing is collected or sent.",
    mqtt_state_connected: (b, since, sent, q) => `Connected to ${b} since ${since} · ${sent} sent · ${q} queued`,
    mqtt_state_connecting: (b) => `Connecting to ${b} …`,
    mqtt_state_error: (b, e, q) => `No connection to ${b}: ${e} · ${q} messages waiting, will be delivered later`,
    mqtt_saved: "MQTT settings saved",
    mqtt_busbar: (q) => `MQTT disconnected` + (q ? ` · ${q} waiting` : ""),
    dp_hint:
      "The values that are passed on (once MQTT or Modbus is set up). Choose per meter with “Values” in the meter list. The key is based on the meaning of the value and stays stable, even if a meter changes the order after an update.",
    dp_empty: "No datapoints yet. In the meter list, click “Values” on a meter and choose.",
    dp_csv_export: "Export CSV",
    dp_csv_import: "Import CSV",
    dp_select_all: "Select all",
    th_no: "No",
    th_dp: "Datapoint",
    th_value: "Value",
    th_quality: "Quality",
    th_send: "Send",
    th_active: "Active",
    q_good: "good",
    q_stale: "stale",
    q_disturbed: "disturbed",
    q_invalid: "invalid",
    q_none: "no value",
    q_stale_title: "The last value is older than 3 terms of the read plan.",
    q_disturbed_title: "The meter does not answer.",
    q_decreased_title: "Meter reading decreased. Was the meter replaced? Then use “Edit → Meter replaced…” on the meter.",
    q_status_title: "The meter reports an error in its status.",
    send_every: "on every reading",
    send_change: "only on change",
    send_daily: "once a day",
    dp_selected: (n) => `${n} selected:`,
    dp_on: "Activate",
    dp_off: "Deactivate",
    dp_delete: "Delete",
    dp_send_to: "Send …",
    confirm_dp_delete: (n) => `Delete ${n} datapoints? Their numbers are never reused.`,
    dp_values_btn: "Values",
    dp_values_title: "Choose values as datapoints",
    dp_pick_title: (name) => `Datapoints: ${name}`,
    dp_pick_read: (ts) => `Values of the reading from ${ts}`,
    dp_pick_none: "No reading yet. Read the meter first, then its values can be chosen here.",
    dp_main: "main value",
    dp_missing: "missing in the last reading",
    dp_tpl: (medium) => `Same choice for all “${medium}” meters on this line`,
    dp_tpl_auto: "also for meters added later",
    dp_saved: (a, r, o) => `Saved: ${a} new, ${r} removed` + (o ? `, ${o} on other meters` : ""),
    dp_templates: "Templates:",
    dp_tpl_n: (n) => (n === 1 ? "1 value" : `${n} values`),
    dp_tpl_auto_short: "new meters too",
    dp_tpl_remove: "Remove template",
    confirm_tpl_remove: (m) => `Remove the template for “${m}”? Existing datapoints stay.`,
    dp_form_title: "Edit datapoint",
    f_unit: "Unit",
    f_factor: "Factor",
    f_factor_hint: "Shown value = meter value × factor, e.g. 0.001 for Wh → kWh. Sending takes effect once MQTT or Modbus is set up.",
    f_send: "Send",
    f_active: "Active (send)",
    dp_import_preview: (c, a, e) => `Preview: ${c} changes, ${a} new datapoints, ${e} errors`,
    dp_import_apply: "Apply",
    dp_import_cancel: "Discard",
    dp_import_done: "CSV applied",
    dp_import_nothing: "The file contains no changes.",
    dp_import_errors: "Nothing is applied because of the errors. Fix the file and import again.",
    dp_field_name: "Name",
    dp_field_unit: "Unit",
    dp_field_factor: "Factor",
    dp_field_send: "Send",
    dp_field_active: "Active",
    dp_new: "new",
    exchange_btn: "Meter replaced…",
    exchange_title: "New meter in the same place: readings continue seamlessly.",
    exchange_prompt: "Address of the new meter (primary address, ID or secondary address):",
    exchange_confirm: (o, n) =>
      `Replace meter ${o} with ${n}?\n\nmbusio reads the new meter and continues the readings of the datapoints seamlessly (old final value → new start value). This is listed under changes in the report.`,
    exchange_done: "Meter replaced",
    ln_dp: (n) => `${n} datapoints`,
    ln_dp_invalid: (n) => `${n} invalid`,
    thin: "History",
    thin_off: "Keep everything (default)",
    thin_days: (n) => `After ${n} days 1 value/day`,
    thin_hint: "Only needed with frequent plans. Without this setting mbusio never deletes readings.",
    disk_low: (p) => `Storage low: ${p} % free`,
    disk_full: (p) => `Storage full: readings are no longer stored (${p} % free)`,
    disk_title:
      "Free up space: export readings and delete old ones, or thin the history in the project. Reading and display continue.",
    plan_last: (ok, n) => `last ${ok}/${n} read`,
    plan_disturbed: (n) => `${n} disturbed`,
    plan_paused: (n) => `${n} suspended`,
    plan_load: (p) => `bus load ${p} %`,
    plan_load_title:
      "The planned reads need almost all the bus time of this line. Read less often or split the meters across lines.",
    plan_held: "paused, disconnected by hand",
    plan_held_title: "The read plan continues as soon as the line is connected again.",
    plan_offline: "not connected",
    plan_line_down: "Line disturbed: no meter answered",
    plan_running: (label, d, n) => `Plan ${label} reading · ${d}/${n}`,
    plan_saved_warn: (w) => `Saved. Note: ${w}`,
    plan_paused_disconnect: "Disconnected. The read plan of this line is paused until you connect again.",
    col_plan: "Plan",
    meter_disturbed: "disturbed",
    meter_disturbed_title: (n) => `${n} terms in a row not read. Still being retried.`,
    meter_paused: (x) => `suspended until ${x}`,
    meter_paused_title:
      "The meter refused (busy or too many readouts). Next attempt on the next day.",
    day_today: "today",
    day_tomorrow: "tomorrow",
    time_unsure: "Time not confirmed",
    time_unsure_title:
      "Neither a time server nor a clock module confirms the time. Values are still read, but marked as “time unsure”. Click to take the time from this device.",
    confirm_time: (x) => `Take the time from this device?\n\n${x}\n\nNew readings then count as reliably timed (until mbusio restarts).`,
    time_taken: (s) => (s ? `Time taken (correction ${s > 0 ? "+" : ""}${s} s)` : "Time confirmed"),
    reading_time_unsure: "time unsure",
    reading_time_unsure_title: "When this was read, mbusio's time was not confirmed; the timestamp may be wrong.",
    ln_issues: (n) => `${n} issue${n === 1 ? "" : "s"}`,
    ln_no_issues: "no issues",
    confirm_gw_delete: (n) => `Delete gateway “${n}” with all lines and meters?`,
    confirm_ln_delete: (n) => `Delete line “${n}” with all meters and readings?`,
    form_project: "Project",
    form_gateway: "Gateway",
    form_line: "Line",
    f_name: "Name",
    f_customer: "Customer",
    f_site: "Site / installation",
    f_technician: "Technician",
    f_order: "Order",
    f_remarks: "Remarks",
    f_address: "Address (e.g. IP)",
    f_note: "Note",
    f_connection: "Connection",
    f_connection_ph: "COM3, /dev/ttyUSB0, tcp://IP:port, SIMULATOR",
    conn_line: (l) => `This connection belongs to line “${l}” and is saved there.`,
    activated: (l) => `Line “${l}” active`,
    activate_failed: (l, e) => `Line “${l}” active, but not connected: ${e}`,
    confirm_cancel_scan: "Really cancel the scan?\n\nMeters found so far stay in the list.",
    confirm_cancel_read: "Really cancel reading?\n\nMeters already read keep their values.",
    idle_ready: (p, b) => `Ready · ${p} · ${b} Bd`,
    idle_off: "Not connected",
    idle_connect: "Connect",
    issues: (n) => `⚠ ${n} issue${n === 1 ? "" : "s"}`,
    issues_none: "✓ No issues",
    dup_pa: "Duplicate PA",
    other_baud: "Different baud rate",
    other_baud_title: (b, bus) => `Meter runs at ${b} Bd, the bus at ${bus} Bd. Recommendation: switch it to ${bus} Bd.`,
    fix_baud: "Change baud rate",
    config_baud_note: (b) => `Meter runs at ${b} Bd – commands are sent at ${b} Bd.`,
    remember_all: "Remember all",
    remember_via: "Remember by",
    mode_sa_title: "Secondary address: unique and stays the same when the PA is changed (recommended)",
    mode_pa_title: "Primary address: when a control system or gateway polls the meters by PA",
    confirm_mode: (m, n) => `Switch the line to “Remember by ${m}”?\n\nThe ${n} meters in the list are switched too, where unique.`,
    mode_done: (m, n) => `Remember by ${m} · ${n} meters switched`,
    mode_kept: (n) => `${n} could not be switched`,
    remember_skipped: (n) => `${n} not unique, not remembered (“Fix all” first)`,
    skip_pa_unknown: "Primary address unknown: cannot remember by PA",
    skip_pa_zero: "PA 0 is the factory default: “Change PA” first",
    skip_pa_dup: (pa) => `PA ${pa} is duplicated: “Change PA” first`,
    skip_sa_unknown: "Secondary address unknown: cannot remember by SA",
    skip_sa_dup: "Secondary address not unique (same ID and manufacturer): switch the line to “Remember by PA”",
    spot_pa: (id, a) => `Meter ${id} gets the free address ${a}. Now press “Set”.`,
    spot_baud: (id, from, to) => `Meter ${id} runs at ${from} Bd. Now press “Switch”, then it runs at ${to} Bd like the bus.`,
    spot_done_pa: (id, a) => `✓ Done: meter ${id} now has address ${a}.`,
    spot_done_baud: (id, b) => `✓ Done: meter ${id} now runs at ${b} Bd.`,
    back_to_hits: "← back to the hit list",
    remembered_all: (n, skipped) => `${n} meters added` + (skipped ? `, ${skipped} already in the list` : ""),
    dup_id: "Duplicate ID + manufacturer",
    dup_id_info: "ID shared",
    dup_id_info_title: "Not an error: IDs are only unique per manufacturer. mbusio tells the meters apart by the full secondary address.",
    dup_pa_title: (pa, n) => `${n} meters are on PA ${pa}. Readdress them by ID.`,
    dup_id_title: "Same ID and same manufacturer: cannot be told apart. The manufacturer has to reprogram the meter.",
    fix_pa: "Change PA",
    warn_title: "Issues",
    warn_dup_pa: (pa, b, ids) => `PA ${pa} (${b} Bd): ${ids.length} meters – ${ids.join(", ")}`,
    warn_dup_id: (id, n) => `ID ${id}: ${n} meters with the same ID and manufacturer – have the manufacturer reprogram them`,
    warn_collision_pa: (pa, b) => `PA ${pa} (${b} Bd): several meters answer at once – recommendation: secondary scan, then “Change PA”`,
    warn_collision_id: (id) => `ID ${id}: several meters with the same ID – find them one by one with the primary scan`,
    why: "Why?",
    history: "History",
    history_title: (n) => `History: ${n}`,
    history_series: "Value",
    history_few: "A history needs at least two readings. Press “Read all” again later.",
    history_none: "No reading stored yet.",
    history_first: "First",
    history_last: "Last",
    history_diff: "Difference",
    history_per_day: "per day",
    history_table: "Table",
    history_chart: "Chart",
    history_csv: "CSV",
    history_close: "Close",
    th_time: "Time",
    th_delta: "Δ to previous",
    chart_label: (n, v) => `History ${n}, last value ${v}`,
    sort_title: "Click to sort (ascending, descending, as scanned)",
    fix_all: "Fix all",
    plan_title: (n) => `Plan: ${n} change${n === 1 ? "" : "s"}`,
    plan_hint: "Untick everything that should not be changed. Every change is verified on the meter.",
    plan_manual: "To do by hand:",
    plan_manual_collision: (pa, b) => `PA ${pa} (${b} Bd): collision – run a secondary scan first to see the meters one by one`,
    plan_manual_nodata: (x) => `${x}: delivers no data – “Why?” shows the cause`,
    plan_manual_sameid: (id) => `ID ${id}: same ID and manufacturer – have the manufacturer reprogram them`,
    plan_manual_id_collision: (id) => `ID ${id}: several meters with this ID – the primary scan shows them one by one (same manufacturer: have them reprogrammed)`,
    plan_none: "Nothing can be fixed automatically.",
    plan_run: (n) => `Run (${n})`,
    plan_close: "Close",
    plan_busy_free: "Looking for free primary addresses …",
    plan_running: (k, n) => `Fixing ${k}/${n} …`,
    plan_done: (ok, n) => `${ok} of ${n} changes done`,
    plan_baud: (id, a, b) => `${id}: baud rate ${a} → ${b} Bd`,
    plan_pa: (id, a, b) => `${id}: primary address ${a} → ${b}`,
    change_pa: (id, a, b) => `${id}: primary address ${a} → ${b}`,
    change_baud: (id, a, b) => `${id}: baud rate ${a} → ${b} Bd`,
    why_tip: "What to do:",
    why_raw: "Received",
    why_unknown: ["No more specific cause known.", "Open the telegram log and look at the lines for this meter."],
    why_collision: ["Several meters answer at once on this address.", "Run a secondary scan, then “Change PA” on the duplicate meters."],
    why_checksum: ["The checksum is wrong: bytes were corrupted on the way.", "Check wiring, terminals and shielding. Try a slower baud rate (e.g. 2400 instead of 9600). Too many meters or too long a line for the level converter?"],
    why_stop: ["The stop byte is missing: the telegram is corrupted.", "Check wiring and shielding, try a slower baud rate."],
    why_garbled: ["Unreadable reply: noise or several meters at once.", "Use the secondary scan to check for several meters on the address. Otherwise check wiring."],
    why_truncated: ["The telegram is cut off: the rest did not arrive.", "Increase the timeout under Connection (e.g. 1000 ms) and reconnect. Otherwise check wiring."],
    why_no_reply: ["The meter answers but sends no data.", "Try again later. Some meters need time after power-up or have reading disabled (manufacturer docs)."],
    why_unexpected: ["The meter answers with an unexpected telegram.", "Have the telegram analysed. Possibly not a standard M-Bus meter."],
    why_wrong_address: (a) => [`The meter answers with address ${a} instead of the one asked.`, "The meter is misconfigured. Address it by secondary address and set a new PA with “Change PA”."],
    why_app_error: (code, text) => [`The meter itself reports an error: ${text} (code ${code}).`, code === "08" ? "Read it again later." : "Look up the error code in the manufacturer docs."],
    why_decode: ["The data cannot be read (unknown format).", "Have the telegram analysed. Possibly a manufacturer-specific format."],
    why_same_id: ["Several meters share this ID and cannot be told apart by ID alone.", "Find them one by one with the primary scan. Same manufacturer: have them reprogrammed."],
    app_errors: { "00": "unspecified", "01": "function not supported", "02": "buffer too long", "03": "too many records", "04": "premature end of record", "05": "more than 10 DIFEs", "06": "more than 10 VIFEs", "08": "meter busy", "09": "too many readouts" },
    start_secondary: "Start secondary scan",
    warn_other_baud: (n, b) => `${n} meter(s) not at the configured baud rate (${b} Bd) – recommendation: “Change baud rate”`,
    warn_fix_hint: "“Change PA” finds a free address on the bus and fills it in with the ID under “Configure meter”.",
    job_done: (k, n) => `${k} done · ${n} hits`,
    job_cancelled: (k, n) => `${k} cancelled · ${n} hits`,
    busy_read: "Reading meter …",
    busy_set: "Setting primary address …",
    busy_baud: "Switching baud rate …",
    busy_detect: "Detecting baud rate …",
    busy_reset: "Sending reset …",
    busy_raw: "Sending telegram …",
    busy_free: "Looking for a free primary address …",
    bauds_none: "None",
    bus_bauds: "Check",
    pick_baud: "Select at least one baud rate for the scan",
    estimate: (t) => `Primary scan takes at most about ${t}`,
    bus_bauds_busy: "Checking 300 to 38400 Bd …",
    bus_bauds_step: (b, k, n, found) => `Checking ${b} Bd … (${k}/${n})` + (found.length ? ` · answer at ${found.join(", ")} Bd` : ""),
    bus_bauds_checking: "Checking …",
    theme_auto: "Theme like system",
    theme_light: "Light",
    theme_dark: "Dark",
    bus_bauds_none: "No device answers at any baud rate",
    bus_bauds_found: (l, b) => `Devices answer at: ${l} Bd` + (l.split(", ").some((x) => Number(x) !== b) ? " – note: not only at the configured baud rate!" : ""),
    duration: (s) => (s < 90 ? `${Math.ceil(s)} s` : `${Math.ceil(s / 60)} min`),
    col_baud: "Baud",
    col_primary: "PA",
    col_secondary: "Secondary address",
    col_id: "ID",
    col_version: "Version",
    sa_raw: "Secondary address as hex (16 digits)",
    col_manufacturer: "Manufacturer",
    col_medium: "Medium",
    col_status: "Status",
    read_meter: "Read meter",
    address: "Address",
    address_ph: "Primary (0–250), ID (8) or secondary (16)",
    address_ph_short: "Primary, ID or secondary",
    read: "Read",
    reading_busy: "Reading …",
    set_primary: "Set primary address",
    new_primary: "New primary address",
    set: "Set",
    set_hint: "Give the meter by primary address, ID or secondary address. If several meters share an address (e.g. 0), use the ID.",
    configure: "Configure meter",
    meter: "Meter",
    baud_title: "Baud rate",
    new_baud: "New baud rate",
    switch: "Switch",
    detect_baud: "Detect baud rate",
    baud_hint: "“Detect” tries 300 to 38400 Bd. After switching, the meter only answers at the new baud rate; it is stored in the meter list.",
    confirm_switch: (a, b) => `Switch meter ${a} to ${b} Bd?`,
    baud_switched: (b) => `Meter now runs at ${b} Bd`,
    baud_switched_list: (b) => `Meter now runs at ${b} Bd, updated in the meter list`,
    detect_busy: "Searching …",
    detect_none: "No answer at any baud rate",
    detect_found: (l) => `Answers at: ${l} Bd`,
    reset_title: "Application reset",
    reset_hint: "Resets the meter's communication (e.g. telegram selection). Meter readings are kept.",
    reset: "Send reset",
    confirm_reset: (a) => `Send application reset to meter ${a}?`,
    reset_done: "Reset acknowledged",
    meters: "Meter list",
    meters_hint: "Remember meters, name them and read them all at once. The list is stored on the computer running mbusio.",
    meters_file: (p) => `File: ${p}`,
    meters_empty: "No meters yet. Click “Remember” in the scan list or “Add meter”.",
    meter_add: "Add meter",
    meters_read_all: "Read all",
    meters_clear: "Clear list",
    report: "Report (PDF)",
    report_title: "Report for the customer: print or save as PDF",
    confirm_meters_clear: (n) => `Delete all ${n} meters from the list?\n\nThis cannot be undone. Tip: “Export” first.`,
    meters_cleared: (n) => `${n} meters deleted`,
    export: "Export",
    import: "Import",
    meter_name: "Name",
    meter_name_ph: "e.g. Flat 3 hot water",
    meter_location: "Location",
    meter_location_ph: "e.g. basement left",
    meter_note: "Note",
    baud_bus: "same as bus",
    save: "Save",
    edit: "Edit",
    remember: "Remember",
    remembered: "Added to meter list",
    last_value: "Last value",
    meter_saved: "Saved",
    confirm_delete_meter: (n) => `Remove “${n}” from the meter list?`,
    imported: (n) => `${n} meters imported`,
    import_bad: "File does not contain a meter list",
    job_read_all: "Read all",
    read_all_done: "Meter list read",
    read_all_cancelled: "Reading the meter list cancelled",
    raw_title: "Analyse / send telegram",
    raw_hint: "Enter bytes or click a TX/RX line above. “Analyse” explains the telegram byte by byte, “Send” sends it unchanged to the bus (experts). Example: 10 40 01 41 16.",
    analyze: "Analyse",
    an_problems: "Problems",
    an_clean: "✓ Telegram is fine",
    an_bytes: "Bytes",
    an_field: "Field",
    an_meaning: "Meaning",
    log_click: "Click to analyse",
    why_analyze: "Analyse telegram",
    raw_bytes: "Bytes (hex)",
    send: "Send",
    raw_none: "No reply",
    raw_ack: "ACK (E5)",
    raw_collision: "Corrupted reply",
    raw_frame: "Telegram",
    readings_export: "Readings & export",
    readings_count: (n) => `reading${n === 1 ? "" : "s"} stored`,
    delete: "Delete",
    telegram_log: "Telegram log",
    clear_log: "Clear log",
    follow: "auto-scroll",
    connected_to: (p) => `Connected to ${p}`,
    job_primary: "Primary scan",
    job_secondary: "Secondary scan",
    found: (n) => `${n} found`,
    hits: (n) => `${n} hit${n === 1 ? "" : "s"}`,
    st_ok: "OK",
    st_collision: "Collision",
    st_no_data: "No data",
    meta_id: "ID",
    meta_manufacturer: "Manufacturer",
    meta_medium: "Medium",
    meta_version: "Version",
    meta_primary: "Primary address",
    meta_secondary: "Secondary address",
    meta_access: "Access no.",
    meta_status: "Status",
    meta_time: "Timestamp",
    th_desc: "Description",
    th_value: "Value",
    th_unit: "Unit",
    th_kind: "Type",
    storage: "Storage",
    tariff: "Tariff",
    subunit: "Subunit",
    enter_address: "Enter an address",
    enter_both: "Enter current and new address",
    confirm_set: (a, n) => `Change primary address from ${a} to ${n}?`,
    address_set: (n) => `New primary address ${n} set`,
    confirm_clear: "Delete all stored readings?",
    token_prompt: "Enter access token:",
  },
};

function storageGet(key) {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function storageSet(key, value) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* privater Modus */
  }
}

let lang = storageGet("lang") || (navigator.language || "de").slice(0, 2);
if (!UI[lang]) lang = "de";
let labels = {}; // Messdaten-Begriffe vom Server
let token = storageGet("token") || "";

function t(key, ...args) {
  const v = UI[lang][key] ?? UI.de[key] ?? key;
  return typeof v === "function" ? v(...args) : v;
}

/// Schlüssel vom Server übersetzen, z.B. "note_error_code:05"
function label(key) {
  if (!key) return "";
  const [base, arg] = key.split(":");
  const text = labels[base] ?? base;
  return arg ? `${text} ${arg}` : text;
}

async function setLang(l) {
  lang = l;
  storageSet("lang", l);
  document.documentElement.lang = l;
  document.querySelectorAll("[data-lang]").forEach((b) => b.classList.toggle("active", b.dataset.lang === l));
  document.querySelectorAll("[data-i18n]").forEach((el) => (el.textContent = t(el.dataset.i18n)));
  document.querySelectorAll("[data-i18n-ph]").forEach((el) => (el.placeholder = t(el.dataset.i18nPh)));
  document.querySelectorAll("[data-i18n-title]").forEach((el) => (el.title = t(el.dataset.i18nTitle)));
  labels = await api("labels?lang=" + l).catch(() => ({}));
  updateExportLinks();
  if (lastReading) renderReading(lastReading);
  if (lastJob) renderJob(lastJob);
  renderMeters();
  renderTree();
  renderHistory();
  renderBusbar();
  // Bezeichnungen der Datenpunkte kommen vom Server in der gewählten Sprache
  loadDatapoints().catch(() => {});
  if (picker) openPicker(picker.meter).catch(() => {});
  if (mqttConfig) updateMqttForm();
  if (sim) loadSim().catch(() => {});
  refreshStatus().catch(() => {});
}

document.querySelectorAll("[data-lang]").forEach((b) => (b.onclick = () => setLang(b.dataset.lang)));


// Design: auto (wie System), hell oder dunkel; das Setzen beim Laden steht in index.html
function setTheme(theme) {
  if (theme === "auto") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = theme;
  storageSet("theme", theme);
  document.querySelectorAll("[data-theme-set]").forEach((b) => b.classList.toggle("active", b.dataset.themeSet === theme));
}
document.querySelectorAll("[data-theme-set]").forEach((b) => (b.onclick = () => setTheme(b.dataset.themeSet)));
setTheme(["light", "dark"].includes(storageGet("theme")) ? storageGet("theme") : "auto");

// ------------------------------------------------------------ Helfer

const $ = (id) => document.getElementById(id);
let connected = false;
let jobRunning = false;
let logSeq = 0;
let lastReading = null;
let lastJob = null;
/// Art des zuletzt angezeigten Jobs (auch "Alle auslesen"), z.B. für die Abbruch-Frage
let currentJobKind = null;
let connBaud = null;
let activeLineId = null;
/// Verbindungsformular trotz Verbindung offen ("Ändern")
let connEditing = false;
/// laufende geplante Auslesungen, Zustand der Uhrzeit, irgendwo ein Plan eingeschaltet
let planRuns = [];
let timeState = null;
let planActive = false;
let disk = null;
let mqttStatus = null;
/// zuletzt gespeicherte Auffälligkeiten (nur bei Änderung neu speichern)
let savedIssues = null;

function saveIssues(items) {
  if (!activeLineId) return;
  const body = { count: items.length, items };
  const text = JSON.stringify(body);
  if (text === savedIssues) return;
  savedIssues = text;
  request(`lines/${activeLineId}/issues`, { method: "PUT", body })
    .then(() => tree && loadTree())
    .catch(() => {});
}
let checking = false;
let conn = null;
let baudsTouched = false;
const BAUDS = [300, 600, 1200, 2400, 4800, 9600, 19200, 38400];

/// Anfragen, die den Bus belegen: Pfad, Text, Bereich für "Ansehen"
const BUS_ACTIONS = [
  [/^read$|^meters\/\d+\/read$/, "busy_read", "#sec-read"],
  [/^set-address$/, "busy_set", "#sec-config"],
  [/^switch-baud$/, "busy_baud", "#sec-config"],
  [/^detect-baud$/, "busy_detect", "#sec-config"],
  [/^reset$/, "busy_reset", "#sec-config"],
  [/^raw$/, "busy_raw", "#sec-log"],
  [/^free-address$/, "busy_free", "#sec-config"],
];

async function api(path, options = {}) {
  // "Alle beheben" zeigt seinen eigenen Fortschritt in der Leiste
  const busy = !planRunning && options.method === "POST" && BUS_ACTIONS.find(([re]) => re.test(path));
  if (!busy) return request(path, options);
  busbar({ text: t(busy[1]), link: busy[2] });
  try {
    return await request(path, options);
  } finally {
    busbar(null);
  }
}

async function request(path, options = {}) {
  const headers = { "Content-Type": "application/json", "X-Lang": lang };
  if (token) headers["X-Token"] = token;
  const opts = { ...options, headers };
  if (opts.body && typeof opts.body !== "string") opts.body = JSON.stringify(opts.body);
  const res = await fetch("api/" + path, opts);
  if (res.status === 401 && !options.retried) {
    const entered = prompt(t("token_prompt"));
    if (entered) {
      token = entered.trim();
      storageSet("token", token);
      updateExportLinks();
      return request(path, { ...options, retried: true });
    }
  }
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data.error || res.statusText);
  return data;
}

function updateExportLinks() {
  const extra = `&lang=${lang}` + (token ? `&token=${encodeURIComponent(token)}` : "");
  $("export-csv").href = "api/export?format=csv" + extra;
  $("export-json").href = "api/export?format=json" + extra;
  $("meters-export").href = "api/meters/export" + (token ? `?token=${encodeURIComponent(token)}` : "");
}

function toast(msg, isError = false) {
  const el = $("toast");
  el.textContent = msg;
  el.className = isError ? "error" : "";
  el.hidden = false;
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => (el.hidden = true), isError ? 6000 : 3000);
}

function esc(s) {
  return String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
}

async function guarded(button, fn) {
  button.disabled = true;
  try {
    await fn();
  } catch (e) {
    toast(e.message, true);
  } finally {
    // Knöpfe ohne Verbindungsbedarf wieder freigeben; die anderen regelt updateButtons
    button.disabled = false;
    updateButtons();
  }
}

function updateButtons() {
  document.querySelectorAll(".needs-conn").forEach((b) => (b.disabled = !connected || jobRunning || checking));
  // ohne markierte Baudrate gibt es nichts zu prüfen oder zu scannen
  const none = !$("scan-bauds").querySelector("input:checked");
  $("bauds-empty").hidden = !none;
  if (none) ["scan-primary", "scan-secondary"].forEach((id) => ($(id).disabled = true));
  const all = !$("scan-bauds").querySelector("input:not(:checked)");
  $("bauds-all").textContent = t(all ? "bauds_none" : "bauds_all");
  $("bauds-all").disabled = checking || jobRunning;
  $("connect").disabled = jobRunning;
  $("disconnect").disabled = !connected || jobRunning;
  $("connect").hidden = connected;
  $("disconnect").hidden = !connected;
  $("scan-cancel").hidden = !jobRunning;
}

// ------------------------------------------------------------ Verbindung

async function loadPorts() {
  const ports = await api("ports");
  const sel = $("port");
  const prev = sel.value;
  sel.innerHTML = ports.map((p) => `<option>${esc(p)}</option>`).join("");
  if (ports.includes(prev)) sel.value = prev;
}

async function refreshStatus() {
  const s = await api("status");
  connected = s.connected;
  const prevBaud = connBaud;
  connBaud = s.connection?.baud ?? null;
  conn = s.connection;
  activeLineId = s.active?.line.id ?? null;
  // Termin fertig geworden: Zählerliste und Projekt-Karte zeigen das neue Ergebnis
  const runsBefore = planRuns.length;
  planRuns = s.plans || [];
  timeState = s.time;
  planActive = !!s.plan_active;
  disk = s.disk;
  mqttStatus = s.mqtt;
  renderMqttState();
  if (runsBefore && !planRuns.length) loadMeters().catch(() => {});
  // verbunden: Verbindung als eine Zeile, Formular erst auf "Ändern"
  $("sec-conn").classList.toggle("compact", connected && !connEditing);
  $("conn-summary").hidden = !connected || connEditing;
  if (connected) $("conn-summary-text").textContent = t("conn_summary", s.connection.port, s.connection.baud);
  // "Baudrate abweichend" hängt von der Bus-Baudrate ab: Trefferliste neu zeichnen, wenn sie sich ändert
  if (connBaud !== prevBaud && lastJob) renderJob(lastJob);
  renderBusbar();
  // Auswahl folgt der eingestellten Baudrate, bis man selbst etwas anklickt
  if (connBaud && connBaud !== prevBaud && !baudsTouched) setPickedBauds([connBaud]);
  updateEstimate();
  jobRunning = s.job_running;
  const badge = $("conn-badge");
  const c = s.connection;
  badge.className = connected ? "chip on" : "chip off";
  badge.querySelector(".chip-text").textContent = connected ? `${c.port} · ${c.baud} Bd` : t("disconnected");
  $("readings-count").innerHTML = `<b>${s.readings}</b>${esc(t("readings_count", s.readings))}`;
  $("meters-file").textContent = s.meters_file ? t("meters_file", s.meters_file) : "";
  updateButtons();
  if (jobRunning) pollJob();
}

$("connect").onclick = () =>
  guarded($("connect"), async () => {
    const port = $("port-manual").value.trim() || $("port").value;
    await api("connect", {
      method: "POST",
      body: { port, baud: Number($("baud").value), timeout_ms: Number($("timeout").value) || 0 },
    });
    toast(t("connected_to", port));
    connEditing = false;
    await refreshStatus();
  });

$("disconnect").onclick = () =>
  guarded($("disconnect"), async () => {
    const r = await api("disconnect", { method: "POST" });
    if (r.plan_paused) toast(t("plan_paused_disconnect"));
    await refreshStatus();
    loadTree().catch(() => {});
  });

$("busbar-time").onclick = () => {
  const now = new Date();
  if (!confirm(t("confirm_time", now.toLocaleString(lang === "de" ? "de-CH" : "en-GB")))) return;
  guarded($("busbar-time"), async () => {
    const r = await api("time", { method: "POST", body: { epoch_ms: Date.now() } });
    toast(t("time_taken", Math.round(r.offset_s)));
    await refreshStatus();
  });
};

$("refresh-ports").onclick = () => loadPorts().catch((e) => toast(e.message, true));

// ------------------------------------------------------------ Scan

let polling = false;
let renderedJob = null;
let renderedHits = 0;

// ------------------------------------------------------------ Leiste unten

let busbarTimer = null;
/// laufende Aktion oder kurz stehende Fertigmeldung; null = Ruhezustand
let busbarState = null;
/// Anzahl Auffälligkeiten der letzten Suche; null = noch keine Suche
let issueCount = null;

/// `null` = Ruhezustand; sonst { text, frac (0..1, ohne = unbestimmt), link, cancel, done, error }
function busbar(state) {
  clearTimeout(busbarTimer);
  busbarState = state;
  renderBusbar();
  // fertig: noch kurz stehen lassen, dann zurück in den Ruhezustand
  if (state?.done) busbarTimer = setTimeout(() => busbar(null), state.error ? 10000 : 6000);
}

/// Die Leiste ist immer da: im Ruhezustand Verbindung bzw. "Nicht verbunden"
function renderBusbar() {
  const el = $("busbar");
  const run = planRuns[0];
  const state =
    busbarState ||
    (run
      ? { text: t("plan_running", run.label, run.done, run.total), frac: run.total ? run.done / run.total : 0, idle: true }
      : {
          text: connected && conn ? t("idle_ready", conn.port, conn.baud) : t("idle_off"),
          idle: true,
          off: !connected,
        });
  // Uhrzeit nur ein Thema, wenn geplant ausgelesen wird
  const tm = $("busbar-time");
  tm.hidden = !(planActive && timeState && !timeState.confirmed);
  // Speicher: "knapp" nur im Dauerbetrieb wichtig, "voll" immer
  const mq = $("busbar-mqtt");
  mq.hidden = mqttStatus?.state !== "error";
  if (!mq.hidden) mq.textContent = t("mqtt_busbar", mqttStatus.queued);
  const dk = $("busbar-disk");
  const level = disk?.level || "ok";
  dk.hidden = level === "ok" || (level === "low" && !planActive);
  if (!dk.hidden) {
    const pct = Math.round(disk.free_pct);
    dk.textContent = level === "full" ? t("disk_full", pct) : t("disk_low", pct);
    dk.classList.toggle("full", level === "full");
  }
  el.hidden = false;
  document.body.classList.add("has-busbar");
  el.classList.toggle("idle", !!state.idle);
  el.classList.toggle("off", !!state.off);
  el.classList.toggle("done", !!state.done && !state.error);
  el.classList.toggle("error", !!state.error);
  $("busbar-text").textContent = state.text;
  const bar = el.querySelector(".busbar-progress");
  bar.hidden = state.frac === undefined;
  if (state.frac !== undefined) bar.querySelector(".bar-fill").style.transform = `scaleX(${state.frac})`;
  const link = $("busbar-link");
  link.hidden = state.idle && !state.off;
  link.href = state.off ? "#sec-conn" : state.link || "#sec-scan";
  link.textContent = t(state.off ? "idle_connect" : "busbar_view");
  $("busbar-cancel").hidden = !state.cancel;
  const issues = $("busbar-issues");
  issues.hidden = issueCount === null;
  issues.className = "busbar-issues " + (issueCount ? "warn" : "ok");
  issues.textContent = issueCount ? t("issues", issueCount) : t("issues_none");
  issues.href = issueCount ? "#hits-warn" : "#hits";
}

let cancelRequested = false;

function cancelJob() {
  const kind = currentJobKind;
  if (!confirm(t(kind === "read_all" ? "confirm_cancel_read" : "confirm_cancel_scan"))) return;
  cancelRequested = true;
  api("job/cancel", { method: "POST" }).catch((e) => toast(e.message, true));
}

$("busbar-cancel").onclick = cancelJob;

function jobLink(job) {
  return job.kind === "read_all" ? "#sec-meters" : "#sec-scan";
}

async function pollJob() {
  if (polling) return;
  polling = true;
  try {
    for (;;) {
      const job = await api("job");
      renderJob(job);
      // Zählerliste während "Alle auslesen" mitlaufend aktualisieren
      if (job.kind === "read_all") await loadMeters().catch(() => {});
      if (!job.running) {
        jobRunning = false;
        updateButtons();
        const cancelled = cancelRequested;
        cancelRequested = false;
        const done =
          job.kind === "read_all"
            ? t(cancelled ? "read_all_cancelled" : "read_all_done")
            : t(cancelled ? "job_cancelled" : "job_done", t("job_" + job.kind), job.hits.length);
        busbar({
          text: job.error || done,
          frac: 1,
          link: jobLink(job),
          done: true,
          error: !!job.error,
        });
        // Erfolg meldet die Leiste unten, nur Fehler zusätzlich als Toast
        if (job.error) toast(job.error, true);
        break;
      }
      await new Promise((r) => setTimeout(r, 600));
    }
  } catch (e) {
    toast(e.message, true);
  } finally {
    polling = false;
    refreshStatus().catch(() => {});
  }
}

/// Ursache eines Treffers in einfachen Worten: [was ist los, was tun]
function explainReason(reason) {
  const [key, arg] = (reason || "").split(":");
  switch (key) {
    case "wrong_address":
      return t("why_wrong_address", arg);
    case "app_error":
      return t("why_app_error", arg, t("app_errors")[arg] || "?");
    case "collision":
    case "checksum":
    case "stop":
    case "garbled":
    case "truncated":
    case "no_reply":
    case "unexpected":
    case "decode":
    case "same_id":
      return t("why_" + key);
    default:
      return t("why_unknown");
  }
}

/// geöffnete "Warum?"-Zeilen, bleiben beim Neuzeichnen offen
const openWhy = new Set();
const hitKey = (h) => `${h.baud}:${h.primary_address}:${h.secondary_address}`;

function whyRow(h, cols) {
  const [what, tip] = explainReason(h.reason);
  return `<tr class="why-row"><td colspan="${cols}"><div class="why">
    <p><b>${esc(what)}</b></p>
    <p><span class="why-label">${esc(t("why_tip"))}</span> ${esc(tip)}</p>
    ${h.raw ? `<p class="why-raw"><span class="why-label">${esc(t("why_raw"))}:</span> <code>${esc(h.raw)}</code><a href="#analysis" class="analyze-link" data-analyze="${esc(h.raw)}">${esc(t("why_analyze"))} →</a></p>` : ""}
  </div></td></tr>`;
}

/// Auffälligkeiten: doppelte Primäradressen (je Baudrate), doppelte IDs, fremde Baudraten
function analyzeHits(hits) {
  const group = (key) => {
    const m = new Map();
    hits.forEach((h, i) => {
      const k = key(h);
      if (k !== null) m.set(k, [...(m.get(k) || []), i]);
    });
    return new Map([...m].filter(([, list]) => list.length > 1));
  };
  // Kollisionen aus dem Primärscan sind schon eine Zeile für sich, die zählen nicht doppelt
  const dupPa = group((h) => (h.primary_address == null || h.status === "collision" ? null : `${h.baud}:${h.primary_address}`));
  const dupId = group((h) => (h.id && /^[0-9]{8}$/.test(h.id) ? h.id : null));
  const rowDupPa = new Set([...dupPa.values()].flat());
  // gleiche ID ist nur ein Problem, wenn auch der Hersteller gleich ist (IDs sind je Hersteller eindeutig)
  const dupIdSame = new Map(
    [...dupId].filter(([, list]) => new Set(list.map((i) => hits[i].manufacturer)).size < list.length)
  );
  const rowDupId = new Set([...dupId.values()].flat());
  const rowDupIdSame = new Set([...dupIdSame.values()].flat());
  const otherBaud = hits.filter((h) => h.baud && h.baud !== connBaud).length;
  return { dupPa, dupId, dupIdSame, rowDupPa, rowDupId, rowDupIdSame, otherBaud };
}

function renderHitsWarn(hits, a) {
  const items = [];
  for (const [k, list] of a.dupPa) {
    const [baud, pa] = k.split(":");
    items.push(t("warn_dup_pa", pa, baud, list.map((i) => hits[i].id || "?")));
  }
  for (const [id, list] of a.dupIdSame) items.push(t("warn_dup_id", id, list.length));
  const collisions = hits.filter((h) => h.status === "collision");
  for (const h of collisions) {
    if (h.primary_address != null) items.push(t("warn_collision_pa", h.primary_address, h.baud));
    else if (h.id) items.push(t("warn_collision_id", h.id));
  }
  // Zähler ohne Daten einzeln, mit Ursache (auch für den Bericht verständlich)
  for (const h of hits.filter((x) => x.status === "no_data")) {
    const name = h.id || `PA ${h.primary_address}`;
    items.push(`${name}: ${explainReason(h.reason)[0]}`);
  }
  if (a.otherBaud) items.push(t("warn_other_baud", a.otherBaud, connBaud));
  const el = $("hits-warn");
  el.hidden = items.length === 0;
  issueCount = items.length;
  // erst wenn der Scan durch ist: bei der Linie merken (Projekt-Karte, Bericht)
  if (!lastJob?.running) saveIssues(items);
  renderBusbar();
  el.innerHTML = items.length
    ? `<b>⚠ ${esc(t("warn_title"))}</b><ul>${items.map((x) => `<li>${esc(x)}</li>`).join("")}</ul>` +
      (a.dupPa.size ? `<p class="hint">${esc(t("warn_fix_hint"))}</p>` : "") +
      (planSteps(hits, a).steps.length
        ? `<button class="small cta needs-conn" data-fix-all>${esc(t("fix_all"))}</button> `
        : "") +
      // Kollision im Primärscan: der Sekundärscan findet die Zähler einzeln
      (collisions.some((h) => h.primary_address != null)
        ? `<button class="small cta needs-conn" data-start-secondary>${esc(t("start_secondary"))}</button>`
        : "")
    : "";
}

function renderJob(job) {
  currentJobKind = job.kind;
  // lastJob ist die Trefferliste; "Alle auslesen" darf sie nicht ersetzen
  if (job.kind !== "read_all") lastJob = job;
  if (job.running) {
    const frac = job.total ? Math.min(job.done / job.total, 1) : 0;
    const found = job.kind === "read_all" ? "" : ` · ${t("found", job.hits.length)}`;
    busbar({ text: `${t("job_" + job.kind)} · ${job.message}${found}`, frac, link: jobLink(job), cancel: true });
  }
  if (job.kind === "read_all") {
    const el = $("meters-job");
    el.hidden = false;
    el.textContent = job.running
      ? `${t("job_read_all")}: ${Math.min(job.done + 1, job.total)}/${job.total} · ${job.message}`
      : `${t("job_read_all")}: ${job.error || job.message}`;
    updateButtons();
    return;
  }
  $("job").hidden = false;
  const p = $("job-progress");
  const frac = job.total ? Math.min(job.done / job.total, 1) : 0;
  p.querySelector(".bar-fill").style.transform = `scaleX(${job.running ? frac : 1})`;
  p.classList.toggle("running", job.running);
  p.setAttribute("aria-valuenow", Math.round(frac * 100));
  $("job-msg").innerHTML = `<b>${esc(t("job_" + job.kind))}</b>${esc(job.message)} · ${esc(t("found", job.hits.length))}`;
  // nur neu hinzugekommene Treffer einblenden, nicht bei jeder Abfrage alle
  const fresh = job.kind === renderedJob && job.hits.length >= renderedHits ? renderedHits : 0;
  renderedJob = job.kind;
  renderedHits = job.hits.length;
  const table = $("hits");
  table.hidden = job.hits.length === 0;
  const issues = analyzeHits(job.hits);
  renderHitsWarn(job.hits, issues);
  document.querySelector(".hits-opt").hidden = table.hidden;
  // Sekundäradresse lesbar zerlegt (ID, Version) oder roh als 16 Hex-Zeichen
  const raw = $("sa-raw").checked;
  const cols = raw
    ? ["col_primary", "col_secondary", "col_manufacturer", "col_medium", "col_status", ""]
    : ["col_primary", "col_id", "col_manufacturer", "col_version", "col_medium", "col_status", ""];
  cols.splice(1, 0, "col_baud");
  const numCols = ["col_primary", "col_baud", "col_version"];
  table.querySelector("thead").innerHTML = `<tr>${cols
    .map((c) => {
      if (!c) return "<th></th>";
      const on = hitSort?.col === c;
      const cls = [numCols.includes(c) ? "num" : "", "sortable", on ? "sorted" : ""].filter(Boolean).join(" ");
      const aria = on ? ` aria-sort="${hitSort.dir > 0 ? "ascending" : "descending"}"` : "";
      return `<th class="${cls}" data-sort="${c}" title="${esc(t("sort_title"))}"${aria}>${t(c)}<span class="sort-ind">${
        on ? (hitSort.dir > 0 ? "▲" : "▼") : ""
      }</span></th>`;
    })
    .join("")}</tr>`;
  table.querySelector("tbody").innerHTML = sortedHits(job.hits)
    .map(([h, i]) => {
      const target = h.status === "ok" ? hitAddress(h) : "";
      const sa = raw
        ? `<td class="mono">${esc(h.secondary_address ?? "–")}</td>
        <td>${esc(h.manufacturer ?? "")}</td>`
        : `<td class="mono">${esc(h.id ?? "–")}</td>
        <td>${esc(h.manufacturer ?? "")}</td>
        <td class="num mono">${h.version ?? ""}</td>`;
      const other = h.baud && h.baud !== connBaud;
      const anim = i >= fresh ? ` class="new" style="animation-delay:${Math.min(i - fresh, 8) * 50}ms"` : "";
      const dupPa = issues.rowDupPa.has(i);
      const dupId = issues.rowDupId.has(i);
      const paCount = dupPa ? issues.dupPa.get(`${h.baud}:${h.primary_address}`).length : 0;
      const warnChips =
        (dupPa ? ` <span class="chip st-collision" title="${esc(t("dup_pa_title", h.primary_address, paCount))}">${t("dup_pa")}</span>` : "") +
        (dupId
          ? issues.rowDupIdSame.has(i)
            ? ` <span class="chip st-collision" title="${esc(t("dup_id_title"))}">${t("dup_id")}</span>`
            : ` <span class="chip st-no_data" title="${esc(t("dup_id_info_title"))}">${t("dup_id_info")}</span>`
          : "");
      // PA ändern: lieber über die ID, bei doppelter ID über die volle Sekundäradresse
      const fixTarget = dupId ? h.secondary_address : h.id || h.secondary_address;
      const fix =
        dupPa && fixTarget
          ? `<button class="small ghost needs-conn" data-fix="${esc(fixTarget)}" data-from="${other ? h.baud : ""}">${t("fix_pa")}</button>`
          : "";
      // Empfehlung: Zähler auf die Baudrate des Busses umstellen
      const fixBaud =
        other && h.status === "ok" && fixTarget
          ? `<button class="small ghost needs-conn" data-fixbaud="${esc(fixTarget)}" data-from="${h.baud}">${t("fix_baud")}</button>`
          : "";
      const otherChip = other && h.status === "ok"
        ? ` <span class="chip st-collision" title="${esc(t("other_baud_title", h.baud, connBaud))}">${t("other_baud")}</span>`
        : "";
      return `<tr${anim}>
        <td class="num mono${dupPa ? " st-other" : ""}"${dupPa ? ` title="${esc(t("dup_pa_title", h.primary_address, paCount))}"` : ""}>${h.primary_address ?? "–"}${dupPa ? " ⚠" : ""}</td>
        <td class="num mono${other ? " st-other" : ""}">${h.baud ?? ""}</td>
        ${sa}
        <td>${esc(label(h.medium))}</td>
        <td><span class="chip st-${h.status}">${t("st_" + h.status)}</span>${warnChips}${otherChip}</td>
        <td><div class="actions">${
          h.status !== "ok" ? `<button class="small ghost" data-why="${esc(hitKey(h))}">${t("why")}</button>` : ""
        }${fix}${fixBaud}${
          target !== ""
            ? `<button class="small quiet needs-conn" data-read="${esc(target)}" data-baud="${h.baud ?? ""}">${t("read")}</button>
               <button class="small ghost" data-remember="${i}">${t("remember")}</button>`
            : ""
        }</div></td>
      </tr>${openWhy.has(hitKey(h)) ? whyRow(h, cols.length) : ""}`;
    })
    .join("");
  updateButtons();
}

/// Eintrag für die Zählerliste aus einem Treffer
/// Adresse zum Ansprechen eines Treffers: volle Sekundäradresse, ausser sie ist mehrdeutig
/// (gleiche ID und gleicher Hersteller), dann die Primäradresse
function hitAddress(h) {
  const same = (lastJob?.hits || []).filter((x) => x.secondary_address && x.secondary_address === h.secondary_address);
  if (h.secondary_address && (same.length < 2 || h.primary_address == null)) return h.secondary_address;
  return String(h.primary_address);
}

/// Wie die aktive Linie Zähler merkt: "secondary" (SA) oder "primary" (PA)
const addressing = () => tree?.active.line.addressing || "secondary";

/// Adresse zum Merken nach der Einstellung der Linie; null + Grund, wenn sie nicht eindeutig ist
function rememberAddress(h) {
  const hits = lastJob?.hits || [];
  if (addressing() === "primary") {
    const pa = h.primary_address;
    if (pa == null) return { reason: t("skip_pa_unknown") };
    if (pa === 0) return { reason: t("skip_pa_zero") };
    if (hits.filter((x) => x.status === "ok" && x.baud === h.baud && x.primary_address === pa).length > 1)
      return { reason: t("skip_pa_dup", pa) };
    return { address: String(pa) };
  }
  // Platzhalter (F) z.B. bei alten Zählern ohne Herstellerkennung: geht, solange eindeutig
  const sa = h.secondary_address;
  if (!sa || /^F{8}/.test(sa)) return { reason: t("skip_sa_unknown") };
  if (hits.filter((x) => x.secondary_address === sa).length > 1) return { reason: t("skip_sa_dup") };
  return { address: sa };
}

function meterFromHit(h) {
  return {
    address: rememberAddress(h).address,
    secondary_address: h.secondary_address || null,
    primary_address: h.primary_address,
    // eigene Baudrate nur merken, wenn sie vom Bus abweicht
    baud: h.baud && h.baud !== connBaud ? h.baud : null,
    meter_id: h.id,
    manufacturer: h.manufacturer || null,
    medium: h.medium,
  };
}

$("hits-warn").addEventListener("click", (ev) => {
  if (ev.target.closest("[data-start-secondary]")) $("scan-secondary").click();
  if (ev.target.closest("[data-fix-all]")) guarded(ev.target.closest("button"), openPlan);
});

// ------------------------------------------------------------ Alle beheben

let plan = null;
let planRunning = false;

/// Was sich automatisch beheben lässt (Schritte) und was von Hand (Texte)
function planSteps(hits, a) {
  const steps = [];
  const manual = [];
  const name = (h) => h.id || h.secondary_address || `PA ${h.primary_address}`;
  // eindeutig ansprechbar nur über die volle Sekundäradresse, sofern sie nicht mehrfach vorkommt
  const target = (h) =>
    h.secondary_address && hits.filter((x) => x.secondary_address === h.secondary_address).length === 1
      ? h.secondary_address
      : null;
  for (const h of hits) {
    if (h.status === "ok" && h.baud && h.baud !== connBaud && target(h)) {
      steps.push({ kind: "baud", target: target(h), name: name(h), from: h.baud, to: connBaud });
    }
  }
  for (const [k, list] of a.dupPa) {
    const pa = Number(k.split(":")[1]);
    const members = list.map((i) => hits[i]).sort((x, y) => String(x.id).localeCompare(String(y.id)));
    // PA 0 ist der Werkszustand: alle umziehen; sonst darf der erste bleiben
    for (const h of pa === 0 ? members : members.slice(1)) {
      if (!target(h)) continue;
      steps.push({ kind: "pa", target: target(h), name: name(h), from: pa, to: null, fromBaud: h.baud !== connBaud ? h.baud : null });
    }
  }
  for (const h of hits) {
    if (h.status === "collision" && h.primary_address != null) manual.push(t("plan_manual_collision", h.primary_address, h.baud));
    // Sekundärscan: gleiche ID, über die ID allein nicht trennbar
    if (h.status === "collision" && h.primary_address == null && h.id) manual.push(t("plan_manual_id_collision", h.id));
    if (h.status === "no_data") manual.push(t("plan_manual_nodata", name(h)));
  }
  for (const id of a.dupIdSame.keys()) manual.push(t("plan_manual_sameid", id));
  return { steps, manual };
}

async function openPlan() {
  const hits = lastJob.hits;
  plan = planSteps(hits, analyzeHits(hits));
  // freie Adressen vorher suchen, damit der Plan konkrete Zahlen zeigt
  const skip = new Set(hits.map((h) => h.primary_address).filter((x) => x != null && x > 0));
  const paSteps = plan.steps.filter((s) => s.kind === "pa");
  if (paSteps.length) busbar({ text: t("plan_busy_free"), link: "#fix-plan" });
  try {
    for (const s of paSteps) {
      const r = await request("free-address", { method: "POST", body: { from: 1, skip: [...skip] } });
      s.to = r.address;
      skip.add(r.address);
    }
  } finally {
    if (paSteps.length) busbar(null);
  }
  renderPlan();
  $("fix-plan").scrollIntoView({ behavior: "smooth", block: "center" });
}

function renderPlan() {
  const el = $("fix-plan");
  if (!plan) {
    el.hidden = true;
    return;
  }
  el.hidden = false;
  const text = (s) => (s.kind === "baud" ? t("plan_baud", s.name, s.from, s.to) : t("plan_pa", s.name, s.from, s.to));
  const checked = plan.steps.filter((s) => !s.skip).length;
  // fertig erst, wenn die Ausführung durch ist (nicht schon beim ersten laufenden Schritt)
  const finished = !planRunning && plan.steps.some((s) => s.state);
  el.innerHTML = `
    <h3>${esc(plan.steps.length ? t("plan_title", plan.steps.length) : t("plan_none"))}</h3>
    ${plan.steps.length && !finished ? `<p class="hint">${esc(t("plan_hint"))}</p>` : ""}
    <ul>${plan.steps
      .map(
        (s, k) => `<li><label><input type="checkbox" data-step="${k}" ${s.skip ? "" : "checked"} ${planRunning || finished ? "disabled" : ""}> ${esc(text(s))}</label>
        <span class="plan-st ${s.state === "ok" ? "ok" : s.state === "err" ? "err" : ""}">${
          s.state === "ok" ? "✓" : s.state === "err" ? "✗ " + esc(s.error) : s.state === "run" ? "…" : ""
        }</span></li>`
      )
      .join("")}</ul>
    ${plan.manual.length ? `<p class="hint"><b>${esc(t("plan_manual"))}</b></p><ul class="manual">${plan.manual.map((m) => `<li>${esc(m)}</li>`).join("")}</ul>` : ""}
    <div class="row">
      ${!finished && !planRunning && plan.steps.length ? `<button class="cta needs-conn" data-plan-run ${checked ? "" : "disabled"}>${esc(t("plan_run", checked))}</button>` : ""}
      ${finished ? `<span class="hint result">${esc(t("plan_done", plan.steps.filter((s) => s.state === "ok").length, plan.steps.filter((s) => s.state).length))}</span>` : ""}
      <button class="quiet" data-plan-close ${planRunning ? "disabled" : ""}>${esc(t(finished ? "plan_close" : "cancel"))}</button>
    </div>`;
  updateButtons();
}

$("fix-plan").addEventListener("change", (ev) => {
  const k = ev.target.dataset.step;
  if (k === undefined) return;
  plan.steps[Number(k)].skip = !ev.target.checked;
  renderPlan();
});

$("fix-plan").addEventListener("click", (ev) => {
  if (ev.target.closest("[data-plan-close]")) {
    plan = null;
    renderPlan();
  } else if (ev.target.closest("[data-plan-run]")) {
    runPlan();
  }
});

async function runPlan() {
  const todo = plan.steps.filter((s) => !s.skip);
  planRunning = true;
  renderPlan();
  // Baudraten zuerst: danach läuft der Zähler mit der Bus-Baudrate, auch für die Adressänderung
  todo.sort((x, y) => (x.kind === "baud" ? 0 : 1) - (y.kind === "baud" ? 0 : 1));
  const switched = new Set();
  for (const [k, s] of todo.entries()) {
    busbar({ text: t("plan_running", k + 1, todo.length), frac: k / todo.length, link: "#fix-plan" });
    s.state = "run";
    renderPlan();
    try {
      if (s.kind === "baud") {
        await doSwitchBaud(s.target, s.to, s.from);
        switched.add(s.target);
      } else {
        await doSetAddress(s.target, s.to, switched.has(s.target) ? null : s.fromBaud);
      }
      s.state = "ok";
    } catch (e) {
      s.state = "err";
      s.error = e.message;
    }
  }
  planRunning = false;
  const ok = todo.filter((s) => s.state === "ok").length;
  busbar({ text: t("plan_done", ok, todo.length), frac: 1, link: "#fix-plan", done: true, error: ok < todo.length });
  renderPlan();
}

function renderRememberMode() {
  document.querySelectorAll(".remember-mode [data-mode]").forEach((b) => b.classList.toggle("active", b.dataset.mode === addressing()));
}

document.querySelector(".remember-mode").addEventListener("click", (ev) => {
  const mode = ev.target.closest("[data-mode]")?.dataset.mode;
  if (!mode || mode === addressing() || !tree) return;
  if (meters.length && !confirm(t("confirm_mode", mode === "primary" ? "PA" : "SA", meters.length))) return;
  guarded(ev.target.closest("button"), async () => {
    const r = await api(`lines/${tree.active.line.id}/addressing`, { method: "PUT", body: { mode } });
    await loadMeters();
    renderRememberMode();
    const msg = t("mode_done", mode === "primary" ? "PA" : "SA", r.changed);
    if (r.kept.length) toast(`${msg} · ${t("mode_kept", r.kept.length)}: ${r.kept.join("; ")}`, true);
    else toast(msg);
  });
});

$("remember-all").onclick = () =>
  guarded($("remember-all"), async () => {
    // nur Zähler, die nach der Einstellung der Linie eindeutig sind: keine gemischte Liste
    const ok = (lastJob?.hits || []).filter((h) => h.status === "ok");
    const list = ok.filter((h) => rememberAddress(h).address).map(meterFromHit);
    const skipped = ok.length - list.length;
    const r = await api("meters/import", { method: "POST", body: { meters: list } });
    toast(t("remembered_all", r.added, list.length - r.added) + (skipped ? " · " + t("remember_skipped", skipped) : ""), skipped > 0);
    await loadMeters();
  });

// ------------------------------------------------------------ Sortieren der Trefferliste

/// Spalte und Richtung (1 aufsteigend, -1 absteigend); null = Reihenfolge des Scans
let hitSort = null;
try {
  hitSort = JSON.parse(storageGet("hit_sort") || "null");
} catch {
  /* ungültig gespeichert */
}

const SORT_KEYS = {
  col_primary: (h) => h.primary_address,
  col_baud: (h) => h.baud,
  col_id: (h) => h.id,
  col_secondary: (h) => h.secondary_address,
  col_manufacturer: (h) => h.manufacturer || null,
  col_version: (h) => h.version,
  col_medium: (h) => (h.medium ? label(h.medium) : null),
  // Probleme zuerst
  col_status: (h) => ({ collision: 0, no_data: 1, ok: 2 })[h.status],
};

/// [Treffer, Index im Scan]; der Index bleibt für Merken, Warum? usw. gültig
function sortedHits(hits) {
  const order = hits.map((h, i) => [h, i]);
  const key = hitSort && SORT_KEYS[hitSort.col];
  if (!key) return order;
  return order.sort(([a, ia], [b, ib]) => {
    const x = key(a);
    const y = key(b);
    // leere Werte immer unten, egal in welche Richtung
    if (x == null || y == null) return x == null && y == null ? ia - ib : x == null ? 1 : -1;
    const c =
      typeof x === "number" && typeof y === "number"
        ? x - y
        : String(x).localeCompare(String(y), undefined, { numeric: true });
    return c * hitSort.dir || ia - ib;
  });
}

$("hits").querySelector("thead").addEventListener("click", (ev) => {
  const col = ev.target.closest("th[data-sort]")?.dataset.sort;
  if (!col) return;
  // auf → ab → wie gescannt
  hitSort = hitSort?.col !== col ? { col, dir: 1 } : hitSort.dir > 0 ? { col, dir: -1 } : null;
  storageSet("hit_sort", JSON.stringify(hitSort));
  if (lastJob) renderJob(lastJob);
});

$("sa-raw").checked = storageGet("sa_raw") === "1";
$("sa-raw").onchange = () => {
  storageSet("sa_raw", $("sa-raw").checked ? "1" : "0");
  if (lastJob) renderJob(lastJob);
};

$("hits").addEventListener("click", (ev) => {
  const an = ev.target.closest("[data-analyze]");
  if (an) {
    ev.preventDefault();
    analyzeHex(an.dataset.analyze).catch((e) => toast(e.message, true));
    return;
  }
  const btn = ev.target.closest("button");
  if (!btn) return;
  const d = btn.dataset;
  if (d.why) {
    if (openWhy.has(d.why)) openWhy.delete(d.why);
    else openWhy.add(d.why);
    renderJob(lastJob);
    return;
  }
  if (d.fixbaud) {
    // Zähler unter "Zähler einstellen" eintragen, Ziel = Baudrate des Busses
    setConfigTarget(d.fixbaud, Number(d.from));
    $("new-baud").value = connBaud;
    spotlight("tile-baud", "switch-baud", t("spot_baud", d.fixbaud, d.from, connBaud));
    $("switch-baud").focus({ preventScroll: true });
    return;
  }
  if (d.fix) {
    // Zähler unter "Zähler einstellen" eintragen; freie Adresse fragt der Bus, nicht die Trefferliste:
    // Zähler ohne Daten haben dort keine PA, belegen sie aber trotzdem
    guarded(btn, async () => {
      setConfigTarget(d.fix, Number(d.from) || null);
      $("set-new").value = "";
      const skip = [...new Set(lastJob.hits.map((h) => h.primary_address).filter((a) => a != null && a > 0))];
      const r = await api("free-address", { method: "POST", body: { from: 1, skip } });
      $("set-new").value = r.address;
      spotlight("tile-pa", "set-addr", t("spot_pa", d.fix, r.address));
      $("set-addr").focus({ preventScroll: true });
    });
    return;
  }
  if (d.remember !== undefined) {
    const h = lastJob.hits[Number(d.remember)];
    guarded(btn, async () => {
      const a = rememberAddress(h);
      if (!a.address) throw new Error(a.reason);
      await api("meters", { method: "POST", body: meterFromHit(h) });
      toast(t("remembered"));
      await loadMeters();
    });
    return;
  }
  if (!d.read) return;
  $("read-addr").value = d.read;
  $("set-old").value = d.read;
  reveal($("sec-read"), "start");
  readMeter(Number(d.baud) || null);
});

async function startScan(path, body) {
  await api(path, { method: "POST", body });
  jobRunning = true;
  updateButtons();
  pollJob();
}

$("scan-primary").onclick = () =>
  guarded($("scan-primary"), () =>
    startScan("scan/primary", {
      // leere Felder: Standardwerte statt 0
      from: Number($("scan-from").value || 0),
      to: Number($("scan-to").value || 250),
      retries: Number($("scan-retries").value || 1),
      bauds: pickedBauds(),
    })
  );

$("scan-secondary").onclick = () =>
  guarded($("scan-secondary"), () => startScan("scan/secondary", { bauds: pickedBauds() }));

// ------------------------------------------------------------ Baudraten für den Scan

$("scan-bauds").innerHTML = BAUDS.map((b) => `<label><input type="checkbox" value="${b}"><span>${b}</span></label>`).join("");
// bis zur Verbindung: Baudrate aus dem Verbindungsfeld
setPickedBauds([Number($("baud").value)]);

function pickedBauds() {
  const list = [...$("scan-bauds").querySelectorAll("input:checked")].map((i) => Number(i.value));
  if (!list.length) throw new Error(t("pick_baud"));
  return list;
}

function setPickedBauds(list) {
  $("scan-bauds").querySelectorAll("input").forEach((i) => (i.checked = list.includes(Number(i.value))));
  updateButtons();
  updateEstimate();
}

/// Obergrenze: jede Adresse ohne Antwort wartet den vollen Timeout ab, je Wiederholung
function updateEstimate() {
  const el = $("scan-estimate");
  const bauds = [...$("scan-bauds").querySelectorAll("input:checked")].map((i) => Number(i.value));
  if (!conn || !bauds.length) return (el.textContent = "");
  const from = Number($("scan-from").value || 0);
  const to = Number($("scan-to").value || 250);
  const tries = Number($("scan-retries").value || 1) + 1;
  const n = Math.max(0, to - from + 1);
  const secs = bauds.reduce((sum, b) => {
    const timeout = conn.timeout_auto ? 330 / b + 0.25 : conn.timeout_ms / 1000;
    return sum + n * tries * timeout;
  }, 0);
  el.textContent = t("estimate", t("duration", secs));
}

$("scan-bauds").addEventListener("change", () => {
  baudsTouched = true;
  updateEstimate();
  updateButtons();
});

$("bauds-all").onclick = () => {
  baudsTouched = true;
  const all = !$("scan-bauds").querySelector("input:not(:checked)");
  setPickedBauds(all ? [] : BAUDS);
};
["scan-from", "scan-to", "scan-retries"].forEach((id) => $(id).addEventListener("input", updateEstimate));

/// Baudraten einzeln abfragen, damit man sieht, was passiert
$("bus-bauds").onclick = async () => {
  const btn = $("bus-bauds");
  const out = $("bus-bauds-result");
  const chip = (b) => $("scan-bauds").querySelector(`input[value="${b}"] + span`);
  // immer alle Baudraten: dauert nur Sekunden, die Markierung gilt nur für den Scan
  const list = BAUDS;
  const found = [];
  checking = true;
  updateButtons();
  btn.textContent = t("bus_bauds_checking");
  BAUDS.forEach((b) => chip(b).classList.remove("found", "none", "checking"));
  try {
    for (const [k, b] of list.entries()) {
      out.textContent = t("bus_bauds_step", b, k + 1, list.length, found);
      busbar({ text: out.textContent, frac: k / list.length, link: "#sec-scan" });
      chip(b).classList.add("checking");
      const r = await api("bus-bauds", { method: "POST", body: { bauds: [b] } });
      chip(b).classList.remove("checking");
      if (r.bauds.length) found.push(b);
      chip(b).classList.add(r.bauds.length ? "found" : "none");
    }
    out.textContent = found.length ? t("bus_bauds_found", found.join(", "), connBaud) : t("bus_bauds_none");
    busbar({ text: out.textContent, frac: 1, link: "#sec-scan", done: true });
    // gefundene Baudraten für den Scan markieren
    if (found.length) {
      baudsTouched = true;
      setPickedBauds(found);
    }
  } catch (e) {
    BAUDS.forEach((b) => chip(b).classList.remove("checking"));
    out.textContent = "";
    busbar(null);
    toast(e.message, true);
  } finally {
    checking = false;
    btn.textContent = t("bus_bauds");
    updateButtons();
  }
};

$("scan-cancel").onclick = cancelJob;

// ------------------------------------------------------------ Auslesen

function renderReading(r) {
  lastReading = r;
  const h = r.header || {};
  const meta = [
    ["meta_id", h.id],
    ["meta_manufacturer", h.manufacturer],
    ["meta_medium", h.medium && label(h.medium)],
    ["meta_version", h.version],
    ["meta_primary", r.primary_address],
    ["meta_secondary", h.secondary_address],
    ["meta_access", h.access_no],
    ["meta_status", h.status !== undefined ? "0x" + h.status.toString(16).padStart(2, "0").toUpperCase() : undefined],
    ["meta_time", r.timestamp + (r.time_unsure ? ` (${t("reading_time_unsure")})` : "")],
  ]
    .filter(([, v]) => v !== undefined && v !== null)
    .map(([k, v]) => `<div><span>${t(k)}</span><span class="mono">${esc(v)}</span></div>`)
    .join("");
  // erster Datensatz gross, meist der Hauptzählerstand
  const first = r.records[0];
  const hero = first
    ? `<div class="hero">
        <span class="hero-label">${esc(label(first.quantity))} · ${esc(label(first.function))}</span>
        <div class="hero-value"><span class="mono">${esc(first.value_text)}</span><span class="unit">${esc(first.unit)}</span></div>
        <span class="hero-sub">${esc([h.manufacturer, h.medium && label(h.medium), r.timestamp].filter(Boolean).join(" · "))}</span>
      </div>`
    : "";
  const rows = r.records
    .map((x, i) => {
      const extra = [];
      if (x.storage) extra.push(`${t("storage")} ${x.storage}`);
      if (x.tariff) extra.push(`${t("tariff")} ${x.tariff}`);
      if (x.subunit) extra.push(`${t("subunit")} ${x.subunit}`);
      const notes = x.notes.length ? ` (${x.notes.map(label).join(", ")})` : "";
      return `<tr>
        <td class="num mono dim">${i + 1}</td>
        <td>${esc(label(x.quantity) + notes)}</td>
        <td class="num mono strong">${esc(x.value_text)}</td>
        <td>${esc(x.unit)}</td>
        <td class="dim">${esc(label(x.function))}${extra.length ? " · " + extra.join(", ") : ""}</td>
        <td class="mono dim">${esc(x.dif)} | ${esc(x.vif)}</td>
      </tr>`;
    })
    .join("");
  $("reading").innerHTML = `
    <div class="${hero ? "reading-top" : ""}">${hero}<div class="meta">${meta}</div></div>
    <div class="table-wrap"><table class="reading-table">
      <thead><tr><th>#</th><th>${t("th_desc")}</th><th class="num">${t("th_value")}</th><th>${t("th_unit")}</th><th>${t("th_kind")}</th><th>DIF | VIF</th></tr></thead>
      <tbody>${rows}</tbody>
    </table></div>`;
}

/// `baud`: Treffer aus dem Scan über alle Baudraten mit dessen Baudrate lesen
async function readMeter(baud = null) {
  await guarded($("read"), async () => {
    const address = $("read-addr").value.trim();
    if (!address) throw new Error(t("enter_address"));
    lastReading = null;
    $("reading").innerHTML = `<p class="loading"><span class="spin"></span>${t("reading_busy")}</p>`;
    try {
      renderReading(await api("read", { method: "POST", body: { address, baud } }));
      await refreshStatus();
    } catch (e) {
      $("reading").innerHTML = "";
      throw e;
    }
  });
}

$("read").onclick = () => readMeter();
$("read-addr").addEventListener("keydown", (e) => e.key === "Enter" && readMeter());

// ------------------------------------------------------------ Zähler einstellen

// ------------------------------------------------------------ Hervorheben, was zu tun ist

/// Kachel hervorheben, die anderen abblenden, Knopf einfärben und in einem Satz sagen, was zu tun ist
function spotlight(tileId, buttonId, text) {
  clearSpotlight();
  $(tileId).closest(".tiles").classList.add("has-spotlight");
  $(tileId).classList.add("spotlight");
  $(buttonId).classList.add("cta");
  const c = document.createElement("div");
  c.id = "callout";
  c.className = "callout";
  c.innerHTML = `<p>${esc(text)}</p><a href="#hits">${esc(t("back_to_hits"))}</a>`;
  $(tileId).querySelector("h3").after(c);
  reveal($(tileId), "center");
}

/// Nach Erfolg: grüne Bestätigung stehen lassen, Hervorhebung aufheben
function spotlightDone(text) {
  const c = $("callout");
  if (!c) return;
  c.classList.add("done");
  c.querySelector("p").textContent = text;
  document.querySelectorAll(".has-spotlight").forEach((e) => e.classList.remove("has-spotlight"));
  document.querySelectorAll(".cta").forEach((e) => e.classList.remove("cta"));
}

function clearSpotlight() {
  $("callout")?.remove();
  document.querySelectorAll(".has-spotlight").forEach((e) => e.classList.remove("has-spotlight"));
  document.querySelectorAll(".spotlight").forEach((e) => e.classList.remove("spotlight"));
  document.querySelectorAll(".cta").forEach((e) => e.classList.remove("cta"));
}

/// Baudrate, mit der der eingetragene Zähler läuft (null = wie der Bus)
let configBaud = null;

function setConfigTarget(address, baud = null) {
  $("set-old").value = address;
  setConfigBaud(baud);
}

function setConfigBaud(baud) {
  configBaud = baud && baud !== connBaud ? baud : null;
  const note = $("config-baud-note");
  note.hidden = !configBaud;
  note.textContent = configBaud ? t("config_baud_note", configBaud) : "";
}

// Wer selbst eine andere Adresse eintippt, meint einen Zähler auf der Bus-Baudrate
$("set-old").addEventListener("input", () => {
  setConfigBaud(null);
  clearSpotlight();
});

/// Treffer der letzten Suche zu einer eingegebenen Adresse
function hitFor(address) {
  const key = address.toUpperCase();
  return lastJob?.hits.find((h) => h.id === key || h.secondary_address === key || String(h.primary_address) === key) ?? null;
}

function configTarget() {
  const address = $("set-old").value.trim();
  if (!address) throw new Error(t("enter_address"));
  return address;
}

/// Zähler der Liste mit derselben Adresse (gleiche Schreibweise)
function listedMeter(address) {
  return meters.find((m) => m.address.toUpperCase() === address.toUpperCase());
}

$("set-addr").onclick = () =>
  guarded($("set-addr"), async () => {
    const address = $("set-old").value.trim();
    const newAddr = $("set-new").value;
    if (!address || newAddr === "") throw new Error(t("enter_both"));
    if (!confirm(t("confirm_set", address, newAddr))) return;
    await doSetAddress(address, Number(newAddr), configBaud);
    toast(t("address_set", newAddr));
    spotlightDone(t("spot_done_pa", address, newAddr));
  });

/// Primäradresse setzen und überall nachführen: Trefferliste, Zählerliste, Änderungsprotokoll
async function doSetAddress(address, newAddr, fromBaud) {
  const hit = hitFor(address);
  await api("set-address", { method: "POST", body: { address, new_address: newAddr, from_baud: fromBaud } });
  // Trefferliste nachführen, damit die Warnung "PA doppelt" ohne neuen Scan verschwindet
  if (hit) {
    hit.primary_address = newAddr;
    renderJob(lastJob);
  }
  // steht der Zähler mit seiner Primäradresse in der Liste, die neue eintragen
  const m = listedMeter(String(address));
  if (m && /^\d+$/.test(m.address)) {
    await api("meters/" + m.id, { method: "PUT", body: { ...m, address: String(newAddr) } });
    await loadMeters();
  }
}

/// Baudrate umstellen und überall nachführen; true, wenn die Zählerliste angepasst wurde
async function doSwitchBaud(address, baud, fromBaud) {
  const hit = hitFor(address);
  await api("switch-baud", { method: "POST", body: { address, baud, from_baud: fromBaud } });
  if (hit) {
    hit.baud = baud;
    renderJob(lastJob);
  }
  const m = listedMeter(address);
  if (!m) return false;
  // eigene Baudrate nur speichern, wenn sie vom Bus abweicht
  await api("meters/" + m.id, { method: "PUT", body: { ...m, baud: baud === connBaud ? null : baud } });
  await loadMeters();
  return true;
}



$("switch-baud").onclick = () =>
  guarded($("switch-baud"), async () => {
    const address = configTarget();
    const baud = Number($("new-baud").value);
    if (!confirm(t("confirm_switch", address, baud))) return;
    const listed = await doSwitchBaud(address, baud, configBaud);
    setConfigBaud(baud);
    spotlightDone(t("spot_done_baud", address, baud));
    toast(t(listed ? "baud_switched_list" : "baud_switched", baud));
  });

$("detect-baud").onclick = () =>
  guarded($("detect-baud"), async () => {
    const address = configTarget();
    const out = $("detect-result");
    out.textContent = t("detect_busy");
    try {
      const r = await api("detect-baud", { method: "POST", body: { address } });
      out.textContent = r.bauds.length ? t("detect_found", r.bauds.join(", ")) : t("detect_none");
      if (r.bauds.length && !r.bauds.includes(connBaud)) {
        // läuft woanders: Befehle mit seiner Baudrate senden, Umstellen auf den Bus vorschlagen
        setConfigBaud(r.bauds[0]);
        $("new-baud").value = connBaud;
      } else if (r.bauds.length) {
        setConfigBaud(null);
      }
    } catch (e) {
      out.textContent = "";
      throw e;
    }
  });

$("app-reset").onclick = () =>
  guarded($("app-reset"), async () => {
    const address = configTarget();
    if (!confirm(t("confirm_reset", address))) return;
    await api("reset", { method: "POST", body: { address, from_baud: configBaud } });
    toast(t("reset_done"));
  });

// ------------------------------------------------------------ Zählerliste

let meters = [];
let editingId = null;

const svg = (d) =>
  `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;
const ICON_EDIT = svg('<path d="M4 20h4L19 9l-4-4L4 16z"/>');
const ICON_DEL = svg('<path d="M6 6l12 12M18 6L6 18"/>');
const ICON_UP = svg('<path d="M6 15l6-6 6 6"/>');
const ICON_DOWN = svg('<path d="M6 9l6 6 6-6"/>');
const ICON_CHART = svg('<path d="M4 19h16M5 15l4-5 4 3 6-7"/>');
const ICON_ALERT = svg('<circle cx="12" cy="12" r="9"/><path d="M12 8v5M12 16.5v.01"/>');

async function loadMeters() {
  meters = await api("meters");
  renderMeters();
  loadDatapoints().catch(() => {});
  // Anzahl Zähler und letzte Auslesung je Linie in der Projekt-Karte mitführen
  if (tree) await loadTree().catch(() => {});
}

/// Plan-Spalte nur, wenn auf der Linie überhaupt etwas geplant ist
const showPlanColumn = () => meters.some((m) => m.plan || (m.plan_info && m.plan_info.effective !== "off"));

function meterPlanCell(m) {
  const pi = m.plan_info || {};
  const own = !!m.plan;
  if (pi.effective === "off") return `<td><span class="${own ? "" : "muted"}">${esc(t("plan_off"))}</span></td>`;
  const chips = [
    pi.disturbed ? `<span class="chip st-collision" title="${esc(t("meter_disturbed_title", pi.fails))}">${esc(t("meter_disturbed"))}</span>` : "",
    pi.paused_until ? `<span class="chip st-no_data" title="${esc(t("meter_paused_title"))}">${esc(t("meter_paused", slotText(pi.paused_until)))}</span>` : "",
  ].join("");
  return `<td class="wrap"><span class="${own ? "" : "muted"}">${esc(planText(pi.effective))}</span>${chips}${
    pi.next ? `<span class="sub">${esc(t("plan_next", slotText(pi.next)))}</span>` : ""
  }</td>`;
}

function renderMeters() {
  const table = $("meters-table");
  const withPlan = showPlanColumn();
  $("th-plan").hidden = !withPlan;
  table.hidden = meters.length === 0;
  $("meters-clear").hidden = meters.length === 0;
  $("meters-empty").hidden = meters.length > 0;
  table.querySelector("tbody").innerHTML = meters
    .map((m) => {
      // ID nur zusätzlich zeigen, wenn die Adresse nicht schon die ID ist
      // die jeweils andere Adresse klein dazu: bei SA die PA, bei PA die ID
      const viaPa = /^\d+$/.test(m.address);
      const info = [
        !viaPa && m.primary_address != null ? `PA ${m.primary_address}` : null,
        m.meter_id !== m.address ? m.meter_id : null,
        m.manufacturer,
      ]
        .filter(Boolean)
        .join(" · ");
      const sub = (text) => (text ? `<span class="sub">${esc(text)}</span>` : "");
      return `<tr>
        <td class="wrap"><span class="name">${esc(m.name || "–")}</span>${sub(m.note)}</td>
        <td class="wrap">${esc(m.location)}</td>
        <td class="mono">${esc(m.address)}${sub(info)}</td>
        <td>${esc(label(m.medium))}</td>
        <td class="num mono">${m.baud ?? ""}</td>
        ${withPlan ? meterPlanCell(m) : ""}
        <td class="wrap">${m.last_value ? `<span class="val">${esc(m.last_value)}</span>` : ""}${sub(
          [m.last_value ? label(m.last_quantity) : "", m.last_read].filter(Boolean).join(" · ")
        )}${m.last_error ? `<span class="err">${ICON_ALERT}${esc(m.last_error)}</span>` : ""}</td>
        <td><div class="actions">
          <button class="small quiet needs-conn" data-meter-read="${m.id}">${t("read")}</button>
          <button class="small quiet" data-meter-values="${m.id}" title="${esc(t("dp_values_title"))}">${esc(t("dp_values_btn"))}</button>
          <button class="icon-btn" data-meter-hist="${m.id}" title="${esc(t("history"))}" aria-label="${esc(t("history"))}">${ICON_CHART}</button>
          <button class="icon-btn" data-meter-edit="${m.id}" title="${esc(t("edit"))}" aria-label="${esc(t("edit"))}">${ICON_EDIT}</button>
          <button class="icon-btn del" data-meter-del="${m.id}" title="${esc(t("delete"))}" aria-label="${esc(t("delete"))}">${ICON_DEL}</button>
        </div></td>
      </tr>`;
    })
    .join("");
  updateButtons();
}

function openMeterForm(m) {
  editingId = m ? m.id : null;
  $("mf-name").value = m?.name ?? "";
  $("mf-location").value = m?.location ?? "";
  $("mf-address").value = m?.address ?? "";
  $("mf-baud").value = m?.baud ?? "";
  $("mf-note").value = m?.note ?? "";
  const line = tree ? findLine(activeLineId) : null;
  $("mf-plan").innerHTML = planField(m?.plan ?? null, t("plan_inherit_line", planText(line?.plan_info?.effective || "off")), t("plan_hint_meter"));
  wirePlanField($("mf-plan"));
  $("mf-exchange").hidden = !m;
  $("meter-form").hidden = false;
  $("mf-name").focus();
}

$("meter-add").onclick = () => openMeterForm(null);
$("mf-cancel").onclick = () => ($("meter-form").hidden = true);

$("meter-form").onsubmit = (ev) => {
  ev.preventDefault();
  guarded(ev.submitter || $("meter-form").querySelector("[type=submit]"), async () => {
    const old = meters.find((m) => m.id === editingId) || {};
    const body = {
      ...old,
      name: $("mf-name").value.trim(),
      location: $("mf-location").value.trim(),
      address: $("mf-address").value.trim(),
      baud: $("mf-baud").value ? Number($("mf-baud").value) : null,
      note: $("mf-note").value.trim(),
    };
    const saved = editingId === null ? await api("meters", { method: "POST", body }) : await api("meters/" + editingId, { method: "PUT", body });
    const plan = planFrom(Object.fromEntries(new FormData($("meter-form")).entries()));
    if (plan !== (old.plan ?? null)) await savePlan("meters/" + saved.id, plan);
    $("meter-form").hidden = true;
    toast(t("meter_saved"));
    await loadMeters();
  });
};

$("meters-table").addEventListener("click", (ev) => {
  const btn = ev.target.closest("button");
  if (!btn) return;
  const d = btn.dataset;
  const m = meters.find((x) => String(x.id) === (d.meterRead ?? d.meterEdit ?? d.meterDel ?? d.meterHist ?? d.meterValues));
  if (!m) return;
  if (d.meterEdit) return openMeterForm(m);
  if (d.meterValues) return guarded(btn, () => openPicker(m));
  if (d.meterHist) return guarded(btn, () => openHistory(m));
  if (d.meterDel) {
    if (!confirm(t("confirm_delete_meter", m.name || m.address))) return;
    guarded(btn, async () => {
      await api("meters/" + m.id, { method: "DELETE" });
      await loadMeters();
    });
    return;
  }
  // Auslesen mit der Baudrate des Zählers, Ergebnis im Bereich "Zähler auslesen"
  $("read-addr").value = m.address;
  $("set-old").value = m.address;
  guarded(btn, async () => {
    $("reading").innerHTML = `<p class="loading"><span class="spin"></span>${t("reading_busy")}</p>`;
    try {
      renderReading(await api(`meters/${m.id}/read`, { method: "POST" }));
    } catch (e) {
      $("reading").innerHTML = "";
      throw e;
    } finally {
      await loadMeters().catch(() => {});
      refreshStatus().catch(() => {});
    }
  });
});

$("meters-clear").onclick = () => {
  if (!confirm(t("confirm_meters_clear", meters.length))) return;
  guarded($("meters-clear"), async () => {
    const r = await api("meters", { method: "DELETE" });
    $("meter-form").hidden = true;
    toast(t("meters_cleared", r.removed));
    await loadMeters();
  });
};

$("meters-read-all").onclick = () =>
  guarded($("meters-read-all"), async () => {
    await api("meters/read-all", { method: "POST" });
    jobRunning = true;
    updateButtons();
    pollJob();
  });

$("meters-import").onchange = (ev) => {
  const file = ev.target.files[0];
  ev.target.value = "";
  if (!file) return;
  guarded($("meter-add"), async () => {
    let data;
    try {
      data = JSON.parse(await file.text());
    } catch {
      throw new Error(t("import_bad"));
    }
    const list = Array.isArray(data) ? data : data?.meters;
    if (!Array.isArray(list)) throw new Error(t("import_bad"));
    const r = await api("meters/import", { method: "POST", body: { meters: list } });
    toast(t("imported", r.added));
    await loadMeters();
  });
};

// ------------------------------------------------------------ Messwerte

$("clear-readings").onclick = () =>
  guarded($("clear-readings"), async () => {
    if (!confirm(t("confirm_clear"))) return;
    await api("readings", { method: "DELETE" });
    await refreshStatus();
  });

// ------------------------------------------------------------ Log

async function pollLog() {
  if (!$("log-box").open) return;
  const entries = await api("log?since=" + logSeq);
  if (!entries.length) return;
  logSeq = entries[entries.length - 1].seq;
  const pre = $("log");
  pre.insertAdjacentHTML(
    "beforeend",
    entries
      .map((e) => {
        // TX/RX-Zeilen anklicken: Telegramm analysieren
        const bytes = e.dir === "TX" || e.dir === "RX" ? ` data-hex="${esc(e.text)}" title="${esc(t("log_click"))}"` : "";
        return `<span class="log-${e.dir}"${bytes}>${e.time} ${e.dir.padEnd(4)} ${esc(e.text)}</span>\n`;
      })
      .join("")
  );
  while (pre.childElementCount > 1000) pre.firstElementChild.remove();
  if ($("log-follow").checked) pre.scrollTop = pre.scrollHeight;
}

$("raw-send").onclick = () =>
  guarded($("raw-send"), async () => {
    const hex = $("raw-hex").value.trim();
    if (!hex) return;
    const r = await api("raw", { method: "POST", body: { hex } });
    const lines = [t("raw_" + r.kind) + (r.hex && r.kind !== "ack" ? ": " + r.hex : "")];
    if (r.decoded) {
      const h = r.decoded.header;
      if (h) lines.push(`ID ${h.id} · ${h.manufacturer} · ${label(h.medium)}`);
      for (const x of r.decoded.records) lines.push(`${label(x.quantity)}: ${x.value_text} ${x.unit}`.trim());
    }
    const out = $("raw-result");
    out.textContent = lines.join("\n");
    out.hidden = false;
  });
$("raw-hex").addEventListener("keydown", (e) => e.key === "Enter" && $("analyze").click());

$("analyze").onclick = () => guarded($("analyze"), () => analyzeHex($("raw-hex").value.trim()));

$("log").addEventListener("click", (ev) => {
  const hex = ev.target.closest("[data-hex]")?.dataset.hex;
  if (hex) analyzeHex(hex).catch((e) => toast(e.message, true));
});

/// Telegramm erklären lassen und unter dem Log anzeigen (sendet nichts)
async function analyzeHex(hex) {
  if (!hex) return;
  const box = $("log-box");
  if (!box.open) box.open = true;
  $("raw-hex").value = hex;
  const a = await api("analyze", { method: "POST", body: { hex } });
  const el = $("analysis");
  el.hidden = false;
  el.innerHTML = `
    <h4>${esc(a.summary)}</h4>
    ${a.problems.length
      ? `<ul class="problems">${a.problems.map((p) => `<li>${esc(p)}</li>`).join("")}</ul>`
      : `<p class="clean">${esc(t("an_clean"))}</p>`}
    <div class="table-wrap"><table>
      <thead><tr><th>${esc(t("an_bytes"))}</th><th>${esc(t("an_field"))}</th><th>${esc(t("an_meaning"))}</th></tr></thead>
      <tbody>${a.fields
        .map((f) => `<tr${f.bad ? ' class="bad"' : ""}><td class="bytes">${esc(f.bytes || "–")}</td><td>${esc(f.name)}</td><td>${esc(f.value)}</td></tr>`)
        .join("")}</tbody>
    </table></div>`;
  reveal(el, "start");
}

$("clear-log").onclick = async () => {
  await api("log", { method: "DELETE" }).catch(() => {});
  $("log").innerHTML = "";
};



// ------------------------------------------------------------ Verlauf eines Zählers

/// { meter, points, keys (Reihen nach Messgrösse/Einheit/Tarif), key (gewählte Reihe), view }
let hist = null;

const parseTime = (s) => new Date(String(s).replace(" ", "T"));

function formatNum(x, maxDigits = 3) {
  return new Intl.NumberFormat(lang === "de" ? "de-CH" : "en-GB", { maximumFractionDigits: maxDigits }).format(x);
}

function seriesName(v) {
  return `${label(v.quantity)}${v.tariff ? ` · ${t("tariff")} ${v.tariff}` : ""}${v.unit ? ` (${v.unit})` : ""}`;
}

async function openHistory(m) {
  const r = await api(`meters/${m.id}/history`);
  const keys = new Map();
  for (const p of r.points) for (const v of p.values) if (!keys.has(v.key)) keys.set(v.key, v);
  // gleicher Zähler nochmal: gewählte Reihe und Ansicht behalten; sonst Hauptwert der Liste zuerst
  const same = hist?.meter.id === m.id ? hist : null;
  const main = [...keys.keys()].find((k) => keys.get(k).quantity === m.last_quantity) ?? [...keys.keys()][0] ?? null;
  hist = {
    meter: m,
    points: r.points,
    keys,
    key: same?.key && keys.has(same.key) ? same.key : main,
    view: same?.view || "chart",
  };
  renderHistory();
  $("history").scrollIntoView({ behavior: "smooth", block: "nearest" });
}

function historySeries() {
  return hist.points
    .map((p) => {
      const v = p.values.find((x) => x.key === hist.key);
      return v
        ? { time: parseTime(p.timestamp), ts: p.timestamp, unsure: p.time_unsure, value: v.value, text: v.text, unit: v.unit }
        : null;
    })
    .filter(Boolean);
}

function renderHistory() {
  const el = $("history");
  if (!hist) {
    el.hidden = true;
    return;
  }
  el.hidden = false;
  const m = hist.meter;
  const name = m.name || m.meter_id || m.address;
  const series = hist.key ? historySeries() : [];
  const opts = [...hist.keys]
    .map(([k, v]) => `<option value="${esc(k)}" ${k === hist.key ? "selected" : ""}>${esc(seriesName(v))}</option>`)
    .join("");
  let sum = "";
  if (series.length >= 2) {
    const a = series[0];
    const b = series[series.length - 1];
    const diff = b.value - a.value;
    const days = (b.time - a.time) / 864e5;
    const u = esc(b.unit);
    sum = `<div class="history-sum">
      <span>${esc(t("history_first"))}: <b>${esc(a.text)} ${u}</b> · ${esc(a.ts)}</span>
      <span>${esc(t("history_last"))}: <b>${esc(b.text)} ${u}</b> · ${esc(b.ts)}</span>
      <span>${esc(t("history_diff"))}: <b>${formatNum(diff)} ${u}</b>${
        days >= 0.5 ? ` · <b>${formatNum(diff / days)} ${u}</b> ${esc(t("history_per_day"))}` : ""
      }</span></div>`;
  }
  el.innerHTML = `<div class="history-head">
      <h3>${esc(t("history_title", name))}</h3>
      ${hist.keys.size ? `<label class="inline"><span>${esc(t("history_series"))}</span> <select id="hist-key">${opts}</select></label>` : ""}
      <span class="spacer"></span>
      <div class="seg" role="group"><button data-hview="chart" class="${hist.view === "chart" ? "active" : ""}">${esc(t("history_chart"))}</button><button data-hview="table" class="${hist.view === "table" ? "active" : ""}">${esc(t("history_table"))}</button></div>
      <button class="small quiet" data-hcsv ${series.length ? "" : "disabled"}>${esc(t("history_csv"))}</button>
      <button class="small quiet" data-hclose>${esc(t("history_close"))}</button>
    </div>${sum}<div id="hist-body"></div>`;
  const body = $("hist-body");
  if (!series.length) body.innerHTML = `<p class="hint">${esc(t("history_none"))}</p>`;
  else if (hist.view === "table") body.innerHTML = historyTable(series);
  else if (series.length < 2) body.innerHTML = `<p class="hint">${esc(t("history_few"))}</p>` + historyTable(series);
  else drawChart(body, series, name);
}

/// Tabelle, neueste zuerst, mit Differenz zur vorherigen Auslesung
function historyTable(series) {
  const rows = series
    .map((p, i) => {
      const d = i ? p.value - series[i - 1].value : null;
      const unsure = p.unsure
        ? ` <span class="chip st-collision" title="${esc(t("reading_time_unsure_title"))}">${esc(t("reading_time_unsure"))}</span>`
        : "";
      return `<tr><td class="mono">${esc(p.ts)}${unsure}</td><td class="num">${esc(p.text)} ${esc(p.unit)}</td><td class="num">${
        d === null ? "" : (d > 0 ? "+" : "") + formatNum(d)
      }</td></tr>`;
    })
    .reverse()
    .join("");
  return `<div class="table-wrap"><table><thead><tr><th>${esc(t("th_time"))}</th><th class="num">${esc(
    t("history_series")
  )}</th><th class="num">${esc(t("th_delta"))}</th></tr></thead><tbody>${rows}</tbody></table></div>`;
}

/// Runde Achsenschritte: 1, 2 oder 5 mal Zehnerpotenz
function niceTicks(lo, hi, n) {
  const raw = (hi - lo) / n;
  const p = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 5, 10].map((k) => k * p).find((s) => s >= raw);
  const out = [];
  for (let v = Math.floor(lo / step) * step; v <= hi + step / 2; v += step) out.push(Number(v.toPrecision(12)));
  if (out[out.length - 1] < hi) out.push(Number((out[out.length - 1] + step).toPrecision(12)));
  return out;
}

/// Achsenbeschriftung mit so vielen Stellen, wie der Abstand der Linien braucht (sonst stehen bei
/// kleinen Änderungen grosser Zählerstände zweimal dieselben Zahlen da)
function axisNum(v, step) {
  const digits = Math.min(6, Math.max(0, -Math.floor(Math.log10(step || 1) + 1e-9)));
  return Math.abs(v) >= 10000 && step >= 1000
    ? new Intl.NumberFormat(lang === "de" ? "de-CH" : "en-GB", { notation: "compact", maximumFractionDigits: 1 }).format(v)
    : new Intl.NumberFormat(lang === "de" ? "de-CH" : "en-GB", { minimumFractionDigits: digits, maximumFractionDigits: digits }).format(v);
}

function timeLabel(d, span) {
  const loc = lang === "de" ? "de-CH" : "en-GB";
  if (span < 2 * 36e5) return d.toLocaleTimeString(loc, { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  if (span < 2 * 864e5) return d.toLocaleTimeString(loc, { hour: "2-digit", minute: "2-digit" });
  return d.toLocaleDateString(loc, { day: "2-digit", month: "2-digit" });
}

/// Eine Reihe als Linie: 2px, dünnes Raster, Endwert beschriftet, Fadenkreuz mit Tooltip
function drawChart(box, pts, name) {
  const W = Math.max(box.clientWidth || 600, 280);
  const H = 240;
  const t0 = +pts[0].time;
  const t1 = +pts[pts.length - 1].time;
  const vals = pts.map((p) => p.value);
  let lo = Math.min(...vals);
  let hi = Math.max(...vals);
  if (lo === hi) {
    lo -= Math.abs(lo) * 0.01 || 1;
    hi += Math.abs(hi) * 0.01 || 1;
  }
  const yt = niceTicks(lo, hi, 4);
  const ystep = yt.length > 1 ? yt[1] - yt[0] : 1;
  lo = yt[0];
  hi = yt[yt.length - 1];
  const last = pts[pts.length - 1];
  const endText = `${last.text} ${last.unit}`;
  // Ränder aus der Länge der Beschriftungen, damit nichts abgeschnitten wird (Schätzung je Zeichen)
  const mg = {
    l: Math.max(...yt.map((v) => axisNum(v, ystep).length)) * 6.6 + 14,
    r: endText.length * 8.2 + 18,
    t: 14,
    b: 28,
  };
  const iw = W - mg.l - mg.r;
  const ih = H - mg.t - mg.b;
  const x = (tm) => mg.l + (t1 === t0 ? iw / 2 : ((tm - t0) / (t1 - t0)) * iw);
  const y = (v) => mg.t + ih - ((v - lo) / (hi - lo)) * ih;
  const path = pts.map((p, i) => `${i ? "L" : "M"}${x(+p.time).toFixed(1)},${y(p.value).toFixed(1)}`).join("");
  const span = t1 - t0;
  const xt = [0, 1, 2, 3, 4].map((k) => new Date(t0 + (span * k) / 4));
  const showDots = pts.length <= 40;
  const grid = yt
    .map((v) => `<line class="grid" x1="${mg.l}" x2="${mg.l + iw}" y1="${y(v)}" y2="${y(v)}"/><text x="${mg.l - 8}" y="${y(v) + 4}" text-anchor="end">${esc(axisNum(v, ystep))}</text>`)
    .join("");
  const xlab = (t1 === t0 ? [new Date(t0)] : xt)
    .map((d, k, a) => {
      const text = timeLabel(d, span);
      // gleiche Beschriftung wie die vorige weglassen
      if (k && text === timeLabel(a[k - 1], span)) return "";
      const anchor = a.length === 1 ? "middle" : k === 0 ? "start" : k === a.length - 1 ? "end" : "middle";
      return `<text x="${x(+d)}" y="${H - 8}" text-anchor="${anchor}">${esc(text)}</text>`;
    })
    .join("");
  const dots = showDots ? pts.map((p) => `<circle class="dot" cx="${x(+p.time)}" cy="${y(p.value)}" r="4"/>`).join("") : "";
  box.innerHTML = `<div class="chart">
    <svg viewBox="0 0 ${W} ${H}" role="img" aria-label="${esc(t("chart_label", name, endText))}">
      <g class="axis">${grid}${xlab}</g>
      <path class="series" d="${path}"/>
      ${dots}
      <circle class="dot" cx="${x(+last.time)}" cy="${y(last.value)}" r="4"/>
      <text class="end-label" x="${x(+last.time) + 9}" y="${y(last.value) + 4}">${esc(endText)}</text>
      <line class="cross" y1="${mg.t}" y2="${mg.t + ih}" visibility="hidden"/>
      <circle class="dot hover" r="5" visibility="hidden"/>
      <rect class="hit" x="${mg.l}" y="${mg.t}" width="${iw}" height="${ih}" tabindex="0"/>
    </svg>
    <div class="chart-tip" hidden></div>
  </div>`;
  const svgEl = box.querySelector("svg");
  const cross = svgEl.querySelector(".cross");
  const hov = svgEl.querySelector(".dot.hover");
  const tip = box.querySelector(".chart-tip");
  const hit = svgEl.querySelector(".hit");
  let idx = pts.length - 1;
  const show = (i) => {
    idx = Math.max(0, Math.min(pts.length - 1, i));
    const p = pts[idx];
    const px = x(+p.time);
    cross.setAttribute("x1", px);
    cross.setAttribute("x2", px);
    cross.setAttribute("visibility", "visible");
    hov.setAttribute("cx", px);
    hov.setAttribute("cy", y(p.value));
    hov.setAttribute("visibility", "visible");
    const scale = svgEl.clientWidth / W;
    tip.hidden = false;
    tip.style.left = `${px * scale}px`;
    tip.style.top = `${y(p.value) * scale}px`;
    // Wert zuerst, Zeitpunkt danach; Texte nur als textContent
    tip.replaceChildren();
    const b = document.createElement("b");
    const key = document.createElement("span");
    key.className = "key";
    b.append(key, `${p.text} ${p.unit}`);
    const ts = document.createElement("span");
    ts.textContent = p.ts;
    tip.append(b, ts);
  };
  const hide = () => {
    cross.setAttribute("visibility", "hidden");
    hov.setAttribute("visibility", "hidden");
    tip.hidden = true;
  };
  hit.addEventListener("pointermove", (ev) => {
    const r = svgEl.getBoundingClientRect();
    const mx = ((ev.clientX - r.left) / r.width) * W;
    let best = 0;
    for (let i = 1; i < pts.length; i++) if (Math.abs(x(+pts[i].time) - mx) < Math.abs(x(+pts[best].time) - mx)) best = i;
    show(best);
  });
  hit.addEventListener("pointerleave", hide);
  hit.addEventListener("focus", () => show(idx));
  hit.addEventListener("blur", hide);
  hit.addEventListener("keydown", (ev) => {
    if (ev.key === "ArrowLeft" || ev.key === "ArrowRight") {
      ev.preventDefault();
      show(idx + (ev.key === "ArrowRight" ? 1 : -1));
    }
  });
}

$("history").addEventListener("change", (ev) => {
  if (ev.target.id !== "hist-key") return;
  hist.key = ev.target.value;
  renderHistory();
});

$("history").addEventListener("click", (ev) => {
  const b = ev.target.closest("button");
  if (!b) return;
  if (b.dataset.hview) {
    hist.view = b.dataset.hview;
    renderHistory();
  } else if (b.dataset.hclose !== undefined) {
    hist = null;
    renderHistory();
  } else if (b.dataset.hcsv !== undefined) {
    // Semikolon und Dezimalkomma wie der übrige CSV-Export (Excel)
    const series = historySeries();
    const v = hist.keys.get(hist.key);
    const lines = [`${t("th_time")};${seriesName(v)}`, ...series.map((p) => `${p.ts};${String(p.text).replace(".", ",")}`)];
    const blob = new Blob(["\ufeff" + lines.join("\r\n")], { type: "text/csv;charset=utf-8" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    const m = hist.meter;
    a.download = `mbusio-${(m.meter_id || m.address).replace(/[^\w-]/g, "_")}-${v.quantity}.csv`;
    a.click();
    URL.revokeObjectURL(a.href);
  }
});

let resizeTimer = null;
window.addEventListener("resize", () => {
  clearTimeout(resizeTimer);
  resizeTimer = setTimeout(() => hist?.view === "chart" && renderHistory(), 150);
});

// ------------------------------------------------------------ Projekte, Gateways, Linien

// ------------------------------------------------------------ Datenpunkte

let datapoints = [];
let dpTemplates = [];
const dpSelected = new Set();
let dpEditing = null;
let picker = null;
let importText = null;
const SEND_MODES = ["every", "change", "daily"];

async function loadDatapoints() {
  const r = await api("datapoints");
  datapoints = r.datapoints;
  dpTemplates = r.templates;
  for (const id of [...dpSelected]) if (!datapoints.some((d) => d.id === id)) dpSelected.delete(id);
  renderDatapoints();
}

function qualityChip(d) {
  const cls = { good: "st-ok", stale: "st-no_data", disturbed: "st-collision", invalid: "st-collision", none: "st-no_data" }[d.quality];
  const title =
    d.quality === "invalid"
      ? t(d.invalid === "status" ? "q_status_title" : "q_decreased_title")
      : d.quality === "stale"
        ? t("q_stale_title")
        : d.quality === "disturbed"
          ? t("q_disturbed_title")
          : "";
  return `<span class="chip ${cls}"${title ? ` title="${esc(title)}"` : ""}>${esc(t("q_" + d.quality))}</span>`;
}

function renderDatapoints() {
  if (tree) {
    const pid = tree.active.project.id;
    $("dp-export").href = `api/projects/${pid}/datapoints/csv?lang=${lang}` + (token ? `&token=${encodeURIComponent(token)}` : "");
  }
  $("dp-table").hidden = !datapoints.length;
  $("dp-empty").hidden = datapoints.length > 0;
  $("dp-table").querySelector("tbody").innerHTML = datapoints
    .map(
      (d) => `<tr class="${dpSelected.has(d.id) ? "selected" : ""}">
      <td><input type="checkbox" data-dp-sel="${d.id}" ${dpSelected.has(d.id) ? "checked" : ""} aria-label="${esc(d.name)}"></td>
      <td class="num mono">${d.no}</td>
      <td class="wrap"><span class="name">${esc(d.name)}</span><span class="sub">${esc(d.meter)} · ${esc(d.label)}</span>${
        d.mqtt && d.mqtt_topic ? `<span class="sub mono">${esc(mqttBase())}/${esc(d.mqtt_topic)}</span>` : ""
      }</td>
      <td class="num">${d.last_value != null ? `<span class="val">${esc(formatNum(d.last_value, 6))} ${esc(d.unit)}</span>` : ""}<span class="sub">${esc(d.last_time || "")}</span></td>
      <td>${qualityChip(d)}</td>
      <td>${esc(t("send_" + d.send))}</td>
      <td><input type="checkbox" data-dp-active="${d.id}" ${d.active ? "checked" : ""} aria-label="${esc(t("th_active"))}"></td>
      <td><input type="checkbox" data-dp-mqtt="${d.id}" ${d.mqtt ? "checked" : ""} aria-label="MQTT" title="${esc(d.mqtt_topic || "")}"></td>
      <td><button class="icon-btn" data-dp-edit="${d.id}" title="${esc(t("edit"))}" aria-label="${esc(t("edit"))}">${ICON_EDIT}</button></td>
    </tr>`
    )
    .join("");
  $("dp-all").checked = datapoints.length > 0 && datapoints.every((d) => dpSelected.has(d.id));
  $("dp-bulk").hidden = dpSelected.size === 0;
  $("dp-bulk-n").textContent = t("dp_selected", dpSelected.size);
  $("dp-bulk-send").innerHTML =
    `<option value="">${esc(t("dp_send_to"))}</option>` + SEND_MODES.map((m) => `<option value="${m}">${esc(t("send_" + m))}</option>`).join("");
  const tpl = $("dp-templates");
  tpl.hidden = !dpTemplates.length;
  tpl.innerHTML =
    `<b>${esc(t("dp_templates"))}</b> ` +
    dpTemplates
      .map(
        (x) => `<span class="dp-tpl">${esc(label(x.medium))}: ${esc(t("dp_tpl_n", x.keys.length))}${x.auto ? ` · ${esc(t("dp_tpl_auto_short"))}` : ""}
        <button class="icon-btn" data-tpl-del="${x.id}" data-medium="${esc(label(x.medium))}" title="${esc(t("dp_tpl_remove"))}" aria-label="${esc(t("dp_tpl_remove"))}">${ICON_DEL}</button></span>`
      )
      .join(" ");
}

$("dp-table").addEventListener("change", (ev) => {
  const el = ev.target;
  if (el.dataset.dpSel) {
    const id = Number(el.dataset.dpSel);
    if (el.checked) dpSelected.add(id);
    else dpSelected.delete(id);
    return renderDatapoints();
  }
  if (el.id === "dp-all") {
    dpSelected.clear();
    if (el.checked) for (const d of datapoints) dpSelected.add(d.id);
    return renderDatapoints();
  }
  if (el.dataset.dpActive) {
    guarded(el, async () => {
      await api("datapoints/bulk", { method: "POST", body: { ids: [Number(el.dataset.dpActive)], action: el.checked ? "on" : "off" } });
      await loadDatapoints();
    });
  }
  if (el.dataset.dpMqtt) {
    guarded(el, async () => {
      await api("datapoints/bulk", { method: "POST", body: { ids: [Number(el.dataset.dpMqtt)], action: el.checked ? "mqtt:on" : "mqtt:off" } });
      await loadDatapoints();
    });
  }
});

$("dp-table").addEventListener("click", (ev) => {
  const b = ev.target.closest("[data-dp-edit]");
  if (b) openDpForm(datapoints.find((d) => d.id === Number(b.dataset.dpEdit)));
});

$("dp-templates").addEventListener("click", (ev) => {
  const b = ev.target.closest("[data-tpl-del]");
  if (!b || !confirm(t("confirm_tpl_remove", b.dataset.medium))) return;
  guarded(b, async () => {
    await api("templates/" + b.dataset.tplDel, { method: "DELETE" });
    await loadDatapoints();
  });
});

async function dpBulk(action) {
  const ids = [...dpSelected];
  if (!ids.length) return;
  if (action === "delete" && !confirm(t("confirm_dp_delete", ids.length))) return;
  await api("datapoints/bulk", { method: "POST", body: { ids, action } });
  if (action === "delete") dpSelected.clear();
  await loadDatapoints();
  if (tree) loadTree().catch(() => {});
}

$("dp-bulk").addEventListener("click", (ev) => {
  const b = ev.target.closest("[data-dp-bulk]");
  if (b) guarded(b, () => dpBulk(b.dataset.dpBulk));
});
$("dp-bulk-send").onchange = () => {
  const m = $("dp-bulk-send").value;
  if (m) guarded($("dp-bulk-send"), () => dpBulk("send:" + m));
};

function openDpForm(d) {
  dpEditing = d;
  $("dpf-name").value = d.name;
  $("dpf-unit").value = d.unit;
  $("dpf-factor").value = d.factor;
  $("dpf-send").innerHTML = SEND_MODES.map((m) => `<option value="${m}" ${m === d.send ? "selected" : ""}>${esc(t("send_" + m))}</option>`).join("");
  $("dpf-active").checked = d.active;
  $("dpf-mqtt").checked = d.mqtt;
  $("dpf-topic").value = d.mqtt_topic || "";
  $("dp-form").hidden = false;
  $("dp-form").scrollIntoView({ behavior: "smooth", block: "nearest" });
  $("dpf-name").focus();
}

$("dpf-cancel").onclick = () => ($("dp-form").hidden = true);
$("dp-form").onsubmit = (ev) => {
  ev.preventDefault();
  guarded(ev.submitter || $("dp-form").querySelector("[type=submit]"), async () => {
    const factor = Number(String($("dpf-factor").value).replace(",", "."));
    await api("datapoints/" + dpEditing.id, {
      method: "PUT",
      body: {
        ...dpEditing,
        name: $("dpf-name").value.trim(),
        unit: $("dpf-unit").value.trim(),
        factor,
        send: $("dpf-send").value,
        active: $("dpf-active").checked,
        mqtt: $("dpf-mqtt").checked,
        mqtt_topic: $("dpf-topic").value.trim() || null,
      },
    });
    $("dp-form").hidden = true;
    await loadDatapoints();
  });
};

/// Auswahl der Werte eines Zählers (unter der Zählerliste)
async function openPicker(m) {
  picker = await api(`meters/${m.id}/values`);
  renderPicker();
  $("dp-picker").scrollIntoView({ behavior: "smooth", block: "nearest" });
}

function renderPicker() {
  const box = $("dp-picker");
  if (!picker) return (box.hidden = true);
  const m = picker.meter;
  const name = m.name || m.meter_id || m.address;
  const none = !picker.values.length;
  // noch nichts gewählt: Hauptwert vorschlagen
  const anySelected = picker.values.some((v) => v.selected);
  const medium = m.medium ? label(m.medium) : "";
  const tpl = picker.template;
  box.innerHTML = `<div class="history-head"><h3>${esc(t("dp_pick_title", name))}</h3><span class="spacer"></span>
      <button type="button" class="small quiet" data-pick-close>${esc(t("history_close"))}</button></div>
    <p class="hint">${esc(none ? t("dp_pick_none") : t("dp_pick_read", picker.read))}</p>
    ${
      none
        ? ""
        : `<div class="dp-pick">${picker.values
            .map(
              (v) => `<label class="dp-pick-row"><input type="checkbox" value="${esc(v.key)}" ${v.selected || (!anySelected && v.main) ? "checked" : ""}>
          <span>${esc(v.label)}${v.main ? ` <span class="chip st-ok">${esc(t("dp_main"))}</span>` : ""}</span>
          <span class="mono">${v.value != null ? `${esc(v.value)} ${esc(v.unit)}` : `<span class="muted">${esc(t("dp_missing"))}</span>`}</span></label>`
            )
            .join("")}</div>
      ${
        medium
          ? `<label class="check"><input type="checkbox" id="pick-tpl" ${tpl ? "checked" : ""}> ${esc(t("dp_tpl", medium))}</label>
             <label class="check sub-check"><input type="checkbox" id="pick-auto" ${tpl?.auto ? "checked" : ""}> ${esc(t("dp_tpl_auto"))}</label>`
          : ""
      }
      <div class="row"><button type="button" data-pick-save>${esc(t("save"))}</button></div>`
    }`;
  box.hidden = false;
}

$("dp-picker").addEventListener("click", (ev) => {
  if (ev.target.closest("[data-pick-close]")) {
    picker = null;
    return renderPicker();
  }
  const b = ev.target.closest("[data-pick-save]");
  if (!b) return;
  guarded(b, async () => {
    const keys = [...$("dp-picker").querySelectorAll(".dp-pick input:checked")].map((x) => x.value);
    const tplOn = $("pick-tpl")?.checked;
    const body = { keys, template: tplOn ? { auto: !!$("pick-auto")?.checked } : null };
    const r = await api(`meters/${picker.meter.id}/datapoints`, { method: "PUT", body });
    toast(t("dp_saved", r.added, r.removed, r.others));
    picker = null;
    renderPicker();
    await loadDatapoints();
    if (tree) loadTree().catch(() => {});
    $("sec-datapoints").scrollIntoView({ behavior: "smooth", block: "nearest" });
  });
});

// CSV: erst Vorschau, dann übernehmen
$("dp-import").onchange = (ev) => {
  const file = ev.target.files[0];
  ev.target.value = "";
  if (!file) return;
  guarded($("dp-export"), async () => {
    importText = await file.text();
    const r = await api(`projects/${tree.active.project.id}/datapoints/import`, { method: "POST", body: { csv: importText, apply: false } });
    renderImportPreview(r);
  });
};

function renderImportPreview(r) {
  const box = $("dp-import-preview");
  const n = r.changes.length + r.added.length;
  const field = (f) => t("dp_field_" + f);
  box.innerHTML = `<div class="history-head"><h3>${esc(t("dp_import_preview", r.changes.length, r.added.length, r.errors.length))}</h3></div>
    ${r.errors.length ? `<ul class="err-list">${r.errors.map((e) => `<li>${esc(e)}</li>`).join("")}</ul><p class="hint">${esc(t("dp_import_errors"))}</p>` : ""}
    ${!n && !r.errors.length ? `<p class="hint">${esc(t("dp_import_nothing"))}</p>` : ""}
    ${
      n
        ? `<div class="table-wrap"><table><tbody>${r.changes
            .map((c) => `<tr><td class="num mono">${c.no}</td><td>${esc(c.name)}</td><td>${esc(field(c.field))}</td><td class="mono">${esc(c.old)} → <b>${esc(c.new)}</b></td></tr>`)
            .join("")}${r.added
            .map((a) => `<tr><td><span class="chip st-ok">${esc(t("dp_new"))}</span></td><td>${esc(a.name)}</td><td colspan="2">${esc(a.meter)}</td></tr>`)
            .join("")}</tbody></table></div>`
        : ""
    }
    <div class="row">${n && !r.errors.length ? `<button type="button" data-imp-apply>${esc(t("dp_import_apply"))}</button>` : ""}
      <button type="button" class="quiet" data-imp-cancel>${esc(t("dp_import_cancel"))}</button></div>`;
  box.hidden = false;
  box.scrollIntoView({ behavior: "smooth", block: "nearest" });
}

$("dp-import-preview").addEventListener("click", (ev) => {
  if (ev.target.closest("[data-imp-cancel]")) {
    importText = null;
    $("dp-import-preview").hidden = true;
    return;
  }
  const b = ev.target.closest("[data-imp-apply]");
  if (!b) return;
  guarded(b, async () => {
    const r = await api(`projects/${tree.active.project.id}/datapoints/import`, { method: "POST", body: { csv: importText, apply: true } });
    if (!r.applied) return renderImportPreview(r);
    importText = null;
    $("dp-import-preview").hidden = true;
    toast(t("dp_import_done"));
    await loadDatapoints();
  });
});

$("mf-exchange").onclick = () => {
  const m = meters.find((x) => x.id === editingId);
  if (!m) return;
  const addr = prompt(t("exchange_prompt"));
  if (!addr || !addr.trim()) return;
  if (!confirm(t("exchange_confirm", m.meter_id || m.address, addr.trim()))) return;
  guarded($("mf-exchange"), async () => {
    const r = await api(`meters/${m.id}/exchange`, { method: "POST", body: { address: addr.trim() } });
    $("meter-form").hidden = true;
    toast(t("exchange_done") + (r.details.length ? ": " + r.details.join(" · ") : ""));
    await loadMeters();
  });
};

// ------------------------------------------------------------ Simulator

let sim = null;
let simKey = null;
/// eigene Simulation in Bearbeitung: { name, meters }
let simDraft = null;
let simDirty = false;

async function loadSim() {
  sim = await api("sim");
  const conn = sim.line.connection || "";
  const used = conn === "SIMULATOR" ? "praxis" : conn.startsWith("SIMULATOR:") ? conn.slice(10) : null;
  if (!simKey || !sim.scenarios.some((x) => x.key === simKey)) simKey = used || sim.scenarios[0].key;
  selectSim(simKey, true);
}

const simScenario = () => sim.scenarios.find((x) => x.key === simKey);
const simConn = (key) => (key === "praxis" ? "SIMULATOR" : `SIMULATOR:${key}`);

function selectSim(key, force = false) {
  if (!force && simDirty && !confirm(t("confirm_sim_discard"))) {
    $("sim-select").value = simKey;
    return;
  }
  simKey = key;
  const s = simScenario();
  simDraft = s.builtin ? null : { name: s.name, meters: s.meters.map((m) => ({ ...m })) };
  simDirty = false;
  renderSim();
}

function renderSim() {
  const s = simScenario();
  $("sim-select").innerHTML = sim.scenarios
    .map((x) => `<option value="${esc(x.key)}" ${x.key === simKey ? "selected" : ""}>${esc(x.name)}${x.builtin ? ` (${esc(t("sim_template"))})` : ""}</option>`)
    .join("");
  $("sim-delete").hidden = s.builtin;
  const inUse = sim.line.connection === simConn(s.key);
  $("sim-line").innerHTML = `${esc(t("sim_line", sim.line.name, sim.line.connection))}${inUse ? ` · <b>${esc(t("sim_line_uses"))}</b>` : ""}`;
  $("sim-use").disabled = inUse && connected;
  const meters = simDraft ? simDraft.meters : s.meters;
  const ro = !simDraft;
  const opt = (list, val, text) => list.map((v) => `<option value="${esc(v)}" ${String(v) === String(val) ? "selected" : ""}>${esc(text(v))}</option>`).join("");
  const rows = meters
    .map((m, i) =>
      ro
        ? `<tr><td class="num mono">${m.primary}</td><td class="mono">${esc(m.id)}</td><td class="mono">${esc(m.manufacturer)}</td>
            <td>${esc(t("sim_m_" + m.medium))}</td><td class="mono">${m.baud ?? esc(t("sim_baud_bus"))}</td>
            <td>${m.fault === "none" ? esc(t("sim_f_none")) : `<span class="chip st-collision">${esc(t("sim_f_" + m.fault))}</span>`}</td><td></td></tr>`
        : `<tr data-i="${i}">
            <td><input class="mono sim-pa" data-f="primary" value="${m.primary}" inputmode="numeric" aria-label="PA"></td>
            <td><input class="mono sim-id" data-f="id" value="${esc(m.id)}" inputmode="numeric" maxlength="8" aria-label="ID"></td>
            <td><input class="mono sim-man" data-f="manufacturer" value="${esc(m.manufacturer)}" maxlength="3" aria-label="${esc(t("sim_col_man"))}"></td>
            <td><select data-f="medium">${opt(sim.media, m.medium, (v) => t("sim_m_" + v))}</select></td>
            <td><select data-f="baud"><option value="">${esc(t("sim_baud_bus"))}</option>${opt(BAUDS, m.baud ?? "", String)}</select></td>
            <td><select data-f="fault">${opt(sim.faults, m.fault, (v) => t("sim_f_" + v))}</select></td>
            <td><button class="icon-btn del" data-sim-del="${i}" title="${esc(t("delete"))}" aria-label="${esc(t("delete"))}">${ICON_DEL}</button></td>
          </tr>`
    )
    .join("");
  const faults = meters.filter((m) => m.fault !== "none").length;
  const warn = simWarnings(meters);
  $("sim-editor").innerHTML = `
    ${ro ? `<p class="hint">${esc(t("sim_builtin_note"))}</p>` : `<div class="row"><label class="grow"><span>${esc(t("sim_name"))}</span> <input id="sim-name" value="${esc(simDraft.name)}"></label></div>`}
    <p class="hint"><b>${esc(t("sim_count", meters.length, faults))}</b></p>
    ${warn.length ? `<ul class="sim-warn">${warn.map((w) => `<li>${esc(w)}</li>`).join("")}</ul>` : ""}
    ${
      meters.length
        ? `<div class="table-wrap"><table class="sim-table"><thead><tr><th class="num">${esc(t("sim_col_pa"))}</th><th>${esc(t("sim_col_id"))}</th><th>${esc(
            t("sim_col_man")
          )}</th><th>${esc(t("sim_col_medium"))}</th><th>${esc(t("sim_col_baud"))}</th><th>${esc(t("sim_col_fault"))}</th><th></th></tr></thead><tbody>${rows}</tbody></table></div>`
        : `<p class="empty">${esc(t("sim_empty"))}</p>`
    }
    ${
      ro
        ? ""
        : `<div class="row sim-actions">
        <button type="button" class="small ghost" data-sim-add>${esc(t("sim_add"))}</button>
        <span class="sim-many"><input id="sim-n" class="mono" value="10" inputmode="numeric" aria-label="${esc(t("sim_add_n"))}">
          <select id="sim-n-medium">${opt(sim.media, "water", (v) => t("sim_m_" + v))}</select>
          <button type="button" class="small ghost" data-sim-add-many>${esc(t("sim_add_many"))}</button></span>
        <span class="spacer"></span>
        <button type="button" data-sim-save ${simDirty ? "" : "disabled"}>${esc(t("sim_save"))}</button></div>`
    }`;
}

/// Hinweise, die in der Praxis zu Fehlern führen (manchmal gewollt)
function simWarnings(meters) {
  const out = [];
  const count = (key) => meters.reduce((m, x) => m.set(x[key], (m.get(x[key]) || 0) + 1), new Map());
  for (const [pa, n] of count("primary")) if (n > 1 && Number(pa) !== 0) out.push(t("sim_dup_pa", pa));
  for (const [id, n] of count("id")) if (n > 1) out.push(t("sim_dup_id", id));
  const zero = meters.filter((m) => Number(m.primary) === 0).length;
  if (zero) out.push(t("sim_pa0", zero));
  return out;
}

const SIM_MAN = { heat: "KAM", cooling: "KAM", heat_old: "KAM", electricity: "ABB", gas: "ELS" };

/// nächster freier Zähler: PA nach der grössten, ID fortlaufend
function simNext(medium) {
  const ms = simDraft.meters;
  const pa = Math.min(250, ms.reduce((m, x) => Math.max(m, Number(x.primary) || 0), 0) + 1);
  const ids = new Set(ms.map((m) => m.id));
  let id = ms.reduce((m, x) => Math.max(m, Number(x.id) || 0), 10000000) + 1;
  while (ids.has(String(id).padStart(8, "0"))) id++;
  return { primary: pa, id: String(id).padStart(8, "0"), manufacturer: SIM_MAN[medium] || "SEN", version: 1, medium, baud: null, fault: "none" };
}

function simChanged() {
  simDirty = true;
  renderSim();
}

$("sim-select").onchange = () => selectSim($("sim-select").value);

$("sim-editor").addEventListener("change", (ev) => {
  const el = ev.target;
  if (el.id === "sim-name") {
    simDraft.name = el.value;
    return simChanged();
  }
  const row = el.closest("tr[data-i]");
  if (!row || !el.dataset.f) return;
  const m = simDraft.meters[Number(row.dataset.i)];
  const f = el.dataset.f;
  if (f === "primary") m.primary = Math.max(0, Math.min(250, Number(el.value) || 0));
  else if (f === "baud") m.baud = el.value ? Number(el.value) : null;
  else if (f === "manufacturer") m.manufacturer = el.value.trim().toUpperCase();
  else if (f === "id") m.id = el.value.trim().padStart(8, "0");
  else m[f] = el.value;
  simChanged();
});

$("sim-editor").addEventListener("click", (ev) => {
  const b = ev.target.closest("button");
  if (!b) return;
  if (b.dataset.simDel !== undefined) {
    simDraft.meters.splice(Number(b.dataset.simDel), 1);
    return simChanged();
  }
  if (b.dataset.simAdd !== undefined) {
    simDraft.meters.push(simNext("water"));
    return simChanged();
  }
  if (b.dataset.simAddMany !== undefined) {
    const n = Math.max(1, Math.min(100, Number($("sim-n").value) || 1));
    const medium = $("sim-n-medium").value;
    for (let k = 0; k < n; k++) simDraft.meters.push(simNext(medium));
    return simChanged();
  }
  if (b.dataset.simSave !== undefined) {
    guarded(b, async () => {
      const r = await api("sim/" + encodeURIComponent(simKey), { method: "PUT", body: { name: simDraft.name, meters: simDraft.meters } });
      simDirty = false;
      toast(t("sim_saved", r.restarted));
      await loadSim();
      refreshStatus().catch(() => {});
    });
  }
});

$("sim-use").onclick = () =>
  guarded($("sim-use"), async () => {
    if (simDirty && !confirm(t("confirm_sim_discard"))) return;
    const r = await api(`sim/${encodeURIComponent(simKey)}/use`, { method: "POST" });
    toast(t("sim_used", r.connection));
    await loadSim();
    await refreshStatus();
    loadTree().catch(() => {});
  });

async function simCreate(name, from) {
  if (simDirty && !confirm(t("confirm_sim_discard"))) return;
  const r = await api("sim", { method: "POST", body: { name, from } });
  simDirty = false;
  simKey = r.key;
  await loadSim();
  loadPorts().catch(() => {});
}

$("sim-copy").onclick = () =>
  guarded($("sim-copy"), () => {
    const name = prompt(t("sim_new_prompt"), t("sim_copy_name", simScenario().name));
    if (name && name.trim()) return simCreate(name.trim(), simKey);
  });

$("sim-new").onclick = () =>
  guarded($("sim-new"), () => {
    const name = prompt(t("sim_new_prompt"), t("sim_new_name"));
    if (name && name.trim()) return simCreate(name.trim(), null);
  });

$("sim-delete").onclick = () => {
  const s = simScenario();
  if (!confirm(t("confirm_sim_delete", s.name))) return;
  guarded($("sim-delete"), async () => {
    await api("sim/" + encodeURIComponent(s.key), { method: "DELETE" });
    simKey = null;
    simDirty = false;
    await loadSim();
    loadPorts().catch(() => {});
  });
};

// ------------------------------------------------------------ MQTT

let mqttConfig = null;
const mqttBase = () => (mqttConfig?.base || "mbusio").replace(/\/+$/, "");

async function loadMqtt() {
  const r = await api("mqtt");
  mqttConfig = r.config;
  mqttStatus = r.status;
  const c = r.config;
  $("mq-enabled").checked = c.enabled;
  $("mq-host").value = c.host;
  $("mq-port").value = c.port;
  $("mq-tls").checked = c.tls;
  $("mq-insecure").checked = c.insecure;
  $("mq-user").value = c.username;
  $("mq-pass").value = "";
  $("mq-pass").placeholder = r.has_password ? t("mqtt_pass_saved") : "";
  $("mq-client").value = c.client_id;
  $("mq-base").value = c.base;
  $("mq-time").checked = c.time_sync;
  $("mq-ca").value = c.ca;
  updateMqttForm();
  renderMqttState();
}

function updateMqttForm() {
  const base = ($("mq-base").value.trim() || "mbusio").replace(/\/+$/, "");
  $("mq-time-label").textContent = t("mqtt_time", base + "/time");
  $("mq-insecure").closest("label").hidden = !$("mq-tls").checked;
  $("mqtt-form").querySelector(".mqtt-ca").hidden = !$("mq-tls").checked;
  const sample = datapoints.find((d) => d.mqtt_topic)?.mqtt_topic || "projekt/gateway-1-linie-1/whg-1/energy";
  $("mqtt-example").textContent = t("mqtt_example", `${base}/${sample}`);
}

function renderMqttState() {
  const s = mqttStatus;
  if (!s) return;
  const chip = $("mqtt-chip");
  chip.className = "chip " + ({ connected: "on", error: "st-collision", connecting: "st-no_data" }[s.state] || "off");
  chip.textContent = t("mqtt_" + (s.state || "off"));
  $("mqtt-state").textContent =
    s.state === "connected"
      ? t("mqtt_state_connected", s.broker, s.since, s.sent, s.queued)
      : s.state === "connecting"
        ? t("mqtt_state_connecting", s.broker)
        : s.state === "error"
          ? t("mqtt_state_error", s.broker, s.error, s.queued)
          : t("mqtt_state_off");
  renderBusbar();
}

for (const id of ["mq-tls", "mq-base"]) $(id).addEventListener("input", updateMqttForm);
$("mq-tls").addEventListener("change", () => {
  // üblicher Port mitwechseln, solange man keinen eigenen hat
  const p = $("mq-port").value.trim();
  if (!p || p === "1883" || p === "8883") $("mq-port").value = $("mq-tls").checked ? "8883" : "1883";
  updateMqttForm();
});

$("mqtt-form").onsubmit = (ev) => {
  ev.preventDefault();
  guarded(ev.submitter || $("mqtt-form").querySelector("[type=submit]"), async () => {
    const pass = $("mq-pass").value;
    await api("mqtt", {
      method: "PUT",
      body: {
        config: {
          enabled: $("mq-enabled").checked,
          host: $("mq-host").value.trim(),
          port: Number($("mq-port").value) || ($("mq-tls").checked ? 8883 : 1883),
          tls: $("mq-tls").checked,
          insecure: $("mq-insecure").checked,
          ca: $("mq-ca").value,
          username: $("mq-user").value.trim(),
          password: "",
          client_id: $("mq-client").value.trim(),
          base: $("mq-base").value.trim() || "mbusio",
          time_sync: $("mq-time").checked,
        },
        // leer = gespeichertes behalten; Benutzer gelöscht = Passwort auch
        password: pass ? pass : $("mq-user").value.trim() ? null : "",
      },
    });
    toast(t("mqtt_saved"));
    await loadMqtt();
    setTimeout(() => refreshStatus().catch(() => {}), 1500);
  });
};

// ------------------------------------------------------------ Ausleseplan

const PLAN_KINDS = ["off", "monthly", "daily", "hourly", "15min"];
const ICON_CLOCK = svg('<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>');

/// "daily 00:05" → "täglich 00:05"
function planText(key) {
  if (!key) return "";
  const [kind, time] = key.split(" ");
  return kind === "daily" || kind === "monthly" ? t("plan_" + kind, time) : t("plan_" + kind);
}

/// "2026-10-18 00:05:00" → "heute 00:05", "morgen 00:05" oder "20.10. 00:05"
function slotText(s) {
  if (!s) return "";
  const d = parseTime(s);
  const day = (x) => new Date(x.getFullYear(), x.getMonth(), x.getDate());
  const days = Math.round((day(d) - day(new Date())) / 86400000);
  const hm = s.slice(11, 16);
  if (days === 0) return `${t("day_today")} ${hm}`;
  if (days === 1) return `${t("day_tomorrow")} ${hm}`;
  return `${d.getDate()}.${d.getMonth() + 1}. ${hm}`;
}

/// Auswahl des Plans für Formulare; `inherit` = Text der Option "wie Projekt/Linie" (sonst keine)
function planField(value, inherit, hint) {
  const [kind, time] = value ? value.split(" ") : [inherit ? "" : "off", null];
  const opts = (inherit ? [["", inherit]] : []).concat(PLAN_KINDS.map((k) => [k, t("plan_opt_" + k)]));
  return `<div class="row plan-row">
      <label><span>${esc(t("plan"))}</span><select name="plan_kind">${opts
        .map(([v, l]) => `<option value="${v}" ${v === kind ? "selected" : ""}>${esc(l)}</option>`)
        .join("")}</select></label>
      <label class="plan-time"><span>${esc(t("plan_time"))}</span><input type="time" name="plan_time" value="${esc(time || "00:05")}"></label>
    </div>${hint ? `<p class="hint plan-hint">${esc(hint)}</p>` : ""}`;
}

/// Uhrzeit nur bei täglich und monatlich
function wirePlanField(root) {
  const sel = root.querySelector("[name=plan_kind]");
  const wrap = root.querySelector(".plan-time");
  const update = () => (wrap.hidden = !["daily", "monthly"].includes(sel.value));
  sel.addEventListener("change", update);
  update();
}

/// Plan aus den Formularwerten; null = erben
function planFrom(values) {
  const k = values.plan_kind;
  if (!k) return null;
  return k === "daily" || k === "monthly" ? `${k} ${values.plan_time || "00:05"}` : k;
}

async function savePlan(path, plan) {
  const r = await api(path + "/plan", { method: "PUT", body: { plan } });
  if (r.warnings?.length) toast(t("plan_saved_warn", r.warnings.join(" ")), true);
  // geerbte Pläne der Zähler ändern sich mit
  await loadMeters().catch(() => {});
}

/// Zeile unter der Linie: Plan, nächster Termin, letztes Ergebnis, Störungen
function linePlanHtml(l) {
  const p = l.plan_info;
  if (!p || !p.meters) return "";
  const chip = (cls, text, title = "") => `<span class="chip ${cls}"${title ? ` title="${esc(title)}"` : ""}>${esc(text)}</span>`;
  const parts = [`<b>${esc(p.effective !== "off" ? planText(p.effective) : t("plan_meters_own", p.meters))}</b>`];
  const svc = l.service;
  if (l.held) parts.push(chip("st-no_data", t("plan_held"), t("plan_held_title")));
  else if (svc && !svc.connected && svc.error) parts.push(chip("st-collision", t("plan_offline"), svc.error));
  if (p.next && !l.held) parts.push(esc(t("plan_next", slotText(p.next))));
  const r = p.result;
  if (r && r.time && !r.offline) {
    if (r.line_down) parts.push(chip("st-collision", t("plan_line_down"), r.error || ""));
    else parts.push(chip(r.ok === r.total ? "st-ok" : "st-collision", t("plan_last", r.ok, r.total), `${r.time}${r.error ? "\n" + r.error : ""}`));
  }
  if (p.disturbed) parts.push(chip("st-collision", t("plan_disturbed", p.disturbed)));
  if (p.paused) parts.push(chip("st-no_data", t("plan_paused", p.paused), t("meter_paused_title")));
  if (p.load_warn) parts.push(chip("st-collision", t("plan_load", Math.round(p.load * 100)), t("plan_load_title")));
  return `<div class="ln-plan">${ICON_CLOCK}${parts.join(" · ")}</div>`;
}

let tree = null;
/// zusätzlich aufgeklappte Gateways (das der aktiven Linie ist immer offen)
const openGateways = new Set();

async function loadTree() {
  tree = await api("projects");
  renderTree();
}

function activeProjectNode() {
  return tree.projects.find((p) => p.id === tree.active.project.id);
}

function renderTree() {
  if (!tree) return;
  renderRememberMode();
  const a = tree.active;
  $("h-project").textContent = a.project.name;
  $("project-sub").textContent = [a.project.customer, a.project.site, a.project.plan && a.project.plan !== "off" ? `${t("plan")}: ${planText(a.project.plan)}` : ""]
    .filter(Boolean)
    .join(" · ");
  $("crumb-text").textContent = `${a.project.name} › ${a.gateway.name} › ${a.line.name}`;
  $("conn-line").textContent = t("conn_line", `${a.gateway.name} › ${a.line.name}`);
  $("project-select").innerHTML = tree.projects
    .map((p) => `<option value="${p.id}" ${p.id === a.project.id ? "selected" : ""}>${esc(p.name)}</option>`)
    .join("");
  $("project-export").href = `api/projects/${a.project.id}/export` + (token ? `?token=${encodeURIComponent(token)}` : "");
  const node = activeProjectNode();
  // Pfeile zum Verschieben; ganz oben/unten ausgegraut
  const moveBtns = (kind, id, k, n) =>
    `<button class="icon-btn" data-${kind}-move="-1" data-id="${id}" ${k === 0 ? "disabled" : ""} title="${esc(t("move_up"))}" aria-label="${esc(t("move_up"))}">${ICON_UP}</button>` +
    `<button class="icon-btn" data-${kind}-move="1" data-id="${id}" ${k === n - 1 ? "disabled" : ""} title="${esc(t("move_down"))}" aria-label="${esc(t("move_down"))}">${ICON_DOWN}</button>`;
  $("tree").innerHTML =
    node.gateways
      .map(
        (g, gk) => {
          // eingeklappt, ausser beim Gateway der aktiven Linie oder wenn aufgeklappt
          const open = g.id === a.gateway.id || openGateways.has(g.id);
          const issues = g.lines.reduce((n, l) => n + (l.issues?.count || 0), 0);
          return `<div class="gw${open ? "" : " collapsed"}">
      <div class="gw-head">
        <h3 data-gw-toggle="${g.id}" role="button" tabindex="0" aria-expanded="${open}"><svg class="chev" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M6 9l6 6 6-6"/></svg>${esc(g.name)}</h3>${g.address ? `<span class="hint mono">${esc(g.address)}</span>` : ""}
        ${open ? "" : `<span class="gw-sum">${esc(t("gw_sum", g.lines.length, issues))}</span>`}
        <span class="spacer"></span>
        ${node.gateways.length > 1 ? moveBtns("gw", g.id, gk, node.gateways.length) : ""}
        <button class="icon-btn" data-gw-edit="${g.id}" title="${esc(t("edit"))}" aria-label="${esc(t("edit"))}">${ICON_EDIT}</button>
        <button class="icon-btn" data-gw-del="${g.id}" title="${esc(t("delete"))}" aria-label="${esc(t("delete"))}">${ICON_DEL}</button>
      </div>
      ${g.lines
        .map((l, lk) => {
          const active = l.id === a.line.id;
          return `<div class="ln${active ? " active" : ""}">
          <span>${active ? `<span class="badge-active">${esc(t("ln_active"))}</span>` : `<button class="small ghost" data-ln-use="${l.id}">${esc(t("ln_use"))}</button>`}</span>
          <span class="ln-name">${esc(l.name)}</span>
          <span class="ln-conn">${l.connection ? `${esc(l.connection)} · ${l.baud} Bd` : esc(t("ln_no_conn"))}</span>
          <span class="ln-info">${esc(t("ln_info", l.meters, l.last_read))}${
            l.datapoints?.count ? ` · ${esc(t("ln_dp", l.datapoints.count))}` : ""
          }${
            l.datapoints?.invalid ? ` <span class="chip st-collision" title="${esc(t("q_decreased_title"))}">${esc(t("ln_dp_invalid", l.datapoints.invalid))}</span>` : ""
          }${
            l.issues
              ? l.issues.count
                ? ` <span class="chip st-collision" title="${esc(l.issues.items.join("\n"))}">⚠ ${esc(t("ln_issues", l.issues.count))}</span>`
                : ` <span class="chip st-ok">✓ ${esc(t("ln_no_issues"))}</span>`
              : ""
          }</span>
          <span class="actions">
            ${g.lines.length > 1 ? moveBtns("ln", l.id, lk, g.lines.length) : ""}
            <button class="icon-btn" data-ln-edit="${l.id}" title="${esc(t("edit"))}" aria-label="${esc(t("edit"))}">${ICON_EDIT}</button>
            <button class="icon-btn" data-ln-del="${l.id}" title="${esc(t("delete"))}" aria-label="${esc(t("delete"))}">${ICON_DEL}</button>
          </span>
          ${linePlanHtml(l)}
        </div>`;
        })
        .join("")}
      <div class="gw-foot"><button class="small quiet" data-ln-add="${g.id}">${esc(t("ln_add"))}</button></div>
    </div>`;
        }
      )
      .join("") + `<div><button class="small ghost" data-gw-add>${esc(t("gw_add"))}</button></div>`;
}

/// Seite neu laden mit einer Meldung danach (Wechsel von Projekt/Linie betrifft fast alles)
function reloadWith(msg, isError = false) {
  try {
    sessionStorage.setItem("flash", JSON.stringify({ msg, isError }));
  } catch {
    /* ohne Meldung */
  }
  location.reload();
}

async function activateLine(id, name = null) {
  const r = await api(`lines/${id}/activate`, { method: "POST" });
  const line = tree.projects.flatMap((p) => p.gateways).flatMap((g) => g.lines).find((l) => l.id === id);
  name ??= line ? line.name : "";
  reloadWith(r.error ? t("activate_failed", name, r.error) : t("activated", name), !!r.error);
}

const findGateway = (id) => activeProjectNode().gateways.find((g) => g.id === id);
const findLine = (id) => activeProjectNode().gateways.flatMap((g) => g.lines).find((l) => l.id === id);

/// Formular für Projekt, Gateway oder Linie unter dem Baum
function openTreeForm(kind, data, onSave) {
  const f = $("tree-form");
  const field = (key, label, value, attrs = "") =>
    `<label><span>${esc(t(label))}</span> <input name="${key}" value="${esc(value ?? "")}" ${attrs}></label>`;
  let body = "";
  if (kind === "project") {
    body = `<div class="row">${field("name", "f_name", data.name, "required")}${field("customer", "f_customer", data.customer)}</div>
      <div class="row">${field("site", "f_site", data.site)}${field("technician", "f_technician", data.technician)}${field("order_no", "f_order", data.order_no)}</div>
      <div class="row">${field("remarks", "f_remarks", data.remarks)}</div>
      ${planField(data.plan || "off", null, t("plan_hint_project"))}
      <div class="row"><label><span>${esc(t("thin"))}</span><select name="thin">${[0, 30, 90, 365]
        .map((d) => `<option value="${d}" ${d === (data.thin_days || 0) ? "selected" : ""}>${esc(d ? t("thin_days", d) : t("thin_off"))}</option>`)
        .join("")}</select></label></div><p class="hint plan-hint">${esc(t("thin_hint"))}</p>`;
  } else if (kind === "gateway") {
    body = `<div class="row">${field("name", "f_name", data.name, "required")}${field("address", "f_address", data.address, 'class="mono"')}${field("note", "f_note", data.note)}</div>`;
  } else {
    body = `<div class="row">${field("name", "f_name", data.name, "required")}${field("connection", "f_connection", data.connection, `class="mono" placeholder="${esc(t("f_connection_ph"))}" list="port-list"`)}
      <label><span>${esc(t("baud"))}</span><select name="baud">${BAUDS.map((b) => `<option ${b === (data.baud || 2400) ? "selected" : ""}>${b}</option>`).join("")}</select></label></div>
      ${planField(data.plan ?? null, t("plan_inherit_project", planText(tree.active.project.plan || "off")), t("plan_hint_line"))}`;
  }
  f.innerHTML = `<h3>${esc(t("form_" + kind))}</h3>${body}
    <datalist id="port-list">${[...$("port").options].map((o) => `<option value="${esc(o.value)}">`).join("")}</datalist>
    <div class="row"><button type="submit">${esc(t("save"))}</button><button type="button" class="quiet" data-form-cancel>${esc(t("cancel"))}</button></div>`;
  f.hidden = false;
  if (f.querySelector("[name=plan_kind]")) wirePlanField(f);
  f.onsubmit = (ev) => {
    ev.preventDefault();
    const values = Object.fromEntries(new FormData(f).entries());
    if (values.baud) values.baud = Number(values.baud);
    guarded(f.querySelector("[type=submit]"), async () => {
      await onSave({ ...data, ...values });
      f.hidden = true;
    });
  };
  f.querySelector("[data-form-cancel]").onclick = () => (f.hidden = true);
  f.querySelector("input").focus();
  f.scrollIntoView({ behavior: "smooth", block: "nearest" });
}

$("tree").addEventListener("click", (ev) => {
  const toggle = ev.target.closest("[data-gw-toggle]");
  if (toggle) {
    const id = Number(toggle.dataset.gwToggle);
    if (id === tree.active.gateway.id) return;
    if (openGateways.has(id)) openGateways.delete(id);
    else openGateways.add(id);
    renderTree();
    return;
  }
  const b = ev.target.closest("button");
  if (!b) return;
  const d = b.dataset;
  if (d.gwMove || d.lnMove) {
    return guarded(b, async () => {
      await api(`${d.gwMove ? "gateways" : "lines"}/${d.id}/move`, { method: "POST", body: { dir: Number(d.gwMove || d.lnMove) } });
      await loadTree();
    });
  }
  const id = Number(d.lnUse || d.lnEdit || d.lnDel || d.lnAdd || d.gwEdit || d.gwDel || 0);
  if (d.lnUse) return guarded(b, () => activateLine(id));
  if (d.lnEdit) {
    const l = findLine(id);
    return openTreeForm("line", l, async (v) => {
      await api(`lines/${id}`, { method: "PUT", body: { name: v.name, connection: v.connection, baud: v.baud, timeout_ms: l.timeout_ms } });
      const plan = planFrom(v);
      if (plan !== (l.plan ?? null)) await savePlan(`lines/${id}`, plan);
      // aktive Linie mit neuem Anschluss: gleich neu verbinden
      if (id === tree.active.line.id && (v.connection !== l.connection || v.baud !== l.baud)) return activateLine(id, v.name);
      await loadTree();
    });
  }
  if (d.lnAdd) {
    const g = findGateway(id);
    const prefill = g.address ? `tcp://${g.address}:` : "";
    return openTreeForm("line", { name: t("ln_new", g.lines.length + 1), connection: prefill, baud: 2400 }, async (v) => {
      const created = await api(`gateways/${id}/lines`, { method: "POST", body: v });
      const plan = planFrom(v);
      if (plan !== null) await savePlan(`lines/${created.id}`, plan);
      await loadTree();
    });
  }
  if (d.gwEdit) {
    return openTreeForm("gateway", findGateway(id), async (v) => {
      await api(`gateways/${id}`, { method: "PUT", body: v });
      await loadTree();
    });
  }
  if (d.gwAdd !== undefined) {
    const n = activeProjectNode().gateways.length + 1;
    return openTreeForm("gateway", { name: t("gw_new", n), address: "", note: "" }, async (v) => {
      await api(`projects/${tree.active.project.id}/gateways`, { method: "POST", body: v });
      await loadTree();
    });
  }
  if (d.lnDel || d.gwDel) {
    const name = d.lnDel ? findLine(id).name : findGateway(id).name;
    if (!confirm(t(d.lnDel ? "confirm_ln_delete" : "confirm_gw_delete", name))) return;
    return guarded(b, async () => {
      const before = tree.active.line.id;
      await api(`${d.lnDel ? "lines" : "gateways"}/${id}`, { method: "DELETE" });
      const after = (await api("projects")).active.line.id;
      if (after !== before) reloadWith(t("activated", ""));
      else await loadTree();
    });
  }
});

$("project-select").onchange = () =>
  guarded($("project-select"), async () => {
    const p = tree.projects.find((x) => x.id === Number($("project-select").value));
    const first = p?.gateways.flatMap((g) => g.lines)[0];
    if (first) await activateLine(first.id);
  });

$("project-new").onclick = () =>
  guarded($("project-new"), async () => {
    const name = prompt(t("project_new_prompt"));
    if (!name || !name.trim()) return;
    await api("projects", { method: "POST", body: { name: name.trim() } });
    reloadWith(t("activated", name.trim()));
  });

$("project-edit").onclick = () =>
  openTreeForm("project", tree.active.project, async (v) => {
    await api(`projects/${tree.active.project.id}`, { method: "PUT", body: v });
    const plan = planFrom(v);
    if (plan !== (tree.active.project.plan || "off")) await savePlan(`projects/${tree.active.project.id}`, plan);
    const thin = Number(v.thin) || null;
    if (thin !== (tree.active.project.thin_days ?? null))
      await api(`projects/${tree.active.project.id}/thin`, { method: "PUT", body: { days: thin } });
    await loadMeters().catch(() => {});
    await loadTree();
  });

$("project-delete").onclick = () => {
  const p = tree.active.project;
  if (!confirm(t("confirm_project_delete", p.name))) return;
  guarded($("project-delete"), async () => {
    await api(`projects/${p.id}`, { method: "DELETE" });
    reloadWith(t("activated", ""));
  });
};

$("project-import").onchange = (ev) => {
  const file = ev.target.files[0];
  ev.target.value = "";
  if (!file) return;
  guarded($("project-new"), async () => {
    let data;
    try {
      data = JSON.parse(await file.text());
    } catch {
      throw new Error(t("import_bad"));
    }
    const r = await api("projects/import", { method: "POST", body: data });
    tree = await api("projects");
    const p = tree.projects.find((x) => x.id === r.project);
    const first = p?.gateways.flatMap((g) => g.lines)[0];
    toast(t("project_imported", p?.name || ""));
    if (first) await activateLine(first.id);
  });
};

// Meldung nach dem Neuladen (Linie gewechselt, Projekt angelegt …)
try {
  const flash = JSON.parse(sessionStorage.getItem("flash") || "null");
  sessionStorage.removeItem("flash");
  if (flash?.msg) setTimeout(() => toast(flash.msg, flash.isError), 300);
} catch {
  /* privater Modus */
}

// ------------------------------------------------------------ Reiter, Hilfe, Verbindung

const TABS = ["project", "scan", "meters", "tools", "sim"];

function showTab(name) {
  if (!TABS.includes(name)) name = "scan";
  document.querySelectorAll("main [data-tab]").forEach((s) => (s.hidden = s.dataset.tab !== name));
  document.querySelectorAll("[data-tab-go]").forEach((b) => {
    b.classList.toggle("active", b.dataset.tabGo === name);
    b.setAttribute("aria-selected", b.dataset.tabGo === name);
  });
  storageSet("tab", name);
  if (name === "sim") loadSim().catch((e) => toast(e.message, true));
}

/// Element zeigen: richtigen Reiter öffnen, dann hinscrollen
function reveal(el, block = "nearest") {
  if (!el) return;
  const section = el.closest("[data-tab]");
  if (section?.hidden) showTab(section.dataset.tab);
  requestAnimationFrame(() => el.scrollIntoView({ behavior: "smooth", block }));
}

document.querySelectorAll("[data-tab-go]").forEach(
  (b) =>
    (b.onclick = () => {
      showTab(b.dataset.tabGo);
      window.scrollTo({ top: 0 });
    })
);

// Links auf Bereiche (Statusleiste, Pfad, "zurück zur Trefferliste" …) öffnen den passenden Reiter
document.addEventListener("click", (ev) => {
  const a = ev.target.closest('a[href^="#"]');
  if (!a || a.getAttribute("href") === "#") return;
  const el = document.getElementById(a.getAttribute("href").slice(1));
  if (!el || !el.closest("[data-tab]")) return;
  ev.preventDefault();
  reveal(el, "start");
});

$("help-toggle").onclick = () => {
  const on = !document.body.classList.contains("show-help");
  document.body.classList.toggle("show-help", on);
  $("help-toggle").setAttribute("aria-pressed", on);
  storageSet("help", on ? "1" : "0");
};
if (storageGet("help") === "1") {
  document.body.classList.add("show-help");
  $("help-toggle").setAttribute("aria-pressed", "true");
}

$("conn-edit").onclick = () => {
  connEditing = true;
  $("sec-conn").classList.remove("compact");
  $("conn-summary").hidden = true;
};
$("conn-sum-disconnect").onclick = () => $("disconnect").click();

showTab(storageGet("tab") || "scan");

// ------------------------------------------------------------ Start

setLang(lang);
loadPorts().catch((e) => toast(e.message, true));
loadTree()
  .then(() => {
    // nicht verbunden: Anschluss der aktiven Linie in die Felder übernehmen
    const l = tree.active.line;
    if (!connected && l.connection) {
      $("port-manual").value = l.connection;
      $("baud").value = l.baud;
      $("timeout").value = l.timeout_ms;
    }
  })
  .catch((e) => toast(e.message, true));
// letztes Scan-Ergebnis nach dem Neuladen wieder anzeigen (läuft ein Scan, übernimmt pollJob);
// erst nach dem Status, denn die Bewertung braucht die Bus-Baudrate
refreshStatus()
  .then(() => (jobRunning ? null : api("last-scan")))
  .then((job) => {
    if (job && job.hits.length) renderJob(job);
  })
  .catch(() => {});
loadMeters().catch((e) => toast(e.message, true));
loadMqtt().catch(() => {});
setInterval(() => pollLog().catch(() => {}), 1000);
setInterval(() => !jobRunning && refreshStatus().catch(() => {}), 5000);
