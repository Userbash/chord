//! Modular Tauri v2 plugin (`search`) encapsulating local fixture search, remote guild message search, and BFF search IPC commands.
//!
//! Provides `search_page`, `search_remote_with_api`, `search_remote_with_token`,
//! `search_remote`, `search_bff_with_client`, `search_bff_cached_with_client`,
//! and `parse_search_query` via `tauri::plugin::Builder::new("search")`.

use std::sync::Arc;

use chord_client::auth::{AuthBackend, AuthError};
use chord_client::data::{self, DataError, SearchFilters, SearchPage};
use chord_client::search::{
    DiscordSearch, RemoteSearch, SearchError, SearchParams, SearchResponse,
};
use chord_client::search_bff::{
    self, SearchBffClient, SearchBffError, SearchBffQuery, SearchBffResponse, SearchBffScope,
};

async fn token_or_search<F>(work: F) -> Result<String, SearchError>
where
    F: FnOnce() -> Result<Option<String>, AuthError> + Send + 'static,
{
    let joined = tokio::task::spawn_blocking(work).await;
    if let Ok(Ok(Some(token))) = &joined {
        if !token.trim().is_empty() {
            return Ok(token.clone());
        }
    }
    if let Some(live_token) = data::resolve_live_token().filter(|t| !t.trim().is_empty()) {
        return Ok(live_token);
    }
    match joined {
        Ok(Ok(Some(token))) if !token.trim().is_empty() => Ok(token),
        Ok(Ok(_)) => Err(SearchError::MissingToken),
        Ok(Err(_)) | Err(_) => Err(SearchError::Storage),
    }
}

/// Searches local fixture messages using R-10 filters (`filters`) and cursor pagination (`cursor`, `limit`).
///
/// # Errors
/// Returns [`DataError`] if the fixtures root cannot be resolved or any filter, cursor, or limit parameter is invalid.
#[allow(dead_code)]
#[inline]
pub fn search_page(
    query: String,
    filters: Option<SearchFilters>,
    cursor: Option<String>,
    limit: Option<usize>,
) -> Result<SearchPage, DataError> {
    let root = data::fixtures_root()?;
    let filters = filters.unwrap_or_default();
    data::search_page(&root, &query, &filters, cursor.as_deref(), limit)
}

/// Executes a remote guild message search (`GET /guilds/{guild_id}/messages/search`) using an initialized [`RemoteSearch`] client.
///
/// # Errors
/// Returns [`SearchError`] if `guild_id` or `params` are invalid, or if the REST request fails.
#[allow(dead_code)]
#[inline]
pub async fn search_remote_with_api<S: RemoteSearch + ?Sized>(
    api: &S,
    guild_id: &str,
    params: Option<SearchParams>,
) -> Result<SearchResponse, SearchError> {
    api.search(guild_id, &params.unwrap_or_default()).await
}

/// Executes a remote guild message search (`GET /guilds/{guild_id}/messages/search`) using a pre-resolved user token.
///
/// # Errors
/// Returns [`SearchError`] if the token is missing/empty, `guild_id` or `params` are invalid, or the REST request fails.
#[allow(dead_code)]
#[inline]
pub async fn search_remote_with_token(
    token: &str,
    guild_id: &str,
    params: Option<SearchParams>,
) -> Result<SearchResponse, SearchError> {
    let api = DiscordSearch::new(token)?;
    search_remote_with_api(&api, guild_id, params).await
}

/// Executes a remote guild message search (`GET /guilds/{guild_id}/messages/search`), reading the user token from [`AuthBackend`].
///
/// # Errors
/// Returns [`SearchError`] if the keyring token is missing/inaccessible or the REST call fails.
#[allow(dead_code)]
pub async fn search_remote(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    guild_id: String,
    params: Option<SearchParams>,
) -> Result<SearchResponse, SearchError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_search(move || backend.token()).await?;
    search_remote_with_token(&token, &guild_id, params).await
}

/// Parses and validates a raw inline search query string into a normalized [`SearchBffQuery`].
///
/// # Errors
/// Returns [`SearchBffError`] if any inline token (`from:`, `mentions:`, `has:`, `pinned:`, `in:`,
/// `before:`, `after:`, `author_type:`) is invalid or the query is empty.
#[allow(dead_code)]
#[inline]
pub fn parse_search_query(raw_query: &str) -> Result<SearchBffQuery, SearchBffError> {
    search_bff::parse_search_query(raw_query)
}

/// Executes a full-text BFF search with 202 indexing polling and 429 exponential backoff using an initialized [`SearchBffClient`].
///
/// # Errors
/// Returns [`SearchBffError`] if scope/query validation fails or the REST request fails.
#[allow(dead_code)]
#[inline]
pub async fn search_bff_with_client(
    client: &SearchBffClient,
    scope: &SearchBffScope,
    query: &SearchBffQuery,
) -> Result<SearchBffResponse, SearchBffError> {
    client.execute_with_retry(scope, query).await
}

