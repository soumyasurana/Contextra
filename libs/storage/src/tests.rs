use crate::cache::{Cache, RedisCache};
use crate::conversation::ConversationRepository;
use crate::db::PgPool;
use crate::document::DocumentRepository;
use crate::repository::Repository;
use crate::session_store::SessionStore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::time::Duration;
use types::{Chunk, CollectionId, ConversationId, Document, DocumentId, Message, Role};
use uuid::Uuid;

#[tokio::test]
async fn test_integration_crud() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = match env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            println!("Skipping integration tests: DATABASE_URL not set");
            return Ok(());
        }
    };

    let pool = PgPool::connect(&database_url).await?;

    // In a real test suite, you'd likely create a separate test DB or rollback transaction.
    // For this integration test, we will run migrations just to be sure.
    pool.run_migrations().await?;

    // Test Document & Chunk Repository
    let doc_repo = DocumentRepository::new(pool.clone());

    let doc = Document {
        id: DocumentId::new(),
        collection_id: CollectionId::new(),
        content: "Test Document".to_string(),
        metadata: HashMap::new(),
    };

    doc_repo.create(&doc).await?;

    let fetched_doc = doc_repo.get(&doc.id).await?.ok_or("Document not found")?;

    assert_eq!(doc, fetched_doc);

    let mut updated_doc = doc.clone();
    updated_doc.content = "Updated Document".to_string();

    doc_repo.update(&updated_doc).await?;

    let chunk = Chunk {
        id: Uuid::now_v7(),
        document_id: doc.id,
        content: "Test Chunk".to_string(),
        metadata: HashMap::new(),
    };

    doc_repo.create_chunk(&chunk).await?;

    let chunks = doc_repo.get_chunks_by_document(&doc.id).await?;

    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], chunk);

    doc_repo.delete(&doc.id).await?;

    let deleted_doc = doc_repo.get(&doc.id).await?;
    assert!(deleted_doc.is_none());

    // Test Conversation & Message Repository
    let conv_repo = ConversationRepository::new(pool.clone());

    let conv_id = ConversationId::new();

    conv_repo.create_conversation(&conv_id).await?;

    let msg = Message {
        id: Uuid::now_v7(),
        conversation_id: conv_id,
        role: Role::User,
        content: "Test Message".to_string(),
        metadata: HashMap::new(),
    };

    conv_repo.create(&msg).await?;

    let fetched_msg = conv_repo.get(&msg.id).await?.ok_or("Message not found")?;

    assert_eq!(msg, fetched_msg);

    let messages = conv_repo.get_messages_by_conversation(&conv_id).await?;

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0], msg);

    let mut updated_msg = msg.clone();
    updated_msg.content = "Updated Message".to_string();

    conv_repo.update(&updated_msg).await?;

    conv_repo.delete(&msg.id).await?;

    let deleted_msg = conv_repo.get(&msg.id).await?;
    assert!(deleted_msg.is_none());

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SessionState {
    turn: u32,
    prompt: String,
    tags: Vec<String>,
}

async fn redis_cache() -> Result<RedisCache, Box<dyn std::error::Error>> {
    let redis_url = match env::var("REDIS_URL") {
        Ok(url) => url,
        Err(_) => {
            println!("Skipping Redis tests: REDIS_URL not set");
            return Err("REDIS_URL not set".into());
        }
    };

    Ok(RedisCache::connect(&redis_url).await?)
}

#[tokio::test]
async fn test_redis_cache_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let cache = match redis_cache().await {
        Ok(cache) => cache,
        Err(err) if err.to_string().contains("not set") => return Ok(()),
        Err(err) => return Err(err),
    };

    let key = format!("contextra:test:cache:{}", Uuid::now_v7());
    let value = SessionState {
        turn: 3,
        prompt: "hello".to_string(),
        tags: vec!["cached".to_string(), "session".to_string()],
    };

    cache
        .set_with_ttl(&key, &value, Duration::from_secs(60))
        .await?;

    assert!(cache.exists(&key).await?);

    let fetched: Option<SessionState> = cache.get(&key).await?;
    assert_eq!(fetched, Some(value));

    cache.delete(&key).await?;
    assert!(!cache.exists(&key).await?);

    Ok(())
}

#[tokio::test]
async fn test_session_store_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let cache = match redis_cache().await {
        Ok(cache) => cache,
        Err(err) if err.to_string().contains("not set") => return Ok(()),
        Err(err) => return Err(err),
    };

    let store = SessionStore::<_, SessionState>::new(cache, Duration::from_secs(120));
    let conversation_id = ConversationId::new();
    let state = SessionState {
        turn: 7,
        prompt: "keep this conversation warm".to_string(),
        tags: vec!["conversation".to_string(), "ttl".to_string()],
    };

    store.set(&conversation_id, &state).await?;
    assert!(store.exists(&conversation_id).await?);

    let fetched = store.get(&conversation_id).await?;
    assert_eq!(fetched, Some(state));

    store.delete(&conversation_id).await?;
    assert!(!store.exists(&conversation_id).await?);

    Ok(())
}
