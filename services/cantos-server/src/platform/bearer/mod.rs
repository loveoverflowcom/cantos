//! Cantos opaque credentials reuse the existing OIDC verifier, registry and provider adapter.
//! Clients never receive provider artifacts. Transactions contain no network calls.
pub mod client;
pub mod core;
pub mod credential;
pub(crate) mod postgres;
use crate::platform::{
    oidc::{
        config::OidcConfig,
        core::{pkce_challenge, AuthError},
        provider::Provider,
    },
    postgres::Store,
};
use cantos_contracts::bearer::{
    CancelRequest, ExchangeRequest, Flow, FlowRequest, RefreshRequest, Session, TokenPair,
};
use client::Client;

pub struct BearerIdentity {
    config: OidcConfig,
    provider: Provider,
    client: Client,
    slots: tokio::sync::Semaphore,
}
impl BearerIdentity {
    pub(crate) fn new(mut config: OidcConfig, client: Client) -> Result<Self, AuthError> {
        if config.client_id != client.name() {
            return Err(AuthError::Configuration);
        }
        if client != Client::TheatreNative {
            config.redirect = config
                .redirect
                .replace("/auth/callback", "/auth/bearer/callback");
        }
        Ok(Self {
            provider: Provider::new(config.clone())?,
            config,
            client,
            slots: tokio::sync::Semaphore::new(3),
        })
    }
    pub(crate) fn accepts_callback_issuer(&self, issuer: Option<&str>) -> bool {
        issuer.is_none_or(|v| v == self.config.issuer)
    }
    pub async fn authorization_failed(
        &self,
        store: &Store,
        state: &str,
    ) -> Result<String, AuthError> {
        if self.client == Client::TheatreNative || !credential::secret(state) {
            return Err(AuthError::State);
        }
        let (id, client_state) = postgres::reject_callback(store, &self.config, state).await?;
        let mut url = url::Url::parse(&format!("{}/auth/bearer/complete", self.config.web_origin))
            .map_err(|_| AuthError::Configuration)?;
        url.query_pairs_mut().extend_pairs([
            ("flow_id", id.as_str()),
            ("state", client_state.as_str()),
            ("error", "authorization_failed"),
        ]);
        Ok(url.into())
    }
    pub fn client(&self) -> Client {
        self.client
    }
    fn authorize_subject(&self, store: &Store, subject: &str) -> Result<(), AuthError> {
        if self.client == Client::Studio {
            store
                .studio_oidc()
                .ok_or(AuthError::Configuration)?
                .authorize_bearer_subject(subject)?;
        }
        Ok(())
    }
    pub async fn begin(&self, store: &Store, request: FlowRequest) -> Result<Flow, AuthError> {
        if !credential::protocol(&request.state)
            || !credential::protocol(&request.nonce)
            || !credential::challenge(&request.code_challenge)
        {
            return Err(AuthError::State);
        }
        let id = uuid::Uuid::new_v4().to_string();
        let provider_state = if self.client == Client::TheatreNative {
            request.state.clone()
        } else {
            random()?
        };
        let verifier = random()?;
        let challenge = if self.client == Client::TheatreNative {
            request.code_challenge.clone()
        } else {
            pkce_challenge(&verifier)
        };
        // Only native may choose a (closed, operator-admitted) callback profile.
        if self.client != Client::TheatreNative && request.callback_profile.is_some() {
            return Err(AuthError::State);
        }
        let redirect = if self.client == Client::TheatreNative {
            self.config.native_redirect(request.callback_profile)?
        } else {
            self.config.redirect.as_str()
        };
        let authorization_url = self
            .provider
            .authorization_url_for_redirect(
                &provider_state,
                &request.nonce,
                &challenge,
                redirect,
                now()?,
            )
            .await?;
        let authorization_url = if request.reauthenticate {
            let mut url =
                url::Url::parse(&authorization_url).map_err(|_| AuthError::Configuration)?;
            url.query_pairs_mut()
                .extend_pairs([("prompt", "login"), ("max_age", "0")]);
            url.into()
        } else {
            authorization_url
        };
        postgres::begin(
            store,
            &self.config,
            &id,
            &provider_state,
            &verifier,
            redirect,
            request,
        )
        .await?;
        Ok(Flow {
            flow_id: id,
            authorization_url,
        })
    }
    /// Web provider callback: returns a short PKCE-bound handoff, never an access credential.
    pub async fn callback(
        &self,
        store: &Store,
        state: &str,
        code: &str,
    ) -> Result<String, AuthError> {
        if self.client == Client::TheatreNative || !credential::secret(state) || !code_valid(code) {
            return Err(AuthError::State);
        }
        let handoff = random()?;
        let flow = postgres::consume_callback(store, &self.config, state).await?;
        let exchange = self
            .provider
            .exchange(code, &flow.verifier, &flow.nonce, now)
            .await?;
        self.complete_exchange(store, &flow.id, exchange, &handoff, &self.config.redirect)
            .await?;
        let mut url = url::Url::parse(&format!("{}/auth/bearer/complete", self.config.web_origin))
            .map_err(|_| AuthError::Configuration)?;
        url.query_pairs_mut().extend_pairs([
            ("flow_id", flow.id.as_str()),
            ("state", flow.state.as_str()),
            ("code", handoff.as_str()),
        ]);
        Ok(url.into())
    }
    pub async fn exchange(
        &self,
        store: &Store,
        flow: &str,
        request: ExchangeRequest,
    ) -> Result<TokenPair, AuthError> {
        validate_exchange(flow, &request)?;
        let redirect = if self.client == Client::TheatreNative {
            let (nonce, redirect) = postgres::consume_native(
                store,
                &self.config,
                flow,
                &request.state,
                &pkce_challenge(&request.code_verifier),
            )
            .await?;
            let exchange = self
                .provider
                .exchange_for_redirect(
                    &request.code,
                    &request.code_verifier,
                    &nonce,
                    &redirect,
                    now,
                )
                .await?;
            self.complete_exchange(store, flow, exchange, "", &redirect)
                .await?;
            redirect
        } else {
            self.config.redirect.clone()
        };
        let result = match Secrets::new(self.client) {
            Ok(secrets) => {
                postgres::issue(
                    store,
                    &self.config,
                    self.client,
                    flow,
                    &request,
                    secrets,
                    &redirect,
                )
                .await
            }
            Err(error) => Err(error),
        };
        if result.is_err() {
            // This proof-bound cleanup is safe even when issue committed but its response was lost.
            // A wrong state/verifier cannot use an exchange refusal to cancel somebody else's flow.
            let _ = self
                .cancel(
                    store,
                    flow,
                    CancelRequest {
                        state: request.state,
                        code_verifier: request.code_verifier,
                    },
                )
                .await;
        }
        result
    }
    async fn complete_exchange(
        &self,
        store: &Store,
        flow: &str,
        exchange: crate::platform::oidc::provider::Exchange,
        handoff: &str,
        redirect: &str,
    ) -> Result<(), AuthError> {
        let result = async {
            self.authorize_subject(store, exchange.proof.key().subject())?;
            let lifetime = core::issue(
                self.client.policy(),
                now()?,
                exchange.proof.expires(),
                exchange.access_expiry,
            )
            .map_err(refusal)?;
            postgres::complete_callback(
                store,
                &self.config,
                flow,
                &exchange,
                lifetime.absolute,
                handoff,
                redirect,
            )
            .await
        }
        .await;
        if result.is_err() {
            match postgres::abandon_exchange(
                store,
                &self.config,
                flow,
                &exchange.access_token,
                redirect,
            )
            .await
            {
                Ok(receipt) => {
                    let _ = self.finish_revoke(store, receipt).await;
                }
                Err(_) => {
                    // A database outage cannot mint local authority. Still contain the upstream
                    // artifact if possible; remote receipt persistence is necessarily unavailable.
                    if let Ok(time) = now() {
                        let _ = self.provider.revoke(&exchange.access_token, time).await;
                    }
                }
            }
        }
        result
    }
    pub async fn cancel(
        &self,
        store: &Store,
        flow: &str,
        request: CancelRequest,
    ) -> Result<bool, AuthError> {
        if uuid::Uuid::parse_str(flow).is_err()
            || !credential::protocol(&request.state)
            || !credential::protocol(&request.code_verifier)
        {
            return Err(AuthError::State);
        }
        let cancellation = postgres::cancel(
            store,
            &self.config,
            flow,
            &request.state,
            &pkce_challenge(&request.code_verifier),
        )
        .await?;
        match cancellation {
            postgres::Cancellation::Complete => Ok(true),
            postgres::Cancellation::Pending => Ok(false),
            postgres::Cancellation::Revoke(receipt) => {
                self.finish_revoke(store, Some(receipt)).await
            }
        }
    }
    pub async fn authenticate(&self, store: &Store, token: &str) -> Result<Session, AuthError> {
        if credential::access_client(token) != Some(self.client) {
            return Err(AuthError::InvalidToken);
        }
        let credential = postgres::credential(store, &self.config, token).await?;
        self.authorize_subject(store, &credential.subject)?;
        self.provider
            .introspect(&credential.provider_access, &credential.subject, now)
            .await?;
        // No cached authority survives local logout, disable or expiry during the provider call.
        Ok(postgres::credential(store, &self.config, token)
            .await?
            .session)
    }
    pub async fn refresh(
        &self,
        store: &Store,
        request: RefreshRequest,
    ) -> Result<TokenPair, AuthError> {
        validate_refresh(&request)?;
        let credential = postgres::refresh_credential(store, &self.config, &request).await?;
        self.authorize_subject(store, &credential.subject)?;
        self.provider
            .introspect(&credential.provider_access, &credential.subject, now)
            .await?;
        postgres::rotate(
            store,
            &self.config,
            self.client,
            request,
            Secrets::new(self.client)?,
        )
        .await
    }
    pub async fn revoke_refresh(
        &self,
        store: &Store,
        request: RefreshRequest,
    ) -> Result<bool, AuthError> {
        validate_refresh(&request)?;
        let access = postgres::revoke(
            store,
            &self.config,
            &request.refresh_token,
            Some(&request.session_id),
            true,
        )
        .await?;
        self.finish_revoke(store, access).await
    }
    pub async fn logout(&self, store: &Store, token: &str) -> Result<bool, AuthError> {
        if credential::access_client(token) != Some(self.client) {
            return Err(AuthError::InvalidToken);
        }
        let access = postgres::revoke(store, &self.config, token, None, false).await?;
        self.finish_revoke(store, access).await
    }
    async fn finish_revoke(
        &self,
        store: &Store,
        receipt: Option<postgres::Revocation>,
    ) -> Result<bool, AuthError> {
        let Some(receipt) = receipt else {
            return Ok(true);
        };
        // Durable local revoke precedes best-effort remote revoke; an outage remains recorded.
        if self
            .provider
            .revoke(receipt.access(), now()?)
            .await
            .is_err()
        {
            return Ok(false);
        }
        postgres::revocation_completed(store, &receipt).await?;
        Ok(true)
    }
    pub async fn with_admission<F, Fut, T>(
        &self,
        store: &Store,
        token: &str,
        dispatch: F,
    ) -> Result<T, AuthError>
    where
        F: FnOnce(Session) -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        let _permit = tokio::time::timeout(std::time::Duration::from_secs(5), self.slots.acquire())
            .await
            .map_err(|_| AuthError::Unavailable)?
            .map_err(|_| AuthError::Unavailable)?;
        postgres::admission(store, &self.config, token, dispatch).await
    }
}
fn validate_refresh(request: &RefreshRequest) -> Result<(), AuthError> {
    if !credential::refresh_secret(&request.refresh_token)
        || uuid::Uuid::parse_str(&request.session_id).is_err()
    {
        return Err(AuthError::InvalidToken);
    }
    Ok(())
}
fn validate_exchange(flow: &str, request: &ExchangeRequest) -> Result<(), AuthError> {
    if uuid::Uuid::parse_str(flow).is_err()
        || !credential::protocol(&request.state)
        || !credential::protocol(&request.code_verifier)
        || !code_valid(&request.code)
    {
        return Err(AuthError::State);
    }
    Ok(())
}
fn code_valid(value: &str) -> bool {
    !value.is_empty() && value.len() <= 4096 && value.bytes().all(|b| b.is_ascii_graphic())
}
fn refusal(value: core::Refusal) -> AuthError {
    match value {
        core::Refusal::Expired => AuthError::Expired,
        core::Refusal::Inactive => AuthError::Inactive,
    }
}
pub(crate) fn now() -> Result<u64, AuthError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|v| v.as_secs())
        .map_err(|_| AuthError::Unavailable)
}
fn random() -> Result<String, AuthError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| AuthError::Unavailable)?;
    Ok(bytes.iter().map(|v| format!("{v:02x}")).collect())
}
struct Secrets {
    access: String,
    refresh: String,
}
impl Secrets {
    fn new(client: Client) -> Result<Self, AuthError> {
        Ok(Self {
            access: format!("{}{}", credential::access_prefix(client), random()?),
            refresh: format!("cr1_{}", random()?),
        })
    }
}
impl Store {
    pub fn bearer(&self, client: Client) -> Option<&BearerIdentity> {
        match client {
            Client::Studio => self.studio_oidc().map(|v| v.bearer()),
            Client::TheatreWeb => self.theatre_identity().map(|v| v.bearer()),
            Client::TheatreNative => self.theatre_native().map(|v| v.bearer()),
        }
    }
}
