use reqwest::{Method, Url};
use serde::de::DeserializeOwned;
use uuid::Uuid;

use oneclient_net::RequestError;

use crate::models::{
    BlockedPlayer, CreateGroupRequest, EditMessageRequest, Friend, FriendRequest, GroupMessage,
    GroupSummary, MAX_PAGE_SIZE, MAX_RESOLVE_BATCH, ResolveRequest, ResolveResponse,
    ResolvedPlayer, SendMessageRequest, SpecialChatStatus,
};
use crate::{PlusClient, PlusError, base_url};

const SNIPPET_LIMIT: usize = 200;

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

    pub async fn claim_group(&self, group_id: i32) -> Result<GroupSummary, PlusError> {
        self.json(
            Method::POST,
            self.url(&format!("/groups/{group_id}/claim"))?,
            None,
        )
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
        match self
            .json::<ResolvedPlayer>(
                Method::GET,
                self.url(&format!("/players/by-username/{username}"))?,
                None,
            )
            .await
        {
            Ok(player) => Ok(Some(player.id)),
            Err(err) if err.is_not_found() => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub async fn special_chat(&self) -> Result<SpecialChatStatus, PlusError> {
        self.json(Method::GET, self.url("/special-chat")?, None)
            .await
    }

    fn url(&self, path: &str) -> Result<Url, PlusError> {
        Ok(Url::parse(&format!("{}{path}", base_url()))?)
    }

    async fn json<T: DeserializeOwned>(
        &self,
        method: Method,
        url: Url,
        body: Option<serde_json::Value>,
    ) -> Result<T, PlusError> {
        let token = self.authorize().await?;
        let header = bearer(&token);

        let attempt = self
            .requester
            .send_json::<T>(
                method.clone(),
                url.clone(),
                body.clone(),
                &[("Authorization", header.as_str())],
            )
            .await;

        if !matches!(attempt, Err(RequestError::HttpStatus { status: 401, .. })) {
            return Ok(attempt?);
        }

        self.forget_token().await;
        let token = self.authorize().await?;
        let header = bearer(&token);

        Ok(self
            .requester
            .send_json::<T>(method, url, body, &[("Authorization", header.as_str())])
            .await?)
    }

    async fn unit(
        &self,
        method: Method,
        url: Url,
        body: Option<serde_json::Value>,
    ) -> Result<(), PlusError> {
        let token = self.authorize().await?;
        let response = self
            .requester
            .send(build(method.clone(), url.clone(), body.clone(), &token)?)
            .await?;

        let response = if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.forget_token().await;
            let token = self.authorize().await?;
            self.requester.send(build(method, url, body, &token)?).await?
        } else {
            response
        };

        if response.status().is_success() {
            return Ok(());
        }

        let status = response.status().as_u16();
        let url = response.url().to_string();
        let snippet = response.text().await.unwrap_or_default();

        Err(PlusError::Request(RequestError::HttpStatus {
            status,
            url,
            snippet: snippet.chars().take(SNIPPET_LIMIT).collect(),
        }))
    }
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

fn serialize<T: serde::Serialize>(value: &T) -> Result<serde_json::Value, PlusError> {
    serde_json::to_value(value)
        .map_err(|err| PlusError::Request(RequestError::SerializeError(err)))
}

fn build(
    method: Method,
    url: Url,
    body: Option<serde_json::Value>,
    token: &str,
) -> Result<reqwest::Request, PlusError> {
    let mut request = reqwest::Request::new(method, url);

    let mut value = reqwest::header::HeaderValue::try_from(bearer(token))
        .map_err(|err| PlusError::Request(RequestError::from(err)))?;
    value.set_sensitive(true);
    request
        .headers_mut()
        .insert(reqwest::header::AUTHORIZATION, value);

    if let Some(body) = body {
        request.headers_mut().insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        *request.body_mut() = Some(
            serde_json::to_vec(&body)
                .map_err(|err| PlusError::Request(RequestError::SerializeError(err)))?
                .into(),
        );
    }

    Ok(request)
}
