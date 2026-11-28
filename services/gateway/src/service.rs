use crate::{
    ChatExecutionRequest, ChatExecutionResponse, CollectionResource, ConversationResource,
    CreateCollectionRequest, CreateConversationRequest, CreateDocumentRequest, DocumentFilter,
    DocumentResource, GatewayService, MessageResource, Page, Pagination,
};
use async_trait::async_trait;
use auth::{AuthContext, authenticate_api_key};
use common::{
    pagination::Cursor,
    time::{Clock, SystemClock, format_rfc3339},
};
use embeddings::{Embedding, EmbeddingError, EmbeddingProvider, OpenAIEmbeddingProvider};
use errors::ContextraError;
use ingestion::{FixedSizeChunker, IngestionPipeline, ParsedDocument, PlainTextParser};
use providers::{ChatMessage, ChatRequest, ChatResponse, ChatStream, LLMProvider, ProviderError};
use settings::Settings;
use std::sync::Arc;
use storage::{
    api_key::PgApiKeyStore,
    collection::CollectionRepository,
    conversation::ConversationRepository,
    db::PgPool,
    document::DocumentRepository,
    repository::Repository,
    vector_store::{QdrantVectorStore, VectorStore},
};
use types::{Collection, CollectionId, ConversationId, DocumentId, Message, Role};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct NoopLLMProvider;

#[async_trait]
impl LLMProvider for NoopLLMProvider {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        Err(ProviderError::MissingConfiguration(
            "No LLM provider configured. Chat is unavailable, but document, collection, and conversation APIs are functioning normally.".to_string(),
        ))
    }

    async fn chat_stream(&self, _request: ChatRequest) -> Result<ChatStream, ProviderError> {
        Err(ProviderError::MissingConfiguration(
            "No LLM provider configured. Chat is unavailable, but document, collection, and conversation APIs are functioning normally.".to_string(),
        ))
    }

    fn supports_function_calling(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone)]
pub struct GatewayEmbeddingProvider {
    openai: Option<OpenAIEmbeddingProvider>,
}

impl GatewayEmbeddingProvider {
    pub fn new(openai_key: Option<String>) -> Self {
        let openai = openai_key
            .filter(|k| !k.trim().is_empty())
            .map(|k| OpenAIEmbeddingProvider::new(k, "text-embedding-3-small", 1536));
        Self { openai }
    }
}

#[async_trait]
impl EmbeddingProvider for GatewayEmbeddingProvider {
    async fn embed_batch(&self, inputs: &[String]) -> Result<Vec<Embedding>, EmbeddingError> {
        if let Some(ref provider) = self.openai {
            provider.embed_batch(inputs).await
        } else {
            // Deterministic mock embedding for demo / keyless operation
            Ok(inputs
                .iter()
                .map(|text| {
                    let mut vec = vec![0.1f32; 1536];
                    let hash = text.bytes().fold(0u64, |acc, b| acc.wrapping_add(b as u64));
                    vec[0] = (hash % 100) as f32 / 100.0;
                    vec
                })
                .collect())
        }
    }

    fn dimensions(&self) -> usize {
        1536
    }

    fn model_name(&self) -> &str {
        if self.openai.is_some() {
            "text-embedding-3-small"
        } else {
            "gateway-fallback-embeddings"
        }
    }
}

pub struct ProductionGatewayService {
    api_key_store: PgApiKeyStore,
    doc_repo: DocumentRepository,
    col_repo: CollectionRepository,
    conv_repo: ConversationRepository,
    vector_store: QdrantVectorStore,
    llm_provider: Arc<dyn LLMProvider>,
    has_real_llm: bool,
    embedding_provider: Arc<dyn EmbeddingProvider>,
    model_name: String,
}

