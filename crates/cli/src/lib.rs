//! CLI support library for the durable usage store.

mod chart;
mod components;
pub mod github_pages;
pub mod identity;
mod pricing;
mod publish;
pub mod publish_config;
mod summary;
pub mod widgets_publish;

pub use chart::chart_svg;
pub use components::{
    gist_raw_base, github_badge_markdown, join_published_url, publish_snippets,
    render_summary_card, website_embed_html, USAGE_CARD_JS,
};
pub use pricing::{estimate_cost_usd, load_price_table, parse_openrouter_prices, PriceTable};
pub use publish::{
    bundle_from_store, bundle_from_summary, gh_login, load_github_config, pull_dir, pull_gist,
    push_gist, save_github_config, write_bundle, GistRef, GithubConfig, PublishBundle,
};
pub use summary::{
    shields_badge, summarize, summarize_priced, DayTotals, HarnessTotals, ModelTotals,
    ProviderDayTotals, ShieldsBadge, UsageSummary,
};

// Compatibility re-exports: the HTTP implementation and wire contract live in
// `toktally-web`; existing CLI consumers keep the same import paths.
pub use toktally_web::{
    app, serve, serve_stateless, ApiError, ApiState, WireHarnessSync, WireObservation,
    WireSessionList, WireSyncRequest, WireSyncStatus,
};
