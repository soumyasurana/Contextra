use async_trait::async_trait;
use tracing::{info, warn};

use crate::error::WorkerError;
use crate::job::{Job, JobKind, JobResult};

use super::JobHandler;

/// Handles `DocumentIngestion` jobs.
///
/// Each job payload is expected to contain at minimum:
///
/// ```json
/// {
///   "document_id": "<uuid>",
///   "collection_id": "<uuid>",
///   "source_path": "/path/to/file.pdf"
/// }
/// ```
///
/// The actual heavy-lifting (parsing → chunking → embedding → upsert) is
/// performed by `libs/ingestion::IngestionPipeline`.  Full wiring of that
/// pipeline requires the vector store and embedding provider to be
/// dependency-injected here; the current implementation logs the intent and
/// succeeds so the rest of the worker infrastructure (queue, retry, telemetry)
/// can be exercised end-to-end while provider integration is completed.
pub struct IngestionJobHandler;

#[async_trait]
impl JobHandler for IngestionJobHandler {
    fn kind(&self) -> JobKind {
        JobKind::DocumentIngestion
    }

    async fn handle(&self, job: &Job) -> Result<JobResult, WorkerError> {
        let document_id = job
            .payload
            .get("document_id")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown>");
        let collection_id = job
            .payload
            .get("collection_id")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown>");
        let source_path = job
            .payload
            .get("source_path")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown>");

        info!(
            job_id = %job.id,
            document_id,
            collection_id,
            source_path,
            "starting document ingestion"
        );

        // Validate that required fields are present.
        if document_id == "<unknown>" || collection_id == "<unknown>" {
            let reason = "ingestion payload missing document_id or collection_id".to_string();
            warn!(job_id = %job.id, reason, "ingestion job rejected");
            return Ok(JobResult::Failure(reason));
        }

        if source_path != "<unknown>" {
            let path = std::path::Path::new(source_path);
            if path.exists() {
                let parsed_col_id = collection_id
                    .parse::<types::CollectionId>()
                    .unwrap_or_else(|_| types::CollectionId::new());
                let store = storage::vector_store::InMemoryVectorStore::new();
                let chunker = match ingestion::FixedSizeChunker::new(512, 64) {
                    Ok(c) => c,
                    Err(e) => return Err(WorkerError::Handler(e.to_string())),
                };

                struct WorkerMockEmbedder;
                #[async_trait]
                impl embeddings::EmbeddingProvider for WorkerMockEmbedder {
                    async fn embed_batch(
                        &self,
                        inputs: &[String],
                    ) -> Result<Vec<embeddings::Embedding>, embeddings::EmbeddingError>
                    {
                        Ok(inputs.iter().map(|_| vec![0.1f32; 1536]).collect())
                    }
                    fn dimensions(&self) -> usize {
                        1536
                    }
                    fn model_name(&self) -> &str {
                        "worker-mock-embeddings"
                    }
                }

                let result = if path.extension().and_then(|e| e.to_str()) == Some("md") {
                    let pipeline = ingestion::IngestionPipeline::new(
                        ingestion::MarkdownParser,
                        chunker,
                        WorkerMockEmbedder,
                        store,
                        collection_id.to_string(),
                        parsed_col_id,
                    );
                    pipeline
                        .ingest_path(path)
                        .await
                        .map_err(|e| WorkerError::Handler(e.to_string()))?
                } else {
                    let pipeline = ingestion::IngestionPipeline::new(
                        ingestion::PlainTextParser,
                        chunker,
                        WorkerMockEmbedder,
                        store,
                        collection_id.to_string(),
                        parsed_col_id,
                    );
                    pipeline
                        .ingest_path(path)
                        .await
                        .map_err(|e| WorkerError::Handler(e.to_string()))?
                };

                info!(
                    job_id = %job.id,
                    document_id,
                    chunks_count = result.chunks.len(),
                    "document ingestion completed successfully"
                );
            } else {
                info!(
                    job_id = %job.id,
                    document_id,
                    source_path,
                    "document ingestion accepted for virtual/remote path"
                );
            }
        } else {
            info!(
                job_id = %job.id,
                document_id,
                "document ingestion job accepted without source_path"
            );
        }

        Ok(JobResult::Success)
    }
}
