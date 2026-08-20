//! Internal model prices. Users never submit $/token.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;
use toktally_domain::UsageCounts;

/// USD per token for one model.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelPrice {
    pub prompt_per_token: f64,
    pub completion_per_token: f64,
    pub cache_read_per_token: Option<f64>,
    pub cache_write_per_token: Option<f64>,
}

/// Looked-up prices keyed by normalized model id.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PriceTable {
    by_id: BTreeMap<String, ModelPrice>,
}

impl PriceTable {
    fn insert(&mut self, id: &str, price: ModelPrice) {
        let key = normalize(id);
        if key.is_empty() {
            return;
        }
        self.by_id.insert(key.clone(), price.clone());
        if let Some(tail) = key.rsplit('/').next() {
            if tail != key {
                self.by_id.entry(tail.to_string()).or_insert(price);
            }
        }
    }

    fn get(&self, model: &str) -> Option<&ModelPrice> {
        let key = normalize(model);
        self.lookup_exact(&key)
            .or_else(|| self.lookup_variant(&key))
    }

    fn lookup_exact(&self, key: &str) -> Option<&ModelPrice> {
        self.by_id.get(key).or_else(|| {
            key.rsplit('/')
                .next()
                .filter(|tail| *tail != key)
                .and_then(|tail| self.by_id.get(tail))
        })
    }

    fn lookup_variant(&self, key: &str) -> Option<&ModelPrice> {
        for candidate in stripped_suffixes(key) {
            if let Some(price) = self.lookup_exact(&candidate) {
                return Some(price);
            }
        }
        self.best_token_match(key)
    }

    fn best_token_match(&self, key: &str) -> Option<&ModelPrice> {
        let query = tokens(key);
        if query.is_empty() {
            return None;
        }
        let mut best_score = 0usize;
        let mut best: Option<&ModelPrice> = None;
        let mut conflict = false;
        for (id, price) in &self.by_id {
            let Some(score) = suffix_match_score(&tokens(id), &query) else {
                continue;
            };
            if score > best_score {
                best_score = score;
                best = Some(price);
                conflict = false;
            } else if score == best_score && !same_rate(best, Some(price)) {
                conflict = true;
            }
        }
        if conflict {
            None
        } else {
            best
        }
    }
}

/// Parse OpenRouter `GET /api/v1/models` JSON.
pub fn parse_openrouter_prices(value: &Value) -> Result<PriceTable, String> {
    let mut table = PriceTable::default();
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| "openrouter payload missing data[]".to_string())?;
    for item in data {
        let Some(id) = item.get("id").and_then(Value::as_str) else {
            continue;
        };
        let pricing = item.get("pricing").cloned().unwrap_or(Value::Null);
        let Some(prompt) = json_f64(pricing.get("prompt")) else {
            continue;
        };
        let Some(completion) = json_f64(pricing.get("completion")) else {
            continue;
        };
        table.insert(
            id,
            ModelPrice {
                prompt_per_token: prompt,
                completion_per_token: completion,
                cache_read_per_token: json_f64(pricing.get("input_cache_read")),
                cache_write_per_token: json_f64(pricing.get("input_cache_write")),
            },
        );
    }
    Ok(table)
}

/// Parse LiteLLM `model_prices_and_context_window.json`.
pub fn parse_litellm_prices(value: &Value) -> Result<PriceTable, String> {
    let obj = value
        .as_object()
        .ok_or_else(|| "litellm payload must be an object".to_string())?;
    let mut table = PriceTable::default();
    for (id, item) in obj {
        if id.starts_with("sample_") || !item.is_object() {
            continue;
        }
        let Some(prompt) = json_f64(item.get("input_cost_per_token")) else {
            continue;
        };
        let Some(completion) = json_f64(item.get("output_cost_per_token")) else {
            continue;
        };
        table.insert(
            id,
            ModelPrice {
                prompt_per_token: prompt,
                completion_per_token: completion,
                cache_read_per_token: json_f64(item.get("cache_read_input_token_cost")),
                cache_write_per_token: json_f64(item.get("cache_creation_input_token_cost")),
            },
        );
    }
    Ok(table)
}

