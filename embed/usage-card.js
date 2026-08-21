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

  var PALETTE = ["#1E61B8", "#2DC2D2", "#25A3B1", "#983D16", "#921C33", "#7C2489", "#184E95", "#1E848F"];
  var FONT = '&quot;Geist Sans&quot;, system-ui, -apple-system, BlinkMacSystemFont, &quot;Segoe UI&quot;, sans-serif';

  function providerColor(provider) {
    var h = 2166136261;
    for (var i = 0; i < provider.length; i++) {
      h ^= provider.charCodeAt(i);
      h = (h * 16777619) >>> 0;
    }
    return PALETTE[h % PALETTE.length];
  }

  function chartSVG(days) {
    if (!days || !days.length) return "";
    var W = 560, H = 200, pl = 8, pr = 8, pt = 12, pb = 30;
    var pw = W - pl - pr, ph = H - pt - pb;
    var max = 1;
    for (var i = 0; i < days.length; i++) {
      if (days[i].input_tokens > max) max = days[i].input_tokens;
    }
    // Order providers by total input tokens.
    var provTotals = {};
    for (var i = 0; i < days.length; i++) {
      var provs = days[i].providers || [];
      for (var j = 0; j < provs.length; j++) {
        provTotals[provs[j].provider] = (provTotals[provs[j].provider] || 0) + provs[j].input_tokens;
      }
    }
    var order = Object.keys(provTotals).sort(function (a, b) { return provTotals[b] - provTotals[a]; });

    var slot = pw / days.length;
    var barW = Math.max(2, slot * 0.6);
    var bars = "", labels = "";
    for (var i = 0; i < days.length; i++) {
      var x = pl + i * slot;
      var cx = x + slot / 2;
      var y = pt + ph;
      var provs = days[i].providers || [];
      for (var k = 0; k < order.length; k++) {
        var seg = null;
        for (var j = 0; j < provs.length; j++) {
          if (provs[j].provider === order[k]) { seg = provs[j]; break; }
        }
        if (!seg) continue;
        var h = Math.round((seg.input_tokens / max) * ph);
        if (h === 0) continue;
        var top = y - h;
        bars += '<rect x="' + (cx - barW / 2) + '" y="' + top + '" width="' + barW +
          '" height="' + h + '" rx="1" fill="' + providerColor(seg.provider) + '"/>';
        y = top;
      }
      if (days.length <= 31 || i % 5 === 0) {
        labels += '<text x="' + cx + '" y="' + (pt + ph + 16) +
          '" text-anchor="middle" font-size="10" fill="#737373" font-family="' + FONT + '">' + dayLabel(days[i].day) + "</text>";
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
