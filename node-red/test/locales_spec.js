const fs = require("fs");
const path = require("path");
const assert = require("assert");
const helper = require("node-red-node-test-helper");
const { i18n } = require("@node-red/util");
helper.init(require.resolve("node-red"));

const LOCALES = path.join(__dirname, "../nodes/locales");
const REFERENCE = "en-US";
const languages = fs.readdirSync(LOCALES).filter((l) => l !== REFERENCE);
const files = fs.readdirSync(path.join(LOCALES, REFERENCE));

/// Alle Schlüssel mit Wert: { "a.b": "Text" }
const flatten = (o, prefix = "") =>
  Object.entries(o).flatMap(([k, v]) => (typeof v === "object" ? flatten(v, `${prefix}${k}.`) : [[`${prefix}${k}`, v]]));
const read = (lang, file) => fs.readFileSync(path.join(LOCALES, lang, file), "utf8");
const placeholders = (text) => (text.match(/__\w+__/g) || []).sort();
/// Was gleich bleiben muss: Eigenschaften der Nachricht und Aufbau der Hilfe
const helpShape = (html) => html.match(/data-help-name="[^"]+"|<dt>.*?<\/dt>|<code>[a-z_.]+<\/code>|<h3>|<dl|<\/dl>/g).filter((x) => !x.startsWith("<code>") || /quality|payload|false|true/.test(x)).join(" ");

describe("Node-RED languages", function () {
  it("has the expected languages", function () {
    assert.deepStrictEqual(languages.sort(), ["de", "es", "fr", "it"]);
  });

  for (const lang of languages) {
    for (const file of files) {
      it(`${lang}/${file} matches ${REFERENCE}`, function () {
        const reference = read(REFERENCE, file);
        const text = read(lang, file);
        if (file.endsWith(".json")) {
          const want = flatten(JSON.parse(reference));
          const got = new Map(flatten(JSON.parse(text)));
          assert.deepStrictEqual([...got.keys()].sort(), want.map(([k]) => k).sort());
          for (const [k, v] of want) assert.deepStrictEqual(placeholders(got.get(k)), placeholders(v), k);
        } else {
          assert.strictEqual(helpShape(text), helpShape(reference));
        }
      });
    }
  }

  it("finds a language from a regional browser code", async function () {
    await i18n.init({});
    await i18n.registerMessageCatalog("mbusio-in-test", LOCALES, "mbusio-in.json");
    for (const [browser, word] of [["fr-CH", "Compteur"], ["it-CH", "Contatore"], ["es-MX", "Contador"], ["es", "Contador"]]) {
      await i18n.i.changeLanguage(browser);
      assert.strictEqual(i18n.i.t("mbusio-in-test:mbusio-in.label.meter"), word, browser);
    }
    await i18n.i.changeLanguage(i18n.defaultLang);
  });
});