impl ProductionGatewayService {
    pub fn new(
        pool: PgPool,
        vector_store: QdrantVectorStore,
        llm_provider: Arc<dyn LLMProvider>,
        has_real_llm: bool,
        settings: &Settings,
    ) -> Self {
        let api_key_store = PgApiKeyStore::new(pool.clone());
        let doc_repo = DocumentRepository::new(pool.clone());
        let col_repo = CollectionRepository::new(pool.clone());
        let conv_repo = ConversationRepository::new(pool.clone());
        let embedding_provider: Arc<dyn EmbeddingProvider> = Arc::new(
            GatewayEmbeddingProvider::new(settings.providers.openai_api_key.clone()),
        );

        Self {
            api_key_store,
            doc_repo,
            col_repo,
            conv_repo,
            vector_store,
            llm_provider,
            has_real_llm,
            embedding_provider,
            model_name: settings
                .providers
                .model
                .as_deref()
                .map(str::trim)
                .filter(|model| !model.is_empty())
                .unwrap_or_else(|| {
                    match settings
                        .providers
                        .provider
                        .as_deref()
                        .unwrap_or("")
                        .trim()
                        .to_ascii_lowercase()
                        .as_str()
                    {
                        "anthropic" | "claude" => "claude-sonnet-4-6",
                        "gemini" | "google" => "gemini-3.5-flash-lite",
                        _ => "gpt-4o-mini",
                    }
                })
                .to_string(),
        }
    }
}

#[async_trait]
impl GatewayService for ProductionGatewayService {
    async fn authenticate(&self, token: &str) -> Result<AuthContext, ContextraError> {
        authenticate_api_key(&self.api_key_store, token).await
    }

    async fn list_documents(
        &self,
        pagination: Pagination,
        filter: DocumentFilter,
    ) -> Result<Page<DocumentResource>, ContextraError> {
        let offset = pagination.cursor.unwrap_or(0);
        let limit = pagination.limit;
        let limit_i64 = (limit + 1) as i64;
        let offset_i64 = offset as i64;

        let collection_id = filter
            .collection_id
            .as_deref()
            .map(parse_collection_id)
            .transpose()?;

        let docs = self
            .doc_repo
            .list(collection_id, limit_i64, offset_i64)
            .await?;
        let has_more = docs.len() > limit;
        let items: Vec<DocumentResource> = docs
            .into_iter()
            .take(limit)
            .map(DocumentResource::from)
            .collect();

        let next_cursor = if has_more {
            Some(Cursor((offset + limit).to_string()))
        } else {
            None
        };

        Ok(Page::new(items, next_cursor, has_more, None))
    }

    async fn get_document(&self, id: DocumentId) -> Result<DocumentResource, ContextraError> {
        self.doc_repo
            .get(&id)
            .await?
            .map(DocumentResource::from)
            .ok_or_else(|| ContextraError::NotFound(format!("Document {id} not found")))
    }

    async fn create_document(
        &self,
        request: CreateDocumentRequest,
    ) -> Result<DocumentResource, ContextraError> {
        let collection_id = match request.collection_id {
            Some(id) => {
                let id = parse_collection_id(&id)?;
                if self.col_repo.get(id).await?.is_none() {
                    return Err(ContextraError::NotFound(format!(
                        "Collection {id} not found"
                    )));
                }
                id
            }
            None => CollectionId::from(Uuid::nil()),
        };

        let collection_name = collection_id.to_string();
        let mut metadata = types::Metadata::new();
        if let Some(name) = request.name.filter(|name| !name.trim().is_empty()) {
            metadata.insert("name".into(), serde_json::json!(name.trim()));
        }
        metadata.insert(
            "uploaded_at".into(),
            serde_json::json!(format_rfc3339(SystemClock.now())),
        );
        metadata.insert(
            "file_size".into(),
            serde_json::json!(format!("{} B", request.source_path.len())),
        );

        let pipeline = IngestionPipeline::new(
            PlainTextParser,
            FixedSizeChunker::new(512, 64)?,
            self.embedding_provider.clone(),
            self.vector_store.clone(),
            collection_name.clone(),
            collection_id,
        );
        let result = pipeline
            .ingest_parsed(ParsedDocument::new(request.source_path, metadata))
            .await?;

        if let Err(error) = self
            .doc_repo
            .create_with_chunks(&result.document, &result.chunks)
            .await
        {
            let ids: Vec<_> = result.chunks.iter().map(|chunk| chunk.id).collect();
            if let Err(cleanup_error) = self.vector_store.delete_by_id(&collection_name, &ids).await
            {
                tracing::error!(%cleanup_error, document_id = %result.document.id, "failed to clean up vectors after document write failure");
            }
            return Err(error);
        }

        Ok(DocumentResource::from(result.document))
    }

