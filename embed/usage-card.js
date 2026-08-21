/* toktally website card. Fetches usage-summary.json. No session ids. */
(function (root) {
  var SELECTOR = ".toktally-card[data-summary-url], .token-usage-card[data-summary-url]";
  var STYLE_ID = "toktally-card-style";

  /* Known providers keep a recognisable colour; anything else draws from
     FALLBACK by first-seen order so two providers never share a swatch. */
  var BRAND = {
    anthropic: "#D97757",
    claude: "#D97757",
    openai: "#10A37F",
    codex: "#10A37F",
    google: "#4285F4",
    gemini: "#4285F4",
    xai: "#A855F7",
    grok: "#A855F7",
    meta: "#0866FF",
    llama: "#0866FF",
    mistral: "#FF7000",
    deepseek: "#4D6BFE",
    qwen: "#615CED",
    cohere: "#39594D",
    moonshot: "#16C79A",
  };

  var FALLBACK = [
    "#3B82F6",
    "#F59E0B",
    "#10B981",
    "#EF4444",
    "#8B5CF6",
    "#EC4899",
    "#14B8A6",
    "#F97316",
  ];

  var CSS = [
    '.toktally-card,.token-usage-card{',
    '--tk-bg:#fff;--tk-fg:#18181b;--tk-muted:#71717a;--tk-border:#e4e4e7;',
    '--tk-track:#f4f4f5;--tk-grid:#ececef;',
    'display:block;box-sizing:border-box;max-width:640px;padding:18px 20px 16px;',
    'border:0;border-radius:0;background:var(--tk-bg);',
    'color:var(--tk-fg);font-family:"Geist Sans",ui-sans-serif,system-ui,-apple-system,',
    'BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif;font-size:14px;',
    'line-height:1.4;-webkit-font-smoothing:antialiased;}',
    '.toktally-card *,.token-usage-card *{box-sizing:border-box;}',
    '@media (prefers-color-scheme:dark){.toktally-card:not([data-theme="light"]),',
    '.token-usage-card:not([data-theme="light"]){--tk-bg:#09090b;--tk-fg:#fafafa;',
    '--tk-muted:#a1a1aa;--tk-border:#27272a;--tk-track:#1c1c1f;--tk-grid:#232327;}}',
    '.toktally-card[data-theme="dark"],.token-usage-card[data-theme="dark"]{',
    '--tk-bg:#09090b;--tk-fg:#fafafa;--tk-muted:#a1a1aa;--tk-border:#27272a;',
    '--tk-track:#1c1c1f;--tk-grid:#232327;}',
    '.tk-head{display:flex;align-items:baseline;justify-content:space-between;gap:12px;}',
    '.tk-brand{font-weight:600;letter-spacing:-.01em;}',
    '.tk-stamp{font-size:11px;color:var(--tk-muted);font-variant-numeric:tabular-nums;}',
    '.tk-stats{display:flex;flex-wrap:wrap;gap:8px 24px;margin-top:12px;}',
    '.tk-stat{display:flex;flex-direction:column;gap:1px;}',
    '.tk-stat b{font-size:20px;font-weight:600;letter-spacing:-.02em;',
    'font-variant-numeric:tabular-nums;}',
    '.tk-stat span{font-size:10px;color:var(--tk-muted);text-transform:uppercase;',
    'letter-spacing:.07em;}',
    '.tk-chart{margin-top:14px;}',
    '.tk-chart svg{display:block;width:100%;height:auto;}',
    '.tk-chart .tk-grid-line{stroke:var(--tk-grid);stroke-width:1;shape-rendering:crispEdges;}',
    '.tk-chart .tk-tick{fill:var(--tk-muted);font-size:9px;font-variant-numeric:tabular-nums;}',
    '.tk-legend{display:flex;flex-wrap:wrap;gap:4px 14px;margin-top:10px;}',
    '.tk-legend span{display:inline-flex;align-items:center;gap:6px;font-size:11px;',
    'color:var(--tk-muted);}',
    '.tk-legend i{width:8px;height:8px;border-radius:2px;flex:none;}',
    '.tk-models{list-style:none;margin:16px 0 0;padding:0;display:grid;gap:7px;}',
    '.tk-models li{display:grid;grid-template-columns:minmax(0,140px) minmax(0,1fr) auto;',
    'align-items:center;gap:12px;}',
    '.tk-models .tk-name{font-size:12px;overflow:hidden;text-overflow:ellipsis;',
    'white-space:nowrap;}',
    '.tk-models .tk-track{height:6px;border-radius:3px;background:var(--tk-track);}',
    '.tk-models .tk-track i{display:block;height:100%;border-radius:3px;min-width:2px;}',
    '.tk-models .tk-val{font-size:11px;color:var(--tk-muted);text-align:right;',
    'font-variant-numeric:tabular-nums;}',
    '.tk-note{margin-top:12px;font-size:12px;color:var(--tk-muted);}',
    '.tk-note[data-kind="error"]{color:#dc2626;}',
    '.tk-skeleton{height:104px;margin-top:14px;border-radius:8px;background:var(--tk-track);',
    'animation:tk-pulse 1.4s ease-in-out infinite;}',
    '@keyframes tk-pulse{0%,100%{opacity:1}50%{opacity:.5}}',
    '@media (max-width:420px){.tk-models li{grid-template-columns:minmax(0,1fr) auto;}',
    '.tk-models .tk-track{display:none;}}',
  ].join("");

  function ensureStyle(doc) {
    if (!doc || doc.getElementById(STYLE_ID)) {
      return;
    }
    var style = doc.createElement("style");
    style.id = STYLE_ID;
    style.textContent = CSS;
    (doc.head || doc.documentElement).appendChild(style);
  }

  function compact(n) {
    n = Number(n) || 0;
    if (n >= 1e12) return (n / 1e12).toFixed(1).replace(/\.0$/, "") + "T";
    if (n >= 1e9) return (n / 1e9).toFixed(1).replace(/\.0$/, "") + "B";
    if (n >= 1e6) return (n / 1e6).toFixed(1).replace(/\.0$/, "") + "M";
    if (n >= 1e3) return (n / 1e3).toFixed(1).replace(/\.0$/, "") + "k";
    return String(Math.round(n));
  }

  function money(v) {
    var n = Number(v);
    if (!isFinite(n)) return "";
    return "$" + (n >= 1 ? n.toFixed(2) : n.toFixed(4));
  }

  var MONTHS = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];

  function dayLabel(dayStart) {
    var d = new Date(dayStart * 1000);
    return MONTHS[d.getUTCMonth()] + " " + d.getUTCDate();
  }

  function stampLabel(seconds) {
    var d = new Date(seconds * 1000);
    return MONTHS[d.getUTCMonth()] + " " + d.getUTCDate() + ", " + d.getUTCFullYear();
  }

  function esc(raw) {
    return String(raw)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function round(n) {
    return Math.round(n * 10) / 10;
  }

  /* Pick a 1/2/5 x 10^n gridline interval so every tick label is a round number. */
  function niceScale(peak, target) {
    if (!(peak > 0)) return { step: 1, count: 1 };
    var raw = peak / target;
    var exp = Math.pow(10, Math.floor(Math.log(raw) / Math.LN10));
    var f = raw / exp;
    var step = (f <= 1 ? 1 : f <= 2 ? 2 : f <= 5 ? 5 : 10) * exp;
    return { step: step, count: Math.ceil(peak / step) };
  }

  function providerOrder(days) {
    var totals = {};
    for (var i = 0; i < (days || []).length; i++) {
      var provs = days[i].providers || [];
      for (var j = 0; j < provs.length; j++) {
        totals[provs[j].provider] = (totals[provs[j].provider] || 0) + provs[j].input_tokens;
      }
    }
    return Object.keys(totals).sort(function (a, b) {
      return totals[b] - totals[a];
    });
  }

  function brandColor(name) {
    var key = String(name || "").toLowerCase();
    for (var brand in BRAND) {
      if (BRAND.hasOwnProperty(brand) && key.indexOf(brand) !== -1) {
        return BRAND[brand];
      }
    }
    return null;
  }

  function buildColors(names) {
    var map = {};
    var used = {};
    var next = 0;
    for (var i = 0; i < names.length; i++) {
      var brand = brandColor(names[i]);
      var color = brand && !used[brand] ? brand : null;
      var attempts = 0;
      while (!color && attempts++ < FALLBACK.length) {
        color = FALLBACK[next++ % FALLBACK.length];
        if (used[color]) color = null;
      }
      if (!color) {
        var hue = (i * 137.508) % 360;
        color = "hsl(" + hue.toFixed(1) + ",65%,55%)";
      }
      used[color] = true;
      map[names[i]] = color;
    }
    return map;
  }

  function chartSVG(days, order, colors) {
    if (!days || !days.length) return "";
    var W = 640, H = 190, pl = 44, pr = 6, pt = 10, pb = 26;
    var pw = W - pl - pr, ph = H - pt - pb;

    var peak = 0;
    for (var i = 0; i < days.length; i++) {
      peak = Math.max(peak, days[i].input_tokens || 0);
    }
    var scale = niceScale(peak, 4);
    var max = scale.step * scale.count;

    var grid = "";
    for (var g = 0; g <= scale.count; g++) {
      var gy = round(pt + ph - (ph * g) / scale.count);
      grid +=
        '<line class="tk-grid-line" x1="' + pl + '" y1="' + gy +
        '" x2="' + (W - pr) + '" y2="' + gy + '"/>' +
        '<text class="tk-tick" x="' + (pl - 8) + '" y="' + (gy + 3) +
        '" text-anchor="end">' + compact(scale.step * g) + "</text>";
    }

    var slot = pw / days.length;
    var barW = Math.max(2, Math.min(20, slot * 0.64));
    /* Keep ~52px between date labels so they never collide. */
    var stride = Math.max(1, Math.ceil(days.length / Math.max(1, Math.floor(pw / 52))));

    var bars = "", labels = "";
    for (var d = 0; d < days.length; d++) {
      var cx = pl + d * slot + slot / 2;
      var y = pt + ph;
      var provs = days[d].providers || [];
      for (var k = 0; k < order.length; k++) {
        var seg = null;
        for (var j = 0; j < provs.length; j++) {
          if (provs[j].provider === order[k]) {
            seg = provs[j];
            break;
          }
        }
        if (!seg || !seg.input_tokens) continue;
        var h = (seg.input_tokens / max) * ph;
        if (h < 0.5) continue;
        y -= h;
        bars +=
          '<rect x="' + round(cx - barW / 2) + '" y="' + round(y) +
          '" width="' + round(barW) + '" height="' + round(h) +
          '" rx="1.5" fill="' + colors[seg.provider] + '"><title>' +
          esc(dayLabel(days[d].day) + " · " + seg.provider + " · " + compact(seg.input_tokens) + " in") +
          "</title></rect>";
      }
      /* Anchor the stride to the newest day so the right edge is always labelled. */
      if ((days.length - 1 - d) % stride === 0) {
        labels +=
          '<text class="tk-tick" x="' + round(cx) + '" y="' + (pt + ph + 15) +
          '" text-anchor="middle">' + esc(dayLabel(days[d].day)) + "</text>";
      }
    }

    return (
      '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ' + W + " " + H +
      '" role="img" aria-label="toktally daily input tokens by provider">' +
      "<g>" + grid + "</g><g>" + bars + "</g><g>" + labels + "</g></svg>"
    );
  }

  function legendNode(doc, order, colors) {
    var wrap = doc.createElement("div");
    wrap.className = "tk-legend";
    for (var i = 0; i < order.length; i++) {
      var item = doc.createElement("span");
      var dot = doc.createElement("i");
      dot.style.background = colors[order[i]];
      var label = doc.createElement("span");
      label.textContent = order[i];
      item.appendChild(dot);
      item.appendChild(label);
      wrap.appendChild(item);
    }
    return wrap;
  }

  function statNode(doc, value, label) {
    var stat = doc.createElement("div");
    stat.className = "tk-stat";
    var b = doc.createElement("b");
    b.textContent = value;
    var span = doc.createElement("span");
    span.textContent = label;
    stat.appendChild(b);
    stat.appendChild(span);
    return stat;
  }

  function modelList(doc, models, colors) {
    if (!models || !models.length) return null;
    var top = models.slice(0, 6);
    var peak = 0;
    for (var i = 0; i < top.length; i++) {
      peak = Math.max(peak, top[i].input_tokens || 0);
    }

    var ul = doc.createElement("ul");
    ul.className = "tk-models toktally-models";
    for (var m = 0; m < top.length; m++) {
      var row = top[m];
      var li = doc.createElement("li");

      var name = doc.createElement("span");
      name.className = "tk-name";
      name.textContent = row.model;
      name.title = row.model;

      var track = doc.createElement("span");
      track.className = "tk-track";
      var fill = doc.createElement("i");
      fill.style.width = (peak ? Math.max(2, (row.input_tokens / peak) * 100) : 0) + "%";
      fill.style.background = colors[row.provider] || brandColor(row.provider) || FALLBACK[0];
      track.appendChild(fill);

      var val = doc.createElement("span");
      val.className = "tk-val";
      val.textContent = compact(row.input_tokens) + " in";

      li.appendChild(name);
      li.appendChild(track);
      li.appendChild(val);
      ul.appendChild(li);
    }
    return ul;
  }

  function text(summary) {
    var line = compact(summary.input_tokens) + " in / " + compact(summary.output_tokens) + " out";
    if (summary.estimated_cost_usd != null) {
      line += " · ~" + money(summary.estimated_cost_usd);
    }
    return line;
  }

  function head(doc, summary) {
    var wrap = doc.createElement("div");
    wrap.className = "tk-head";
    var brand = doc.createElement("strong");
    brand.className = "tk-brand";
    brand.textContent = "toktally";
    wrap.appendChild(brand);
    if (summary && summary.generated_at) {
      var stamp = doc.createElement("span");
      stamp.className = "tk-stamp";
      stamp.textContent = "updated " + stampLabel(summary.generated_at);
      wrap.appendChild(stamp);
    }
    return wrap;
  }

  function render(summary, el) {
    if (!el) {
      return text(summary);
    }
    var doc = el.ownerDocument || root.document;
    ensureStyle(doc);
    el.textContent = "";
    el.setAttribute("data-state", "ready");

    el.appendChild(head(doc, summary));

    var stats = doc.createElement("div");
    stats.className = "tk-stats";
    stats.appendChild(statNode(doc, compact(summary.input_tokens), "tokens in"));
    stats.appendChild(statNode(doc, compact(summary.output_tokens), "tokens out"));
    if (summary.estimated_cost_usd != null) {
      stats.appendChild(statNode(doc, money(summary.estimated_cost_usd), "est. cost"));
    }
    el.appendChild(stats);

    var order = providerOrder(summary.days);
    var colors = buildColors(order);

    var svg = chartSVG(summary.days, order, colors);
    if (svg) {
      var chart = doc.createElement("div");
      chart.className = "tk-chart toktally-chart";
      chart.innerHTML = svg;
      el.appendChild(chart);
      if (order.length > 1) {
        el.appendChild(legendNode(doc, order, colors));
      }
    }

    var models = modelList(doc, summary.models, colors);
    if (models) {
      el.appendChild(models);
    }
  }

  function note(el, message, kind) {
    var doc = el.ownerDocument || root.document;
    ensureStyle(doc);
    el.textContent = "";
    el.setAttribute("data-state", kind);
    el.appendChild(head(doc, null));
    if (kind === "loading") {
      var skeleton = doc.createElement("div");
      skeleton.className = "tk-skeleton";
      el.appendChild(skeleton);
      return;
    }
    var p = doc.createElement("p");
    p.className = "tk-note";
    p.setAttribute("data-kind", kind);
    p.textContent = message;
    el.appendChild(p);
  }

  function mount(el) {
    var url = el.getAttribute("data-summary-url");
    if (!url || !root.fetch) {
      return;
    }
    note(el, "", "loading");
    root
      .fetch(url)
      .then(function (res) {
        if (!res.ok) {
          throw new Error("HTTP " + res.status);
        }
        return res.json();
      })
      .then(function (summary) {
        render(summary, el);
      })
      .catch(function () {
        note(el, "Usage summary unavailable.", "error");
      });
  }

  function boot() {
    var doc = root.document;
    if (!doc || !doc.querySelectorAll) {
      return;
    }
    ensureStyle(doc);
    var nodes = doc.querySelectorAll(SELECTOR);
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
