use reqwest::{Method, Url};
use serde::de::DeserializeOwned;
use uuid::Uuid;

use oneclient_auth::MinecraftAccount;
use oneclient_net::RequestError;

use crate::api::send_to;
use crate::models::{
    BlockedPlayer, CreateGroupRequest, EditMessageRequest, Friend, FriendRequest, GroupMessage,
    GroupSummary, IncomingSessionInvite, MAX_PAGE_SIZE, MAX_RESOLVE_BATCH, PlayerStatus, ResolveRequest, ResolveResponse,
    ResolvedPlayer, SendMessageRequest, StatusBody,
};
use crate::{PlusClient, PlusError, base_url, session};

impl PlusClient {
    pub async fn groups(&self) -> Result<Vec<GroupSummary>, PlusError> {
        self.json(Method::GET, self.url("/groups")?, None).await
    }

    pub async fn open_direct_message(&self, player: Uuid) -> Result<GroupSummary, PlusError> {
        self.json(Method::POST, self.url(&format!("/groups/dm/{player}"))?, None)
            .await
    }

    pub async fn create_group(
        &self,
        name: &str,
        members: &[Uuid],
    ) -> Result<GroupSummary, PlusError> {
        let body = serialize(&CreateGroupRequest { name, members })?;
        self.json(Method::POST, self.url("/groups")?, Some(body))
            .await
    }

    pub async fn add_member(&self, group_id: i32, player: Uuid) -> Result<(), PlusError> {
        self.unit(
            Method::POST,
            self.url(&format!("/groups/{group_id}/members/{player}"))?,
            None,
        )
        .await
    }

    pub async fn remove_member(&self, group_id: i32, player: Uuid) -> Result<(), PlusError> {
        self.unit(
            Method::DELETE,
            self.url(&format!("/groups/{group_id}/members/{player}"))?,
            None,
        )
        .await
    }

    pub async fn messages(
        &self,
        group_id: i32,
        before: Option<i64>,
        limit: Option<u64>,
    ) -> Result<Vec<GroupMessage>, PlusError> {
        let mut url = self.url(&format!("/groups/{group_id}/messages"))?;
        {
            let mut query = url.query_pairs_mut();
            if let Some(before) = before {
                query.append_pair("before", &before.to_string());
            }
            if let Some(limit) = limit {
                query.append_pair("limit", &limit.min(MAX_PAGE_SIZE).to_string());
            }
        }

        self.json(Method::GET, url, None).await
    }

    pub async fn send_message(
        &self,
        group_id: i32,
        content: &str,
        idempotency_key: Uuid,
    ) -> Result<GroupMessage, PlusError> {
        let body = serialize(&SendMessageRequest {
            content,
            idempotency_key,
        })?;

        self.json(
            Method::POST,
            self.url(&format!("/groups/{group_id}/messages"))?,
            Some(body),
        )
        .await
    }

    pub async fn edit_message(
        &self,
        group_id: i32,
        message_id: i64,
        content: &str,
    ) -> Result<GroupMessage, PlusError> {
        let body = serialize(&EditMessageRequest { content })?;

        self.json(
            Method::PATCH,
            self.url(&format!("/groups/{group_id}/messages/{message_id}"))?,
            Some(body),
        )
        .await
    }

    pub async fn delete_message(&self, group_id: i32, message_id: i64) -> Result<(), PlusError> {
        self.unit(
            Method::DELETE,
            self.url(&format!("/groups/{group_id}/messages/{message_id}"))?,
            None,
        )
        .await
    }

    pub async fn mark_read(&self, group_id: i32, message_id: i64) -> Result<(), PlusError> {
        self.unit(
            Method::POST,
            self.url(&format!("/groups/{group_id}/read/{message_id}"))?,
            None,
        )
        .await
    }

    pub async fn friends(&self) -> Result<Vec<Friend>, PlusError> {
        self.json(Method::GET, self.url("/social/friends")?, None)
            .await
    }

    pub async fn remove_friend(&self, player: Uuid) -> Result<(), PlusError> {
        self.unit(
            Method::DELETE,
            self.url(&format!("/social/friends/{player}"))?,
            None,
        )
        .await
    }

    pub async fn incoming_requests(&self) -> Result<Vec<FriendRequest>, PlusError> {
        self.json(Method::GET, self.url("/social/requests/incoming")?, None)
            .await
    }

    pub async fn outgoing_requests(&self) -> Result<Vec<FriendRequest>, PlusError> {
        self.json(Method::GET, self.url("/social/requests/outgoing")?, None)
            .await
    }

