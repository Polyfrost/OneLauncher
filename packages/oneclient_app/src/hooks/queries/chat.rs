use std::time::Duration;

use freya::query::{QueriesStorage, Query, QueryCapability, UseQuery, use_query};
use oneclient_polyplus::{BlockedPlayer, Friend, FriendRequest, PlusError};

const ROSTER_STALE: Duration = Duration::from_secs(60);
const ROSTER_CLEAN: Duration = Duration::from_secs(10 * 60);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FriendRequests {
    pub incoming: Vec<FriendRequest>,
    pub outgoing: Vec<FriendRequest>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FriendsQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FriendsKeys;

impl QueryCapability for FriendsQuery {
    type Ok = Vec<Friend>;
    type Err = PlusError;
    type Keys = FriendsKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let Some(client) = oneclient_polyplus::client() else {
            return Ok(Vec::new());
        };

        client.friends().await
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FriendRequestsQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FriendRequestsKeys;

impl QueryCapability for FriendRequestsQuery {
    type Ok = FriendRequests;
    type Err = PlusError;
    type Keys = FriendRequestsKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let Some(client) = oneclient_polyplus::client() else {
            return Ok(FriendRequests::default());
        };

        Ok(FriendRequests {
            incoming: client.incoming_requests().await?,
            outgoing: client.outgoing_requests().await?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockedPlayersQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockedPlayersKeys;

impl QueryCapability for BlockedPlayersQuery {
    type Ok = Vec<BlockedPlayer>;
    type Err = PlusError;
    type Keys = BlockedPlayersKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let Some(client) = oneclient_polyplus::client() else {
            return Ok(Vec::new());
        };

        client.blocked().await
    }
}

pub fn use_friends() -> UseQuery<FriendsQuery> {
    use_query(
        Query::new(FriendsKeys, FriendsQuery)
            .stale_time(ROSTER_STALE)
            .clean_time(ROSTER_CLEAN),
    )
}

pub fn use_friend_requests() -> UseQuery<FriendRequestsQuery> {
    use_query(
        Query::new(FriendRequestsKeys, FriendRequestsQuery)
            .stale_time(ROSTER_STALE)
            .clean_time(ROSTER_CLEAN),
    )
}

pub fn use_blocked_players() -> UseQuery<BlockedPlayersQuery> {
    use_query(
        Query::new(BlockedPlayersKeys, BlockedPlayersQuery)
            .stale_time(ROSTER_STALE)
            .clean_time(ROSTER_CLEAN),
    )
}

pub async fn invalidate_chat_queries() {
    QueriesStorage::<FriendsQuery>::invalidate_matching(FriendsKeys).await;
    QueriesStorage::<FriendRequestsQuery>::invalidate_matching(FriendRequestsKeys).await;
    QueriesStorage::<BlockedPlayersQuery>::invalidate_matching(BlockedPlayersKeys).await;
}