/// Executes a cached full-text BFF search (`MAX_SEARCH_CACHE_ENTRIES = 64`) using an initialized [`SearchBffClient`].
///
/// # Errors
/// Returns [`SearchBffError`] if scope/query validation fails or the REST request fails.
#[allow(dead_code)]
#[inline]
pub async fn search_bff_cached_with_client(
    client: &SearchBffClient,
    scope: &SearchBffScope,
    query: &SearchBffQuery,
) -> Result<SearchBffResponse, SearchBffError> {
    client.search_cached(scope, query).await
}

/// Initializes the modular Tauri v2 `"search"` plugin.
#[inline]
#[must_use]
pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("search").build()
}

/// Alias for [`init`] (`plugin::search()`).
#[allow(dead_code)]
#[inline]
#[must_use]
pub fn search<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    init()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Mutex;

    struct FakeRemoteSearch {
        recorded: Mutex<Vec<(String, SearchParams)>>,
        outcome: Result<SearchResponse, SearchError>,
    }

    impl FakeRemoteSearch {
        fn ok(response: SearchResponse) -> Self {
            Self {
                recorded: Mutex::new(Vec::new()),
                outcome: Ok(response),
            }
        }

        fn err(error: SearchError) -> Self {
            Self {
                recorded: Mutex::new(Vec::new()),
                outcome: Err(error),
            }
        }
    }

    impl RemoteSearch for FakeRemoteSearch {
        fn search<'a>(
            &'a self,
            guild_id: &'a str,
            params: &'a SearchParams,
        ) -> Pin<Box<dyn Future<Output = Result<SearchResponse, SearchError>> + Send + 'a>>
        {
            self.recorded
                .lock()
                .expect("recorded lock")
                .push((guild_id.to_string(), params.clone()));
            let outcome = self.outcome.clone();
            Box::pin(async move { outcome })
        }
    }

    #[test]
    fn search_remote_with_api_defaults_params_and_forwards_calls() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");

        let expected = SearchResponse {
            messages: vec![serde_json::json!({"id": "101", "content": "hello"})],
            total_results: 1,
            doing_deep_historical_index: false,
            documents_indexed: 0,
            indexing: false,
            retry_after_ms: None,
        };
        let api = FakeRemoteSearch::ok(expected.clone());
        let actual = runtime
            .block_on(search_remote_with_api(&api, "123456789012345678", None))
            .expect("search_remote_with_api ok");
        assert_eq!(actual, expected);

        let recorded = api.recorded.lock().expect("recorded lock");
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].0, "123456789012345678");
        assert_eq!(recorded[0].1, SearchParams::default());
    }

    #[test]
    fn search_remote_with_api_propagates_typed_errors() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");

        let api = FakeRemoteSearch::err(SearchError::Forbidden);
        let err = runtime
            .block_on(search_remote_with_api(&api, "123456789012345678", None))
            .expect_err("must propagate Forbidden");
        assert_eq!(err, SearchError::Forbidden);
    }

    #[test]
    fn search_remote_with_token_rejects_empty_token_before_network() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");

        for empty in ["", "   "] {
            let err = runtime
                .block_on(search_remote_with_token(empty, "123456789012345678", None))
                .expect_err("empty token must fail");
            assert_eq!(err, SearchError::MissingToken);
        }
    }

    #[test]
    fn token_or_search_maps_missing_and_storage_errors() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");

        assert_eq!(
            runtime.block_on(token_or_search(|| Ok(Some("valid-token".to_string())))),
            Ok("valid-token".to_string())
        );
        assert_eq!(
            runtime.block_on(token_or_search(|| Ok(None))),
            Err(SearchError::MissingToken)
        );
        assert_eq!(
            runtime.block_on(token_or_search(|| Ok(Some("   ".to_string())))),
            Err(SearchError::MissingToken)
        );
        assert_eq!(
            runtime.block_on(token_or_search(|| {
                Err(AuthError::Keyring {
                    message: "Secret Service unavailable".to_string(),
                })
            })),
            Err(SearchError::Storage)
        );
    }

    #[test]
    fn parse_search_query_delegates_to_search_bff() {
        let query = parse_search_query("from:123456789012345678 has:image hello")
            .expect("valid inline query");
        assert_eq!(query.from, vec!["123456789012345678".to_string()]);
        assert_eq!(query.has, vec!["image".to_string()]);
        assert_eq!(query.content.as_deref(), Some("hello"));
        assert_eq!(parse_search_query("   "), Err(SearchBffError::EmptyQuery));
    }
}
