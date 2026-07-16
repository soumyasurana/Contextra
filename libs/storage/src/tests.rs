use crate::conversation::ConversationRepository;
use crate::db::PgPool;
use crate::document::DocumentRepository;
use crate::repository::Repository;
use std::collections::HashMap;
use std::env;
use types::{Chunk, CollectionId, ConversationId, Document, DocumentId, Message, Role};
use uuid::Uuid;

#[tokio::test]
async fn test_integration_crud() {
    let database_url = match env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            println!("Skipping integration tests: DATABASE_URL not set");
            return;
        }
    };

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to DB");

    // In a real test suite, you'd likely create a separate test DB or rollback transaction.
    // For this integration test, we will run migrations just to be sure.
    pool.run_migrations()
        .await
        .expect("Failed to run migrations");

    // Test Document & Chunk Repository
    let doc_repo = DocumentRepository::new(pool.clone());
    let doc = Document {
        id: DocumentId::new(),
        collection_id: CollectionId::new(),
        content: "Test Document".to_string(),
        metadata: HashMap::new(),
    };

    doc_repo
        .create(&doc)
        .await
        .expect("Failed to create document");

    let fetched_doc = doc_repo
        .get(&doc.id)
        .await
        .expect("Failed to fetch document")
        .expect("Document not found");
    assert_eq!(doc, fetched_doc);

    let mut updated_doc = doc.clone();
    updated_doc.content = "Updated Document".to_string();
    doc_repo
        .update(&updated_doc)
        .await
        .expect("Failed to update document");

    let chunk = Chunk {
        id: Uuid::now_v7(),
        document_id: doc.id,
        content: "Test Chunk".to_string(),
        metadata: HashMap::new(),
    };
    doc_repo
        .create_chunk(&chunk)
        .await
        .expect("Failed to create chunk");

    let chunks = doc_repo
        .get_chunks_by_document(&doc.id)
        .await
        .expect("Failed to fetch chunks");
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], chunk);

    doc_repo
        .delete(&doc.id)
        .await
        .expect("Failed to delete document");
    let deleted_doc = doc_repo
        .get(&doc.id)
        .await
        .expect("Failed to fetch deleted document");
    assert!(deleted_doc.is_none());

    // Test Conversation & Message Repository
    let conv_repo = ConversationRepository::new(pool.clone());
    let conv_id = ConversationId::new();

    conv_repo
        .create_conversation(&conv_id)
        .await
        .expect("Failed to create conversation");

    let msg = Message {
        id: Uuid::now_v7(),
        conversation_id: conv_id,
        role: Role::User,
        content: "Test Message".to_string(),
        metadata: HashMap::new(),
    };

    conv_repo
        .create(&msg)
        .await
        .expect("Failed to create message");

    let fetched_msg = conv_repo
        .get(&msg.id)
        .await
        .expect("Failed to fetch message")
        .expect("Message not found");
    assert_eq!(msg, fetched_msg);

    let messages = conv_repo
        .get_messages_by_conversation(&conv_id)
        .await
        .expect("Failed to fetch conversation messages");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0], msg);

    let mut updated_msg = msg.clone();
    updated_msg.content = "Updated Message".to_string();
    conv_repo
        .update(&updated_msg)
        .await
        .expect("Failed to update message");

    conv_repo
        .delete(&msg.id)
        .await
        .expect("Failed to delete message");
    let deleted_msg = conv_repo
        .get(&msg.id)
        .await
        .expect("Failed to fetch deleted message");
    assert!(deleted_msg.is_none());
}
