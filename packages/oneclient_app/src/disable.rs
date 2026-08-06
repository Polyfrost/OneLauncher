//! Turning a package off, and everything that has to go off with it.
//!
//! Every way to disable a package in the UI comes through here, so the warning
//! cannot be skipped by taking a different route to the same switch. The flow
//! is: work out the impact, and if there is none, disable straight away — the
//! common case must not cost the user a dialog. If other packages require it,
//! the caller is handed a [`PendingDisable`] to put in front of them, and
//! nothing is written until they confirm.

use std::time::Duration;

use freya::prelude::{State, WritableUtils, spawn_forever};
pub use oneclient_content::packages::DisableRoot;
use oneclient_content::packages::PackageStore;
use oneclient_db::models::ClusterId;

use crate::hooks::invalidate_cluster_queries;
use crate::launcher;

/// How long the dependency backfill may hold up a toggle.
///
/// Only packages installed before the launcher recorded dependency lists need
/// it, and only once each, but that first toggle should not sit on an
/// unreachable provider. Past the budget the impact is worked out from what is
/// stored, which is the same answer an offline launcher gives.
const BACKFILL_BUDGET: Duration = Duration::from_secs(5);

/// A bundle-provided row the cluster has not installed. Its state is an
/// override on the bundle rather than a flag on an artifact, so it is carried
/// separately from the hashes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundlePackage {
    pub bundle_name: String,
    pub package_id: String,
}

/// What the user asked to switch off.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisableRequest {
    pub cluster_id: ClusterId,
    /// What to call it on screen.
    pub name: String,
    pub root: DisableRoot,
    pub bundle: Option<BundlePackage>,
}

/// A disable waiting on the user: what the modal says, and what applying it
/// writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingDisable {
    pub request: DisableRequest,
    /// Packages that require the target and go off with it, by display name.
    pub required: Vec<String>,
    /// Packages that only optionally depend on it. Listed, then left alone.
    pub optional: Vec<String>,
    /// Every artifact the confirmation switches off, the target first.
    hashes: Vec<String>,
}

/// The entry point every "turn this off" control uses.
///
/// The resolve is a database round trip and sometimes a provider one, so it
/// runs off the event handler. `spawn_forever` rather than `spawn`: the row
/// that was pressed is free to scroll out of the list, and its scope going with
/// it must not cancel the answer.
pub fn request(request: DisableRequest, mut pending: State<Option<PendingDisable>>) {
    spawn_forever(async move {
        match resolve(request).await {
            // Nothing else was affected, so it is already off.
            Ok(None) => invalidate_cluster_queries().await,
            Ok(Some(confirm)) => pending.set(Some(confirm)),
            Err(err) => {
                tracing::warn!(%err, "failed to work out what a disable would affect");
                if let Ok(state) = launcher::state() {
                    state
                        .services
                        .events
                        .notify("Could not disable")
                        .body(err.to_string())
                        .error()
                        .send();
                }
            }
        }
    });
}

/// Works out what a disable would take with it, and applies it immediately when
/// the answer is nothing.
///
/// `Ok(None)` means it is already done.
pub async fn resolve(request: DisableRequest) -> anyhow::Result<Option<PendingDisable>> {
    let state = launcher::state()?;
    let content = state.services.content();

    // A cluster whose packages predate the dependency table has nothing to walk
    // until this has run. It is a no-op from the second call on.
    if let Err(_elapsed) = tokio::time::timeout(
        BACKFILL_BUDGET,
        oneclient_core::ensure_cluster_dependencies(request.cluster_id, &content),
    )
    .await
    {
        tracing::debug!(
            cluster_id = request.cluster_id,
            "dependency backfill ran out of time; using what is stored"
        );
    }

    let impact =
        oneclient_core::disable_impact(request.cluster_id, &request.root, &content).await?;

    let mut hashes = match &request.root {
        DisableRoot::Artifact(hash) => vec![hash.clone()],
        // Nothing of it is installed, so there is no flag to write — only the
        // bundle override the caller carries.
        DisableRoot::Package(..) => Vec::new(),
    };
    hashes.extend(impact.required_hashes());

    if impact.is_empty() {
        apply_now(&request, &hashes, &content).await?;
        return Ok(None);
    }

    Ok(Some(PendingDisable {
        request,
        required: impact.required.into_iter().map(|dep| dep.name).collect(),
        optional: impact.optional.into_iter().map(|dep| dep.name).collect(),
        hashes,
    }))
}

