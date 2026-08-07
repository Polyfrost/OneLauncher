//! Query state readers The useful cases straddle two `QueryStateData`
//! variants a refetch still holds the previous value the UI wants to show

use std::fmt::Display;

use freya::query::{QueryCapability, QueryStateData, UseQuery};

/// Keeps the previous value visible across a refetch `None` means there has
/// never been a successful result (not finished yet or only errors so far)
pub fn settled_or_loading<Q>(query: &UseQuery<Q>) -> Option<Q::Ok>
where
    Q: QueryCapability,
    Q::Ok: Clone,
{
    let reader = query.read();
    let state = reader.state();
    state.ok().cloned()
}

/// Not reported while loading a refetch in flight has not failed yet even if
/// the previous attempt did
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

/// A refetch still holding a previous value is *not* loading here Use
/// [`query_is_busy`] where the distinction matters
pub fn query_is_loading<Q: QueryCapability>(query: &UseQuery<Q>) -> bool {
    let reader = query.read();
    let state = reader.state();
    matches!(
        &*state,
        QueryStateData::Pending | QueryStateData::Loading { res: None }
    )
}

pub fn query_is_busy<Q: QueryCapability>(query: &UseQuery<Q>) -> bool {
    let reader = query.read();
    let state = reader.state();
    matches!(
        &*state,
        QueryStateData::Pending | QueryStateData::Loading { .. }
    )
}