/// USD for these counts, or `None` when the model or rate is unknown.
pub fn estimate_cost_usd(
    table: &PriceTable,
    model: Option<&str>,
    counts: &UsageCounts,
) -> Option<f64> {
    let price = table.get(model?)?;
    let mut usd = counts.input_tokens() as f64 * price.prompt_per_token
        + counts.output_tokens() as f64 * price.completion_per_token;
    if let (Some(n), Some(rate)) = (counts.extras().cache_read, price.cache_read_per_token) {
        usd += n as f64 * rate;
    }
    if let (Some(n), Some(rate)) = (counts.extras().cache_write, price.cache_write_per_token) {
        usd += n as f64 * rate;
    }
    Some(usd)
}

/// Load prices: `TOKEN_USAGE_PRICES` file, store-adjacent cache, then fetch.
pub fn load_price_table(store_path: &Path) -> Option<PriceTable> {
    if let Some(path) =
        std::env::var_os("TOKTALLY_PRICES").or_else(|| std::env::var_os("TOKEN_USAGE_PRICES"))
    {
        return parse_prices_file(Path::new(&path));
    }
    let cache = store_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prices.json");
    if let Some(table) = parse_prices_file(&cache) {
        return Some(table);
    }
    let fetch_opt = std::env::var("TOKTALLY_PRICES_FETCH")
        .or_else(|_| std::env::var("TOKEN_USAGE_PRICES_FETCH"));
    if fetch_opt.ok().as_deref() == Some("0") {
        return None;
    }
    let url = std::env::var("TOKTALLY_PRICES_URL")
        .or_else(|_| std::env::var("TOKEN_USAGE_PRICES_URL"))
        .unwrap_or_else(|_| "https://openrouter.ai/api/v1/models".to_string());
    let raw = fetch_prices_json(&url)?;
    if let Ok(value) = serde_json::from_str::<Value>(&raw) {
        if let Some(table) = parse_any_prices(&value) {
            let _ = std::fs::write(&cache, raw);
            return Some(table);
        }
    }
    None
}

fn parse_prices_file(path: &Path) -> Option<PriceTable> {
    let raw = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&raw).ok()?;
    parse_any_prices(&value)
}

fn parse_any_prices(value: &Value) -> Option<PriceTable> {
    parse_openrouter_prices(value)
        .ok()
        .filter(|t| !t.by_id.is_empty())
        .or_else(|| {
            parse_litellm_prices(value)
                .ok()
                .filter(|t| !t.by_id.is_empty())
        })
}

fn fetch_prices_json(url: &str) -> Option<String> {
    ureq::get(url)
        .timeout(Duration::from_secs(10))
        .call()
        .ok()?
        .into_string()
        .ok()
}

fn json_f64(value: Option<&Value>) -> Option<f64> {
    let value = value?;
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|n| n as f64))
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}

fn normalize(model: &str) -> String {
    model.trim().to_ascii_lowercase()
}

fn tokens(model: &str) -> Vec<&str> {
    model
        .split(['-', '/'])
        .filter(|part| !part.is_empty())
        .collect()
}

fn is_variant_token(token: &str) -> bool {
    let token = token.to_ascii_lowercase();
    if token.len() >= 2 {
        let (num, unit) = token.split_at(token.len() - 1);
        if matches!(unit, "k" | "m") && !num.is_empty() && num.chars().all(|c| c.is_ascii_digit()) {
            return true;
        }
    }
    if token.len() == 8 && token.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    matches!(
        token.as_str(),
        "latest" | "preview" | "beta" | "exp" | "experimental"
    )
}

fn stripped_suffixes(key: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = key.to_string();
    while let Some((head, tail)) = current.rsplit_once('-') {
        if !is_variant_token(tail) {
            break;
        }
        out.push(head.to_string());
        if let Some((_, rest)) = head.rsplit_once('/') {
            if rest != head {
                out.push(rest.to_string());
            }
        }
        current = head.to_string();
    }
    out
}

fn suffix_match_score(table_tokens: &[&str], query: &[&str]) -> Option<usize> {
    if table_tokens.is_empty() || query.is_empty() {
        return None;
    }
    let min = if table_tokens.len() == 1 { 1 } else { 2 };
    for len in (min..=table_tokens.len()).rev() {
        let suffix = &table_tokens[table_tokens.len() - len..];
        if let Some(start) = find_contiguous(suffix, query) {
            let after = &query[start + len..];
            if after.iter().all(|token| is_variant_token(token)) {
                return Some(len);
            }
        }
    }
    None
}

fn find_contiguous(needle: &[&str], haystack: &[&str]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn same_rate(a: Option<&ModelPrice>, b: Option<&ModelPrice>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => {
            a.prompt_per_token == b.prompt_per_token
                && a.completion_per_token == b.completion_per_token
        }
        _ => false,
    }
}