    async fn list_collections(
        &self,
        pagination: Pagination,
    ) -> Result<Page<CollectionResource>, ContextraError> {
        let offset = pagination.cursor.unwrap_or(0);
        let limit = pagination.limit;
        let limit_i64 = (limit + 1) as i64;
        let offset_i64 = offset as i64;

        let collections = self.col_repo.list(limit_i64, offset_i64).await?;
        let has_more = collections.len() > limit;
        let items: Vec<CollectionResource> = collections
            .into_iter()
            .take(limit)
            .map(|c| CollectionResource {
                id: c.id.to_string(),
                name: c.name,
                metadata: c.metadata,
            })
            .collect();

        let next_cursor = if has_more {
            Some(Cursor((offset + limit).to_string()))
        } else {
            None
        };

        Ok(Page::new(items, next_cursor, has_more, None))
    }

    async fn get_collection(&self, id: CollectionId) -> Result<CollectionResource, ContextraError> {
        self.col_repo
            .get(id)
            .await?
            .map(|c| CollectionResource {
                id: c.id.to_string(),
                name: c.name,
                metadata: c.metadata,
            })
            .ok_or_else(|| ContextraError::NotFound(format!("Collection {id} not found")))
    }

    async fn create_collection(
        &self,
        request: CreateCollectionRequest,
    ) -> Result<CollectionResource, ContextraError> {
        let mut metadata = request.metadata;
        metadata.insert(
            "created_at".into(),
            serde_json::json!(format_rfc3339(SystemClock.now())),
        );
        let collection = Collection {
            id: CollectionId::new(),
            name: request.name.trim().to_string(),
            metadata,
        };

        self.vector_store
            .create_collection(
                &collection.id.to_string(),
                self.embedding_provider.dimensions(),
            )
            .await?;
        if let Err(error) = self.col_repo.create(&collection).await {
            if let Err(cleanup_error) = self
                .vector_store
                .remove_collection(&collection.id.to_string())
                .await
            {
                tracing::error!(%cleanup_error, collection_id = %collection.id, "failed to clean up vector collection");
            }
            return Err(error);
        }

        Ok(CollectionResource {
            id: collection.id.to_string(),
            name: collection.name,
            metadata: collection.metadata,
        })
    }

    async fn update_collection(
        &self,
        id: CollectionId,
        request: CreateCollectionRequest,
    ) -> Result<CollectionResource, ContextraError> {
        let updated = self
            .col_repo
            .update(id, request.name.trim(), &request.metadata)
            .await?
            .ok_or_else(|| ContextraError::NotFound(format!("Collection {id} not found")))?;
        Ok(CollectionResource {
            id: id.to_string(),
            name: updated.name,
            metadata: updated.metadata,
        })
    }

    async fn list_conversations(
        &self,
        pagination: Pagination,
    ) -> Result<Page<ConversationResource>, ContextraError> {
        let offset = pagination.cursor.unwrap_or(0);
        let limit = pagination.limit;
        let limit_i64 = (limit + 1) as i64;
        let offset_i64 = offset as i64;

        let convs = self
            .conv_repo
            .list_conversations(limit_i64, offset_i64)
            .await?;
        let has_more = convs.len() > limit;
        let items: Vec<ConversationResource> = convs
            .into_iter()
            .take(limit)
            .map(|c| ConversationResource {
                id: c.id.to_string(),
                title: c.title,
                metadata: c.metadata,
            })
            .collect();

        let next_cursor = if has_more {
            Some(Cursor((offset + limit).to_string()))
        } else {
            None
        };

        Ok(Page::new(items, next_cursor, has_more, None))
    }

