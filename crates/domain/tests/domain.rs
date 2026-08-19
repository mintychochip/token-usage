//! Construction and identity rules for usage observations.

use token_usage_domain::{
    DomainError, ExtraCounts, Harness, ObservationIdentity, ObservationSource, SessionId,
    SessionStoreCompleteness, UsageCounts, UsageObservation,
};

#[test]
fn named_harnesses_cover_supported_hosts() {
    let names = [
        ("claude-code", Harness::ClaudeCode),
        ("codex", Harness::Codex),
        ("grok", Harness::Grok),
        ("oh-my-pi", Harness::OhMyPi),
        ("jcode", Harness::Jcode),
        ("hermes", Harness::Hermes),
        ("opencode", Harness::OpenCode),
        ("gemini-cli", Harness::GeminiCli),
        ("aider", Harness::Aider),
        ("goose", Harness::Goose),
        ("amp", Harness::Amp),
        ("droid", Harness::Droid),
        ("cline", Harness::Cline),
        ("pi", Harness::Pi),
    ];
    for (slug, expected) in names {
        assert_eq!(Harness::parse(slug).expect("parse"), expected);
        assert_eq!(expected.as_str(), slug);
    }
}

#[test]
fn harness_aliases_normalize_to_canonical_names() {
    assert_eq!(Harness::parse("Claude Code").unwrap(), Harness::ClaudeCode);
    assert_eq!(Harness::parse("claude").unwrap(), Harness::ClaudeCode);
    assert_eq!(Harness::parse("Grok Build").unwrap(), Harness::Grok);
    assert_eq!(Harness::parse("grok-build").unwrap(), Harness::Grok);
    assert_eq!(Harness::parse("omp").unwrap(), Harness::OhMyPi);
    assert_eq!(Harness::parse("Hermes Agent").unwrap(), Harness::Hermes);
    assert_eq!(Harness::parse("hermes-agent").unwrap(), Harness::Hermes);
    assert_eq!(Harness::parse("open-code").unwrap(), Harness::OpenCode);
    assert_eq!(Harness::parse("gemini").unwrap(), Harness::GeminiCli);
    assert_eq!(Harness::parse("factory-droid").unwrap(), Harness::Droid);
    assert_eq!(Harness::parse("factory").unwrap(), Harness::Droid);
}

#[test]
fn unknown_harness_is_rejected() {
    let err = Harness::parse("cursor").unwrap_err();
    assert!(matches!(err, DomainError::UnknownHarness(_)));
}

#[test]
fn named_harnesses_are_pairwise_distinct() {
    let all = Harness::all();
    assert_eq!(all.len(), 14);
    for (i, a) in all.iter().enumerate() {
        for (j, b) in all.iter().enumerate() {
            assert_eq!(a == b, i == j);
        }
    }
}

#[test]
fn empty_session_id_is_rejected() {
    assert!(matches!(
        SessionId::parse(""),
        Err(DomainError::EmptySessionId)
    ));
    assert!(matches!(
        SessionId::parse("   "),
        Err(DomainError::EmptySessionId)
    ));
}

#[test]
fn session_identity_is_harness_plus_session_id() {
    let grok_s1 = ObservationIdentity::new(Harness::Grok, SessionId::parse("s1").unwrap());
    let claude_s1 = ObservationIdentity::new(Harness::ClaudeCode, SessionId::parse("s1").unwrap());
    let grok_s2 = ObservationIdentity::new(Harness::Grok, SessionId::parse("s2").unwrap());
    assert_ne!(grok_s1, claude_s1);
    assert_ne!(grok_s1, grok_s2);
    assert_eq!(
        grok_s1,
        ObservationIdentity::new(Harness::Grok, SessionId::parse("s1").unwrap())
    );
}

#[test]
fn observation_carries_counts_source_and_completeness() {
    let obs = UsageObservation::new(
        ObservationIdentity::new(Harness::Codex, SessionId::parse("thr_1").unwrap()),
        UsageCounts::new(1200, 340).with_extras(ExtraCounts {
            cache_read: Some(80),
            cache_write: Some(12),
            reasoning: Some(50),
        }),
        ObservationSource::PluginReport,
        SessionStoreCompleteness::Complete,
    );
    assert_eq!(obs.identity().harness(), Harness::Codex);
    assert_eq!(obs.identity().session_id().as_str(), "thr_1");
    assert_eq!(obs.counts().input_tokens(), 1200);
    assert_eq!(obs.counts().output_tokens(), 340);
    assert_eq!(obs.counts().extras().cache_read, Some(80));
    assert_eq!(obs.source(), ObservationSource::PluginReport);
    assert_eq!(obs.completeness(), SessionStoreCompleteness::Complete);
}

#[test]
fn extra_counts_are_optional() {
    let counts = UsageCounts::new(10, 2);
    assert_eq!(counts.extras().cache_read, None);
    assert_eq!(counts.extras().cache_write, None);
    assert_eq!(counts.extras().reasoning, None);
}

#[test]
fn source_distinguishes_plugin_report_from_global_approximation() {
    assert_ne!(
        ObservationSource::PluginReport,
        ObservationSource::HarnessGlobalApproximation
    );
}

#[test]
fn completeness_includes_complete_partial_and_unknown() {
    let values = [
        SessionStoreCompleteness::Complete,
        SessionStoreCompleteness::Partial,
        SessionStoreCompleteness::Unknown,
    ];
    assert_eq!(values.len(), 3);
    assert_ne!(values[0], values[1]);
    assert_ne!(values[1], values[2]);
}

#[test]
fn grok_fragment_can_be_unknown_completeness() {
    let obs = UsageObservation::new(
        ObservationIdentity::new(
            Harness::Grok,
            SessionId::parse("019f8886-1253-75f1-98e3-8ab6896f3296").unwrap(),
        ),
        UsageCounts::new(212362, 0),
        ObservationSource::PluginReport,
        SessionStoreCompleteness::Unknown,
    );
    assert_eq!(obs.identity().harness(), Harness::Grok);
    assert_eq!(obs.completeness(), SessionStoreCompleteness::Unknown);
}
