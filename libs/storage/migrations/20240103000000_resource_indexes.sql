CREATE INDEX IF NOT EXISTS documents_collection_id_idx ON documents (collection_id);
CREATE INDEX IF NOT EXISTS chunks_document_id_idx ON chunks (document_id);
CREATE INDEX IF NOT EXISTS messages_conversation_id_id_idx ON messages (conversation_id, id);