/// Applies a disable the user has confirmed.
pub async fn apply(pending: &PendingDisable) -> anyhow::Result<()> {
    let state = launcher::state()?;
    apply_now(&pending.request, &pending.hashes, &state.services.content()).await
}

async fn apply_now(
    request: &DisableRequest,
    hashes: &[String],
    content: &oneclient_content::ContentCtx,
) -> anyhow::Result<()> {
    let overrides: Vec<(String, String)> = request
        .bundle
        .iter()
        .map(|bundle| (bundle.bundle_name.clone(), bundle.package_id.clone()))
        .collect();

    PackageStore::disable_artifacts(request.cluster_id, hashes, &overrides, content).await?;
    Ok(())
}

/// "Sodium is needed by other packages"
pub fn title(pending: &PendingDisable) -> String {
    if pending.required.is_empty() {
        format!("Other packages use {}", pending.request.name)
    } else {
        format!("{} is needed by other packages", pending.request.name)
    }
}

pub fn summary(pending: &PendingDisable) -> String {
    let name = &pending.request.name;

    if pending.required.is_empty() {
        return match pending.optional.len() {
            1 => format!(
                "One installed package optionally depends on {name}. It runs without it, so it stays enabled."
            ),
            count => format!(
                "{count} installed packages optionally depend on {name}. They run without it, so they stay enabled."
            ),
        };
    }

    let tail =
        "A package left without a dependency it requires usually stops the game from starting.";
    match pending.required.len() {
        1 => format!(
            "One installed package requires {name}. Turning it off turns that one off too. {tail}"
        ),
        count => format!(
            "{count} installed packages require {name}. Turning it off turns them off too. {tail}"
        ),
    }
}

/// The heading over the packages that go off with it.
pub fn disabled_heading(pending: &PendingDisable) -> String {
    format!(
        "Will be disabled with {} ({})",
        pending.request.name,
        pending.required.len()
    )
}

/// The heading over the packages that are only mentioned.
pub fn optional_heading(pending: &PendingDisable) -> String {
    format!(
        "Optionally depends on {} ({}) — stays enabled",
        pending.request.name,
        pending.optional.len()
    )
}

/// "Disable all 4" — the count includes the package the user actually toggled,
/// because that is what leaves the cluster.
pub fn confirm_label(pending: &PendingDisable) -> String {
    if pending.required.is_empty() {
        return "Disable anyway".to_string();
    }

    format!("Disable all {}", pending.required.len() + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending(required: &[&str], optional: &[&str]) -> PendingDisable {
        PendingDisable {
            request: DisableRequest {
                cluster_id: 1,
                name: "Sodium".to_string(),
                root: DisableRoot::Artifact("hash".to_string()),
                bundle: None,
            },
            required: required.iter().map(|n| (*n).to_string()).collect(),
            optional: optional.iter().map(|n| (*n).to_string()).collect(),
            hashes: vec!["hash".to_string()],
        }
    }

    #[test]
    fn the_confirmation_counts_the_package_being_toggled() {
        let pending = pending(&["Sodium Extra", "Reese's Sodium Options"], &[]);
        assert_eq!(confirm_label(&pending), "Disable all 3");
    }

    #[test]
    fn an_only_optional_impact_never_claims_anything_is_required() {
        let pending = pending(&[], &["Iris"]);

        assert_eq!(confirm_label(&pending), "Disable anyway");
        let summary = summary(&pending);
        assert!(summary.contains("optionally depends on"), "{summary}");
        assert!(summary.contains("stays enabled"), "{summary}");
        assert!(!summary.contains("requires"), "{summary}");
    }

    #[test]
    fn one_dependent_reads_as_one() {
        let summary = summary(&pending(&["Iris"], &[]));
        assert!(
            summary.starts_with("One installed package requires Sodium."),
            "{summary}"
        );
    }

    #[test]
    fn several_dependents_read_as_several() {
        let summary = summary(&pending(&["Iris", "Sodium Extra"], &[]));
        assert!(
            summary.starts_with("2 installed packages require Sodium."),
            "{summary}"
        );
    }
}
