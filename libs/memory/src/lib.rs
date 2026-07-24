use async_trait::async_trait;
use errors::ContextraError;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use storage::cache::Cache;
use storage::conversation::ConversationRepository;
use storage::repository::Repository;
use storage::session_store::SessionStore;
use types::{ConversationId, Message, Metadata, Role};
use uuid::Uuid;

const DEFAULT_CONTEXT_TOKEN_LIMIT: usize = 4_000;
const DEFAULT_SESSION_TTL: Duration = Duration::from_secs(60 * 60);

#[async_trait]
pub trait ConversationHistoryStore: Send + Sync {
    async fn create_conversation(&self, id: &ConversationId) -> Result<(), ContextraError>;

    async fn append_message(&self, message: &Message) -> Result<(), ContextraError>;

    async fn messages(
        &self,
        conversation_id: &ConversationId,
    ) -> Result<Vec<Message>, ContextraError>;
}

#[async_trait]
impl ConversationHistoryStore for ConversationRepository {
    async fn create_conversation(&self, id: &ConversationId) -> Result<(), ContextraError> {
        ConversationRepository::create_conversation(self, id).await
    }

    async fn append_message(&self, message: &Message) -> Result<(), ContextraError> {
        self.create(message).await
    }

    async fn messages(
        &self,
        conversation_id: &ConversationId,
    ) -> Result<Vec<Message>, ContextraError> {
        self.get_messages_by_conversation(conversation_id).await
    }
}

#[async_trait]
pub trait HotSessionStore<S>: Send + Sync {
    async fn get(&self, conversation_id: &ConversationId) -> Result<Option<S>, ContextraError>;

    async fn set(&self, conversation_id: &ConversationId, state: &S) -> Result<(), ContextraError>;

    async fn delete(&self, conversation_id: &ConversationId) -> Result<(), ContextraError>;

    async fn exists(&self, conversation_id: &ConversationId) -> Result<bool, ContextraError>;
}

#[async_trait]
impl<C, S> HotSessionStore<S> for SessionStore<C, S>
where
    C: Cache,
    S: Serialize + DeserializeOwned + Send + Sync,
{
    async fn get(&self, conversation_id: &ConversationId) -> Result<Option<S>, ContextraError> {
        SessionStore::get(self, conversation_id).await
    }

    async fn set(&self, conversation_id: &ConversationId, state: &S) -> Result<(), ContextraError> {
        SessionStore::set(self, conversation_id, state).await
    }

    async fn delete(&self, conversation_id: &ConversationId) -> Result<(), ContextraError> {
        SessionStore::delete(self, conversation_id).await
    }

    async fn exists(&self, conversation_id: &ConversationId) -> Result<bool, ContextraError> {
        SessionStore::exists(self, conversation_id).await
    }
}

pub trait TokenCounter: Send + Sync {
    fn count_tokens(&self, text: &str) -> usize;

