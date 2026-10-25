// Gemeinsamer Editor-Code der mbusio-Nodes: Baum von mbusio laden (über Node-RED, siehe
// mbusio-server.js) und Auswahllisten füllen. Texte stehen im Katalog von mbusio-in.
(function () {
  const API_VERSION = 1;
  const t = (k) => RED._("node-red-contrib-mbusio/mbusio-in:mbusio-in.editor." + k);

  window.mbusioEditor = {
    /** Eintrag einer Liste im Baum nach ID */
    find: (list, id) => (list || []).find((x) => String(x.id) === String(id)),

    /** Auswahlliste füllen: erster Eintrag leer („–“ bzw. `empty`), dann `items` */
    options: function ($s, items, value, text, empty) {
      $s.empty().append($("<option>").val("").text(empty || "–"));
      for (const i of items) $s.append($("<option>").val(i.id).text(text(i)));
      $s.val(String(value || ""));
      if ($s.val() === null) $s.val("");
    },

    /**
     * Baum laden, sobald ein Server gewählt ist, und bei ↻ (#mbusio-reload); Meldungen in
     * #mbusio-message. `onTree(tree)` zeichnet die Auswahl; der Baum liegt danach in `node._tree`.
     */
    loader: function (node, onTree) {
      node._tree = null;
      // Node-RED meldet die Wahl des Servers beim Öffnen mehrfach: nur bei neuem Server laden
      let loadedFor = null;
      function load(force) {
        const id = $("#node-input-server").val();
        if (force !== true && id === loadedFor) return;
        loadedFor = id;
        const srv = RED.nodes.node(id);
        $("#mbusio-message").text(srv ? t("loading") : t("noServer"));
        if (!srv) return;
        $.ajax({
          url: "mbusio/tree",
          method: "POST",
          contentType: "application/json",
          data: JSON.stringify({ id: srv.id, url: srv.url, token: srv.credentials && srv.credentials.token }),
        })
          .done(function (tree) {
            if (tree.version !== API_VERSION) return $("#mbusio-message").text(t("version"));
            node._tree = tree;
            $("#mbusio-message").text("");
            onTree(tree);
          })
          .fail(function (x) {
            const why = (x.responseJSON && x.responseJSON.error) || x.status;
            $("#mbusio-message").text(t("loadError") + " (" + why + ")" + (node.selection ? " – " + t("keeps") + ": " + node.selection : ""));
          });
      }
      $("#node-input-server").on("change", load);
      $("#mbusio-reload").on("click", () => load(true));
    },
  };
})();
