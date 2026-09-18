use async_trait::async_trait;
use tracing::info;

use crate::error::WorkerError;
use crate::job::{Job, JobKind, JobResult};

use super::JobHandler;

/// Handles `MemorySweep` jobs.
///
/// A memory sweep re-scores importance for recent messages and, where the
/// conversation token count exceeds the configured limit, synthesises a new
/// rolling summary.  These jobs are typically enqueued by the `Scheduler` on a
/// configurable periodic interval rather than in response to API requests.
///
/// The actual work is performed by `libs/memory`:
/// - `ImportanceScorer::score_message` — re-evaluate and promote memories
/// - `ConversationMemory::summarize_overflow` — produce rolling summaries
///
/// Full wiring requires a vector memory store and an LLM summariser.  This
/// stub logs the sweep intent so the scheduler → queue → worker → telemetry
/// path can be validated end-to-end.
pub struct MemorySweepJobHandler;

#[async_trait]
impl JobHandler for MemorySweepJobHandler {
    fn kind(&self) -> JobKind {
        JobKind::MemorySweep
    }

    async fn handle(&self, job: &Job) -> Result<JobResult, WorkerError> {
        let user_id = job
            .payload
            .get("user_id")
            .and_then(|v| v.as_str())
            .unwrap_or("all");
        let scope = job
            .payload
            .get("scope")
            .and_then(|v| v.as_str())
            .unwrap_or("global");

        info!(
            job_id = %job.id,
            user_id,
            scope,
            "starting memory importance-scoring sweep"
        );

        let scorer = memory::ImportanceScorer::default();
        if let Some(msg_content) = job.payload.get("message_content").and_then(|v| v.as_str()) {
            let msg = types::Message {
                id: uuid::Uuid::now_v7(),
                conversation_id: types::ConversationId::new(),
                role: types::Role::User,
                content: msg_content.to_string(),
                metadata: types::Metadata::new(),
            };
            let score = scorer.score_message(&msg);
            let promote = scorer.should_promote(&msg);
            info!(
                job_id = %job.id,
                user_id,
                score,
                promote,
                "memory importance scoring evaluated"
            );
        } else {
            info!(
                job_id = %job.id,
                user_id,
                scope,
                "memory sweep completed successfully"
            );
        }

        Ok(JobResult::Success)
    }
}
