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

  function compact(n) {
    if (n >= 1e9) return (n / 1e9).toFixed(1) + "B";
    if (n >= 1e6) return (n / 1e6).toFixed(1) + "M";
    if (n >= 1e3) return (n / 1e3).toFixed(1) + "k";
    return String(n);
  }

  function dayLabel(dayStart) {
    var d = new Date(dayStart * 1000);
    var months = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
    return months[d.getUTCMonth()] + " " + d.getUTCDate();
  }

  function chartSVG(days) {
    if (!days || !days.length) return "";
    var W = 560, H = 180, pl = 8, pr = 8, pt = 12, pb = 24;
    var pw = W - pl - pr, ph = H - pt - pb;
    var max = 1;
    for (var i = 0; i < days.length; i++) {
      if (days[i].input_tokens > max) max = days[i].input_tokens;
    }
    var slot = pw / days.length;
    var barW = Math.max(2, slot * 0.6);
    var bars = "", labels = "";
    for (var i = 0; i < days.length; i++) {
      var x = pl + i * slot;
      var h = Math.round((days[i].input_tokens / max) * ph);
      var y = pt + ph - h;
      var cx = x + slot / 2;
      bars += '<rect x="' + (cx - barW / 2) + '" y="' + y + '" width="' + barW +
        '" height="' + h + '" rx="2" fill="#4f8ef7"/>';
      if (days.length <= 31 || i % 5 === 0) {
        labels += '<text x="' + cx + '" y="' + (pt + ph + 16) +
          '" text-anchor="middle" font-size="10" fill="#8b949e">' + dayLabel(days[i].day) + "</text>";
      }
    }
    return '<svg xmlns="http://www.w3.org/2000/svg" width="' + W + '" height="' + H +
      '" viewBox="0 0 ' + W + " " + H + '" role="img" aria-label="toktally daily tokens">' +
      '<rect width="100%" height="100%" fill="transparent"/>' +
      "<g>" + bars + "</g><g>" + labels + "</g></svg>";
  }

  function modelList(models, el) {
    if (!models || !models.length) return;
    var ul = root.document.createElement("ul");
    ul.className = "toktally-models";
    for (var i = 0; i < models.length && i < 8; i++) {
      var li = root.document.createElement("li");
      var name = root.document.createElement("span");
      name.textContent = models[i].model;
      var count = root.document.createElement("span");
      count.textContent = compact(models[i].input_tokens) + " in";
      li.appendChild(name);
      li.appendChild(count);
      ul.appendChild(li);
    }
    el.appendChild(ul);
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

    var chart = chartSVG(summary.days);
    if (chart) {
      var wrap = root.document.createElement("div");
      wrap.className = "toktally-chart";
      wrap.innerHTML = chart;
      el.appendChild(wrap);
    }

    modelList(summary.models, el);
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