    fn count_message_tokens(&self, message: &Message) -> usize {
        role_token_cost(&message.role) + self.count_tokens(&message.content)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ApproximateTokenCounter;

impl TokenCounter for ApproximateTokenCounter {
    fn count_tokens(&self, text: &str) -> usize {
        let wordish = text
            .split_whitespace()
            .map(|token| token.chars().count().div_ceil(4).max(1))
            .sum::<usize>();

        wordish.max(text.chars().count().div_ceil(4))
    }
}

#[derive(Debug, Clone)]
pub struct SlidingWindowTruncator<T = ApproximateTokenCounter> {
    max_tokens: usize,
    counter: T,
    preserve_system_messages: bool,
}

impl SlidingWindowTruncator<ApproximateTokenCounter> {
    pub fn new(max_tokens: usize) -> Self {
        Self {
            max_tokens: max_tokens.max(1),
            counter: ApproximateTokenCounter,
            preserve_system_messages: true,
        }
    }
}

impl<T> SlidingWindowTruncator<T>
where
    T: TokenCounter,
{
    pub fn with_counter(max_tokens: usize, counter: T) -> Self {
        Self {
            max_tokens: max_tokens.max(1),
            counter,
            preserve_system_messages: true,
        }
    }

    pub fn preserve_system_messages(mut self, preserve: bool) -> Self {
        self.preserve_system_messages = preserve;
        self
    }

    pub fn max_tokens(&self) -> usize {
        self.max_tokens
    }

    pub fn token_count(&self, messages: &[Message]) -> usize {
        messages
            .iter()
            .map(|message| self.counter.count_message_tokens(message))
            .sum()
    }

    pub fn truncate(&self, messages: &[Message]) -> Vec<Message> {
        let mut selected_reversed = Vec::new();
        let mut used_tokens = 0_usize;

        for message in messages.iter().rev() {
            if self.preserve_system_messages && matches!(message.role, Role::System) {
                continue;
            }

            let tokens = self.counter.count_message_tokens(message);
            if used_tokens + tokens <= self.max_tokens || selected_reversed.is_empty() {
                used_tokens += tokens;
                selected_reversed.push(message.clone());
            } else {
                break;
            }
        }

        selected_reversed.reverse();

        if self.preserve_system_messages {
            let mut preserved = messages
                .iter()
                .filter(|message| matches!(message.role, Role::System))
                .cloned()
                .collect::<Vec<_>>();
            preserved.extend(selected_reversed);
            preserved
        } else {
            selected_reversed
        }
    }
}

impl Default for SlidingWindowTruncator<ApproximateTokenCounter> {
    fn default() -> Self {
        Self::new(DEFAULT_CONTEXT_TOKEN_LIMIT)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationSession {
    pub conversation_id: ConversationId,
    pub created_at_epoch_seconds: u64,
    pub last_active_epoch_seconds: u64,
    pub expires_at_epoch_seconds: u64,
    pub message_count: usize,
}

impl ConversationSession {
    fn new(conversation_id: ConversationId, ttl: Duration) -> Self {
        let now = now_epoch_seconds();
        Self {
            conversation_id,
            created_at_epoch_seconds: now,
            last_active_epoch_seconds: now,
            expires_at_epoch_seconds: now + ttl.as_secs().max(1),
            message_count: 0,
        }
    }

    fn touch(&mut self, ttl: Duration, message_count: usize) {
        let now = now_epoch_seconds();
        self.last_active_epoch_seconds = now;
        self.expires_at_epoch_seconds = now + ttl.as_secs().max(1);
        self.message_count = message_count;
    }
}

#[derive(Debug, Clone)]
pub struct SessionManager<H, S = ConversationSession> {
    hot_store: H,
    ttl: Duration,
    _state: std::marker::PhantomData<S>,
}

impl<H> SessionManager<H, ConversationSession> {
    pub fn new(hot_store: H) -> Self {
        Self::with_ttl(hot_store, DEFAULT_SESSION_TTL)
    }

    pub fn with_ttl(hot_store: H, ttl: Duration) -> Self {
        Self {
            hot_store,
            ttl,
            _state: std::marker::PhantomData,
        }
    }
}

impl<H> SessionManager<H, ConversationSession>
where
    H: HotSessionStore<ConversationSession>,
{
    pub async fn create(&self) -> Result<ConversationSession, ContextraError> {
        let session = ConversationSession::new(ConversationId::new(), self.ttl);
        self.hot_store
            .set(&session.conversation_id, &session)
            .await?;
        Ok(session)
    }

    pub async fn resume(
        &self,
        conversation_id: &ConversationId,
    ) -> Result<Option<ConversationSession>, ContextraError> {
        self.hot_store.get(conversation_id).await
    }

    pub async fn touch(
        &self,
        conversation_id: &ConversationId,
        message_count: usize,
    ) -> Result<ConversationSession, ContextraError> {
        let mut session = self
            .hot_store
            .get(conversation_id)
            .await?
            .unwrap_or_else(|| ConversationSession::new(*conversation_id, self.ttl));
        session.touch(self.ttl, message_count);
        self.hot_store.set(conversation_id, &session).await?;
        Ok(session)
    }

    pub async fn expire(&self, conversation_id: &ConversationId) -> Result<(), ContextraError> {
        self.hot_store.delete(conversation_id).await
    }

    pub async fn exists(&self, conversation_id: &ConversationId) -> Result<bool, ContextraError> {
        self.hot_store.exists(conversation_id).await
    }
}

#[derive(Debug, Clone)]
pub struct ConversationMemory<P, H, T = ApproximateTokenCounter> {
    durable_store: P,
    session_manager: SessionManager<H>,
    truncator: SlidingWindowTruncator<T>,
}

impl<P, H> ConversationMemory<P, H, ApproximateTokenCounter> {
    pub fn new(durable_store: P, hot_store: H) -> Self {
        Self {
            durable_store,
            session_manager: SessionManager::new(hot_store),
            truncator: SlidingWindowTruncator::default(),
        }
    }
}

impl<P, H, T> ConversationMemory<P, H, T>
where
    T: TokenCounter,
{
    pub fn with_truncator(
        durable_store: P,
        hot_store: H,
        truncator: SlidingWindowTruncator<T>,
    ) -> Self {
        Self {
            durable_store,
            session_manager: SessionManager::new(hot_store),
            truncator,
        }
    }

    pub fn with_session_ttl(mut self, ttl: Duration) -> Self {
        self.session_manager.ttl = ttl;
        self
    }

    pub fn session_manager(&self) -> &SessionManager<H> {
        &self.session_manager
    }

    pub fn truncator(&self) -> &SlidingWindowTruncator<T> {
        &self.truncator
    }
}

impl<P, H, T> ConversationMemory<P, H, T>
where
    P: ConversationHistoryStore,
    H: HotSessionStore<ConversationSession>,
    T: TokenCounter,
{
    pub async fn create_session(&self) -> Result<ConversationSession, ContextraError> {
        let session = self.session_manager.create().await?;
        self.durable_store
            .create_conversation(&session.conversation_id)
            .await?;
        Ok(session)
    }

    pub async fn append_message(
        &self,
        conversation_id: ConversationId,
        role: Role,
        content: impl Into<String>,
        metadata: Metadata,
    ) -> Result<Message, ContextraError> {
        let message = Message {
            id: Uuid::now_v7(),
            conversation_id,
            role,
            content: content.into(),
            metadata,
        };

        self.durable_store.append_message(&message).await?;
        let message_count = self.durable_store.messages(&conversation_id).await?.len();
        self.session_manager
            .touch(&conversation_id, message_count)
            .await?;

        Ok(message)
    }

    pub async fn history(
        &self,
        conversation_id: &ConversationId,
    ) -> Result<Vec<Message>, ContextraError> {
        self.durable_store.messages(conversation_id).await
    }

    pub async fn context_window(
        &self,
        conversation_id: &ConversationId,
    ) -> Result<Vec<Message>, ContextraError> {
        let messages = self.history(conversation_id).await?;
        Ok(self.truncator.truncate(&messages))
    }

    pub async fn resume_session(
        &self,
        conversation_id: &ConversationId,
    ) -> Result<Option<ConversationSession>, ContextraError> {
        self.session_manager.resume(conversation_id).await
    }

    pub async fn expire_session(
        &self,
        conversation_id: &ConversationId,
    ) -> Result<(), ContextraError> {
        self.session_manager.expire(conversation_id).await
    }
}

fn role_token_cost(role: &Role) -> usize {
    match role {
        Role::System => 4,
        Role::User | Role::Assistant => 3,
        Role::Tool => 5,
    }
}

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[derive(Debug, Clone, Copy)]
    struct WordTokenCounter;

    impl TokenCounter for WordTokenCounter {
        fn count_tokens(&self, text: &str) -> usize {
            text.split_whitespace().count()
        }

        fn count_message_tokens(&self, message: &Message) -> usize {
            self.count_tokens(&message.content)
        }
    }

    #[test]
    fn sliding_window_truncates_old_messages_by_token_count() {
        let conversation_id = ConversationId::new();
        let messages = vec![
            message(conversation_id, Role::System, "system prompt remains"),
            message(conversation_id, Role::User, "one two three"),
            message(conversation_id, Role::Assistant, "four five six"),
            message(conversation_id, Role::User, "seven eight"),
            message(conversation_id, Role::Assistant, "nine ten"),
        ];
        let truncator = SlidingWindowTruncator::with_counter(7, WordTokenCounter);

        let window = truncator.truncate(&messages);

        assert_eq!(window.len(), 4);
        assert_eq!(window[0].role, Role::System);
        assert_eq!(window[1].content, "four five six");
        assert_eq!(window[2].content, "seven eight");
        assert_eq!(window[3].content, "nine ten");
        assert_eq!(truncator.token_count(&window[1..]), 7);
    }

    #[tokio::test]
    async fn session_lifecycle_create_append_resume_expire()
    -> Result<(), Box<dyn std::error::Error>> {
        let durable = InMemoryConversationStore::default();
        let hot = InMemoryHotSessionStore::default();
        let memory = ConversationMemory::with_truncator(
            durable.clone(),
            hot.clone(),
            SlidingWindowTruncator::with_counter(10, WordTokenCounter),
        );

        let session = memory.create_session().await?;
        assert!(
            memory
                .session_manager()
                .exists(&session.conversation_id)
                .await?
        );

        memory
            .append_message(
                session.conversation_id,
                Role::User,
                "hello there",
                Metadata::new(),
            )
            .await?;
        memory
            .append_message(
                session.conversation_id,
                Role::Assistant,
                "general kenobi",
                Metadata::new(),
            )
            .await?;

        let resumed = memory
            .resume_session(&session.conversation_id)
            .await?
            .ok_or_else(|| std::io::Error::other("session should exist"))?;
        assert_eq!(resumed.conversation_id, session.conversation_id);
        assert_eq!(resumed.message_count, 2);

        let history = memory.history(&session.conversation_id).await?;
        assert_eq!(history.len(), 2);

        memory.expire_session(&session.conversation_id).await?;
        assert!(
            memory
                .resume_session(&session.conversation_id)
                .await?
                .is_none()
        );
        assert_eq!(memory.history(&session.conversation_id).await?.len(), 2);

        Ok(())
    }

    #[derive(Debug, Clone, Default)]
    struct InMemoryConversationStore {
        state: Arc<Mutex<HashMap<ConversationId, Vec<Message>>>>,
    }

    #[async_trait]
    impl ConversationHistoryStore for InMemoryConversationStore {
        async fn create_conversation(&self, id: &ConversationId) -> Result<(), ContextraError> {
            self.state.lock().await.entry(*id).or_default();
            Ok(())
        }

        async fn append_message(&self, message: &Message) -> Result<(), ContextraError> {
            let mut state = self.state.lock().await;
            let messages = state.get_mut(&message.conversation_id).ok_or_else(|| {
                ContextraError::NotFound(format!(
                    "conversation {} not found",
                    message.conversation_id
                ))
            })?;
            messages.push(message.clone());
            Ok(())
        }

        async fn messages(
            &self,
            conversation_id: &ConversationId,
        ) -> Result<Vec<Message>, ContextraError> {
            Ok(self
                .state
                .lock()
                .await
                .get(conversation_id)
                .cloned()
                .unwrap_or_default())
        }
    }

    #[derive(Debug, Clone, Default)]
    struct InMemoryHotSessionStore {
        state: Arc<Mutex<HashMap<ConversationId, ConversationSession>>>,
    }

    #[async_trait]
    impl HotSessionStore<ConversationSession> for InMemoryHotSessionStore {
        async fn get(
            &self,
            conversation_id: &ConversationId,
        ) -> Result<Option<ConversationSession>, ContextraError> {
            Ok(self.state.lock().await.get(conversation_id).cloned())
        }

        async fn set(
            &self,
            conversation_id: &ConversationId,
            state: &ConversationSession,
        ) -> Result<(), ContextraError> {
            self.state
                .lock()
                .await
                .insert(*conversation_id, state.clone());
            Ok(())
        }

        async fn delete(&self, conversation_id: &ConversationId) -> Result<(), ContextraError> {
            self.state.lock().await.remove(conversation_id);
            Ok(())
        }

        async fn exists(&self, conversation_id: &ConversationId) -> Result<bool, ContextraError> {
            Ok(self.state.lock().await.contains_key(conversation_id))
        }
    }

    fn message(conversation_id: ConversationId, role: Role, content: &str) -> Message {
        Message {
            id: Uuid::now_v7(),
            conversation_id,
            role,
            content: content.to_string(),
            metadata: Metadata::new(),
        }
    }
}
