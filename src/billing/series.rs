//! Zeitreihe: Stand je Tag bzw. Monatsende, dazu das Ende des Zeitraums; je Zähler und Wert.

use super::table::{head, scale_of, source, Ctx, Notes, Table};
use super::*;

/// Monatsenden im Zeitraum, dazu das Ende des Zeitraums
fn steps(from: NaiveDate, to: NaiveDate, step: &str) -> Vec<NaiveDate> {
    let mut out = Vec::new();
    let mut d = from;
    while d <= to {
        let next = d + Duration::days(1);
        if step == "day" || next.day() == 1 || d == to {
            out.push(d);
        }
        d = next;
    }
    out
}

impl Db {
    pub(super) fn series_rows(
        &self,
        t: &mut Table,
        ctx: &Ctx,
        items: &[Item],
        s: &Settings,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<()> {
        let lang = ctx.lang;
        let dates = steps(from, to, &s.step);
        t.total = items
            .iter()
            .map(|it| it.keys.len().max(1) * if it.keys.is_empty() { 1 } else { dates.len() })
            .sum();
        t.header = [
            vec![tr("Datum", "Date")],
            head(),
            vec![tr("Stand", "Reading"), tr("Quelle", "Source")],
        ]
        .concat();
        for it in items {
            for key in &it.keys {
                let (factor, unit) = scale_of(it, key);
                for d in &dates {
                    if ctx.full(t) {
                        return Ok(());
                    }
                    let st = self.stand_at(it.meter.id, key, *d, None)?;
                    let mut n = Notes::default();
                    if st.value.is_none() {
                        n.add("missing", String::new());
                    }
                    if st.unsure {
                        n.add("unsure", String::new());
                    }
                    n.fault(&st);
                    t.count(&n);
                    t.rows.push(
                        [
                            vec![d.to_string()],
                            ctx.place(it),
                            vec![
                                key_label(key, lang),
                                unit.clone(),
                                ctx.number(st.value, factor),
                                source(&st, lang),
                            ],
                        ]
                        .concat(),
                    );
                }
            }
            if it.keys.is_empty() && !ctx.full(t) {
                let mut n = Notes::default();
                n.add("never_read", String::new());
                t.count(&n);
                t.rows.push(
                    [
                        vec![String::new()],
                        ctx.place(it),
                        vec![
                            String::new(),
                            String::new(),
                            String::new(),
                            tr("noch nie ausgelesen", "never read"),
                        ],
                    ]
                    .concat(),
                );
            }
        }
        Ok(())
    }
}
