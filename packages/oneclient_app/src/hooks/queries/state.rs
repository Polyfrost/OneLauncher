//! Reading a query's state without spelling out the match every time.
//!
//! `QueryStateData` has three variants and the useful cases straddle two of
//! them: a query that is refetching still holds the previous value, and the UI
//! wants to keep showing it rather than flash a spinner. Written out by hand
//! that is a five-line match repeated over a hundred times, with enough small
//! divergences (does a settled `Err` read as empty or as absent? does
//! "loading" include a refetch?) that the differences were impossible to see.

use std::fmt::Display;

use freya::query::{QueryCapability, QueryStateData, UseQuery};

/// The query's value, keeping the previous one visible across a refetch.
///
/// `None` means there has never been a successful result: either the query
/// has not finished yet or the only outcome so far is an error. Callers that
/// treat "failed" the same as "empty" finish with `.unwrap_or_default()`.
pub fn settled_or_loading<Q>(query: &UseQuery<Q>) -> Option<Q::Ok>
where
    Q: QueryCapability,
    Q::Ok: Clone,
{
    let reader = query.read();
    let state = reader.state();
    state.ok().cloned()
}

/// The error from a settled query, rendered for display.
///
/// Not reported while loading: a refetch that is still in flight has not
/// failed yet, even if the previous attempt did.
pub fn query_error<Q>(query: &UseQuery<Q>) -> Option<String>
where
    Q: QueryCapability,
    Q::Err: Display,
{
    let reader = query.read();
    let state = reader.state();
    match &*state {
        QueryStateData::Settled { res: Err(err), .. } => Some(err.to_string()),
        _ => None,
    }
}

/// Whether the query is working with nothing to show yet.
///
/// A refetch that still holds a previous value is *not* loading by this
/// definition, since the UI has something to render and a spinner would be a
/// step backwards. Use [`query_is_busy`] where the distinction matters.
pub fn query_is_loading<Q: QueryCapability>(query: &UseQuery<Q>) -> bool {
    let reader = query.read();
    let state = reader.state();
    matches!(
        &*state,
        QueryStateData::Pending | QueryStateData::Loading { res: None }
    )
}

/// Whether the query is working at all, including a refetch over a value that
/// is already on screen.
pub fn query_is_busy<Q: QueryCapability>(query: &UseQuery<Q>) -> bool {
    let reader = query.read();
    let state = reader.state();
    matches!(
        &*state,
        QueryStateData::Pending | QueryStateData::Loading { .. }
    )
}