    async fn create_conversation(
        &self,
        request: CreateConversationRequest,
    ) -> Result<ConversationResource, ContextraError> {
        let id = ConversationId::new();
        self.conv_repo
            .create_conversation_with_details(&id, request.title.as_deref(), &request.metadata)
            .await?;

        Ok(ConversationResource {
            id: id.to_string(),
            title: request.title,
            metadata: request.metadata,
        })
    }

    async fn list_messages(
        &self,
        conversation_id: ConversationId,
        pagination: Pagination,
    ) -> Result<Page<MessageResource>, ContextraError> {
        let offset = pagination.cursor.unwrap_or(0);
        let limit = pagination.limit;

        let messages = self
            .conv_repo
            .get_messages_by_conversation(&conversation_id)
            .await?;
        let total_count = messages.len() as u64;

        let items: Vec<MessageResource> = messages
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|m| MessageResource {
                id: m.id,
                conversation_id: m.conversation_id.to_string(),
                role: serde_json::to_string(&m.role)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string(),
                content: m.content,
                metadata: m.metadata,
            })
            .collect();

        let has_more = offset + limit < total_count as usize;
        let next_cursor = if has_more {
            Some(Cursor((offset + limit).to_string()))
        } else {
            None
        };

        Ok(Page::new(items, next_cursor, has_more, Some(total_count)))
    }

    async fn execute_chat(
        &self,
        conversation_id: ConversationId,
        request: ChatExecutionRequest,
    ) -> Result<ChatExecutionResponse, ContextraError> {
        if !self.has_real_llm {
            return Err(ContextraError::ServiceUnavailable(
                "No LLM provider configured. Chat is unavailable, but document, collection, and conversation APIs are functioning normally.".to_string(),
            ));
        }

        // Ensure conversation exists
        self.conv_repo.create_conversation(&conversation_id).await?;

        // 1. Fetch history
        let history = self
            .conv_repo
            .get_messages_by_conversation(&conversation_id)
            .await?;

        // 2. Build chat request
        let mut chat_messages = vec![ChatMessage::system(
            "You are Contextra, an intelligent context engineering platform assistant.",
        )];
        for msg in history {
            match msg.role {
                Role::User => chat_messages.push(ChatMessage::user(msg.content)),
                Role::Assistant => chat_messages.push(ChatMessage::assistant(msg.content)),
                Role::System => chat_messages.push(ChatMessage::system(msg.content)),
                Role::Tool => {}
            }
        }
        chat_messages.push(ChatMessage::user(request.message.clone()));

        let req = ChatRequest::new(&self.model_name, chat_messages);
        let resp = self.llm_provider.chat(req).await?;
        let reply_content = resp
            .message
            .content
            .as_ref()
            .filter(|reply| !reply.trim().is_empty())
            .ok_or_else(|| {
                ContextraError::ProviderError("The model returned an empty reply".into())
            })?;

        let timestamp = serde_json::json!(format_rfc3339(SystemClock.now()));
        let user_msg = Message {
            id: Uuid::now_v7(),
            conversation_id,
            role: Role::User,
            content: request.message,
            metadata: types::Metadata::from([("timestamp".into(), timestamp.clone())]),
        };
        let assistant_msg = Message {
            id: Uuid::now_v7(),
            conversation_id,
            role: Role::Assistant,
            content: reply_content.clone(),
            metadata: types::Metadata::from([("timestamp".into(), timestamp)]),
        };
        self.conv_repo.save_turn(&user_msg, &assistant_msg).await?;

        Ok(ChatExecutionResponse::from(resp))
    }
}

fn parse_collection_id(id: &str) -> Result<CollectionId, ContextraError> {
    id.parse()
        .map_err(|_| ContextraError::Validation("invalid collection ID".into()))
}
