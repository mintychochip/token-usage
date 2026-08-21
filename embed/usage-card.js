/* toktally website card. Fetches usage-summary.json. No session ids. */
(function (root) {
  function text(summary) {
    var input = summary.input_tokens;
    var output = summary.output_tokens;
    var line = input + " in / " + output + " out";
    if (summary.estimated_cost_usd != null) {
      line += " · ~$" + Number(summary.estimated_cost_usd).toFixed(4);
    }
    return line;
  }

  function render(summary, el) {
    if (!el) {
      return text(summary);
    }
    el.textContent = "";
    var title = root.document.createElement("strong");
    title.textContent = "toktally";
    var body = root.document.createElement("span");
    body.textContent = " " + text(summary);
    el.appendChild(title);
    el.appendChild(body);
  }

  function mount(el) {
    var url = el.getAttribute("data-summary-url");
    if (!url || !root.fetch) {
      return;
    }
    root
      .fetch(url)
      .then(function (res) {
        return res.json();
      })
      .then(function (summary) {
        render(summary, el);
      });
  }

  function boot() {
    var doc = root.document;
    if (!doc || !doc.querySelectorAll) {
      return;
    }
    var nodes = doc.querySelectorAll(".toktally-card[data-summary-url]");
    for (var i = 0; i < nodes.length; i++) {
      mount(nodes[i]);
    }
  }

  if (root.document && root.document.readyState === "loading") {
    root.document.addEventListener("DOMContentLoaded", boot);
  } else {
    boot();
  }

  root.toktallyCard = root.tokenUsageCard = { render: render, mount: mount, text: text };
})(typeof window !== "undefined" ? window : this);