    pub async fn send_friend_request(
        &self,
        player: Uuid,
    ) -> Result<Option<FriendRequest>, PlusError> {
        self.json(
            Method::POST,
            self.url(&format!("/social/requests/{player}"))?,
            None,
        )
        .await
    }

    pub async fn accept_request(&self, request_id: i32) -> Result<(), PlusError> {
        self.unit(
            Method::POST,
            self.url(&format!("/social/requests/{request_id}/accept"))?,
            None,
        )
        .await
    }

    pub async fn decline_request(&self, request_id: i32) -> Result<(), PlusError> {
        self.unit(
            Method::POST,
            self.url(&format!("/social/requests/{request_id}/decline"))?,
            None,
        )
        .await
    }

    pub async fn incoming_session_invites(&self) -> Result<Vec<IncomingSessionInvite>, PlusError> {
        self.json(Method::GET, self.url("/sessions/invites/incoming")?, None)
            .await
    }

    pub async fn decline_session_invite(&self, invite_id: i32) -> Result<(), PlusError> {
        self.unit(
            Method::POST,
            self.url(&format!("/sessions/invites/{invite_id}/decline"))?,
            None,
        )
        .await
    }

    pub async fn cancel_request(&self, request_id: i32) -> Result<(), PlusError> {
        self.unit(
            Method::DELETE,
            self.url(&format!("/social/requests/{request_id}"))?,
            None,
        )
        .await
    }

    pub async fn blocked(&self) -> Result<Vec<BlockedPlayer>, PlusError> {
        self.json(Method::GET, self.url("/social/blocked")?, None)
            .await
    }

    pub async fn block(&self, player: Uuid) -> Result<(), PlusError> {
        self.unit(
            Method::POST,
            self.url(&format!("/social/blocked/{player}"))?,
            None,
        )
        .await
    }

    pub async fn unblock(&self, player: Uuid) -> Result<(), PlusError> {
        self.unit(
            Method::DELETE,
            self.url(&format!("/social/blocked/{player}"))?,
            None,
        )
        .await
    }

    pub async fn status(&self) -> Result<PlayerStatus, PlusError> {
        let body: StatusBody = self
            .json(Method::GET, self.url("/account/status")?, None)
            .await?;
        Ok(body.status)
    }

    pub async fn set_status(&self, status: PlayerStatus) -> Result<(), PlusError> {
        let body = serialize(&StatusBody { status })?;
        self.unit(Method::PUT, self.url("/account/status")?, Some(body))
            .await
    }

    pub async fn resolve_players(&self, ids: &[Uuid]) -> Result<Vec<ResolvedPlayer>, PlusError> {
        let mut resolved = Vec::with_capacity(ids.len());

        for chunk in ids.chunks(MAX_RESOLVE_BATCH) {
            let body = serialize(&ResolveRequest { ids: chunk })?;
            let page: ResolveResponse = self
                .json(Method::POST, self.url("/players/resolve")?, Some(body))
                .await?;
            resolved.extend(page.players);
        }

        Ok(resolved)
    }

    pub async fn lookup_username(&self, username: &str) -> Result<Option<Uuid>, PlusError> {
        let mut url = self.url("/players/by-username")?;
        url.path_segments_mut()
            .map_err(|()| url::ParseError::RelativeUrlWithCannotBeABaseBase)?
            .push(username);

        match self.json::<ResolvedPlayer>(Method::GET, url, None).await {
            Ok(player) => Ok(Some(player.id)),
            Err(err) if err.is_not_found() => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn url(&self, path: &str) -> Result<Url, PlusError> {
        Ok(Url::parse(&format!("{}{path}", base_url()))?)
    }

    async fn account(&self) -> Result<MinecraftAccount, PlusError> {
        session::current_account(&self.auth)
            .await?
            .ok_or(PlusError::NoAccount)
    }

    async fn json<T: DeserializeOwned>(
        &self,
        method: Method,
        url: Url,
        body: Option<serde_json::Value>,
    ) -> Result<T, PlusError> {
        let account = self.account().await?;
        Ok(send_to(&account, method, url.as_str(), body.as_ref())
            .await?
            .json::<T>()
            .await?)
    }

    async fn unit(
        &self,
        method: Method,
        url: Url,
        body: Option<serde_json::Value>,
    ) -> Result<(), PlusError> {
        let account = self.account().await?;
        send_to(&account, method, url.as_str(), body.as_ref()).await?;
        Ok(())
    }
}

fn serialize<T: serde::Serialize>(value: &T) -> Result<serde_json::Value, PlusError> {
    serde_json::to_value(value)
        .map_err(|err| PlusError::Request(RequestError::SerializeError(err)))
}
