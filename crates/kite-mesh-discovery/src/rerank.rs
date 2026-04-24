//! Exact facet filter + cosine rerank.

use kite_mesh_proto::{CapabilityCard, CapabilityFacets};

use crate::encoder::cosine;

/// Scored candidate returned by the rerank pipeline.
#[derive(Debug, Clone)]
pub struct ScoredCandidate {
    pub card: CapabilityCard,
    pub score: f32,
    pub passed_filter: bool,
    pub filter_trace: Vec<String>,
}

/// Apply the exact facet filter. Returns a trace of any mismatches so the
/// `explain_match` path can surface them.
pub fn facet_filter(
    required: &CapabilityFacets,
    candidate: &CapabilityFacets,
) -> (bool, Vec<String>) {
    let mut trace = Vec::new();

    // Required tools must be a subset of candidate tools.
    for t in &required.tools {
        if !candidate.tools.iter().any(|c| c.eq_ignore_ascii_case(t)) {
            trace.push(format!("missing tool: {t}"));
        }
    }
    for o in &required.ontologies {
        if !candidate
            .ontologies
            .iter()
            .any(|c| c.eq_ignore_ascii_case(o))
        {
            trace.push(format!("missing ontology: {o}"));
        }
    }

    // Equality-required fields (skeleton uses case-insensitive exact match).
    for (label, r, c) in [
        (
            "sandbox_level",
            &required.sandbox_level,
            &candidate.sandbox_level,
        ),
        (
            "network_access",
            &required.network_access,
            &candidate.network_access,
        ),
        ("gpu_class", &required.gpu_class, &candidate.gpu_class),
        (
            "privacy_tier",
            &required.privacy_tier,
            &candidate.privacy_tier,
        ),
        ("region", &required.region, &candidate.region),
        (
            "pricing_band",
            &required.pricing_band,
            &candidate.pricing_band,
        ),
        ("trust_tier", &required.trust_tier, &candidate.trust_tier),
    ] {
        if !r.is_empty() && !r.eq_ignore_ascii_case(c) {
            trace.push(format!("{label} mismatch: need {r}, got {c}"));
        }
    }

    let passed = trace.is_empty();
    (passed, trace)
}

/// Rerank a set of candidates by cosine similarity against the query vector.
/// Cards that fail the facet filter are kept in the output but flagged.
pub fn rerank(
    required: &CapabilityFacets,
    query_vector: &[f32],
    candidates: Vec<CapabilityCard>,
) -> Vec<ScoredCandidate> {
    let mut scored: Vec<ScoredCandidate> = candidates
        .into_iter()
        .map(|card| {
            let (passed, trace) = card
                .facets
                .as_ref()
                .map(|f| facet_filter(required, f))
                .unwrap_or((false, vec!["missing facets on candidate card".into()]));
            let score = card
                .embedding
                .as_ref()
                .map(|e| cosine(query_vector, &e.vector))
                .unwrap_or(f32::NEG_INFINITY);
            ScoredCandidate {
                card,
                score,
                passed_filter: passed,
                filter_trace: trace,
            }
        })
        .collect();
    scored.sort_by(|a, b| {
        b.passed_filter.cmp(&a.passed_filter).then(
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    scored
}

#[cfg(test)]
mod tests {
    use super::*;
    use kite_mesh_proto::AgentStatus;

    fn facets(sandbox: &str) -> CapabilityFacets {
        CapabilityFacets {
            tools: vec!["python_exec".into()],
            ontologies: vec!["code-execution".into()],
            sandbox_level: sandbox.into(),
            network_access: "egress-limited".into(),
            gpu_class: "none".into(),
            privacy_tier: "trusted".into(),
            region: "local".into(),
            pricing_band: "free".into(),
            trust_tier: "local".into(),
            status: AgentStatus::Available as i32,
        }
    }

    #[test]
    fn filter_pass_and_fail() {
        let required = facets("container");
        let (pass, trace) = facet_filter(&required, &facets("container"));
        assert!(pass);
        assert!(trace.is_empty());

        let (pass, trace) = facet_filter(&required, &facets("none"));
        assert!(!pass);
        assert!(trace.iter().any(|t| t.contains("sandbox_level")));
    }
}
