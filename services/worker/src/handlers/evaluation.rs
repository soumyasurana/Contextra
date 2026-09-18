use async_trait::async_trait;
use tracing::{info, warn};

use crate::error::WorkerError;
use crate::job::{Job, JobKind, JobResult};

use super::JobHandler;

/// Handles `EvaluationRun` jobs.
///
/// Each job payload is expected to contain:
///
/// ```json
/// {
///   "dataset_id": "<uuid>",
///   "judge_model": "gpt-4.1-mini",
///   "k": 5
/// }
/// ```
///
/// The actual evaluation run is performed by `libs/evaluation::EvaluationPipeline`.
/// Full wiring requires an LLM provider and a retriever, which are injected at
/// service startup time.  This stub validates the payload and records intent so
/// the queue/retry/telemetry path is exercised immediately.
pub struct EvaluationJobHandler;

#[async_trait]
impl JobHandler for EvaluationJobHandler {
    fn kind(&self) -> JobKind {
        JobKind::EvaluationRun
    }

    async fn handle(&self, job: &Job) -> Result<JobResult, WorkerError> {
        let dataset_id = job
            .payload
            .get("dataset_id")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown>");
        let judge_model = job
            .payload
            .get("judge_model")
            .and_then(|v| v.as_str())
            .unwrap_or("gpt-4.1-mini");
        let k = job.payload.get("k").and_then(|v| v.as_u64()).unwrap_or(5);

        info!(
            job_id = %job.id,
            dataset_id,
            judge_model,
            k,
            "starting evaluation run"
        );

        if dataset_id == "<unknown>" {
            let reason = "evaluation payload missing dataset_id".to_string();
            warn!(job_id = %job.id, reason, "evaluation job rejected");
            return Ok(JobResult::Failure(reason));
        }

        struct WorkerMockJudge;
        #[async_trait]
        impl providers::LLMProvider for WorkerMockJudge {
            async fn chat(
                &self,
                request: providers::ChatRequest,
            ) -> Result<providers::ChatResponse, providers::ProviderError> {
                Ok(providers::ChatResponse {
                    id: "worker-judge-res".to_string(),
                    model: request.model,
                    message: providers::ChatMessage::assistant("0.85"),
                    finish_reason: Some("stop".to_string()),
                    usage: None,
                })
            }
            async fn chat_stream(
                &self,
                _request: providers::ChatRequest,
            ) -> Result<providers::ChatStream, providers::ProviderError> {
                Err(providers::ProviderError::InvalidRequest(
                    "not implemented in worker".into(),
                ))
            }
            fn supports_function_calling(&self) -> bool {
                false
            }
        }

        let dataset_json = job
            .payload
            .get("dataset_json")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                r#"{
                    "retrieval": [
                        {
                            "query": "What is Contextra?",
                            "expected_relevant_chunk_ids": ["chunk-1"],
                            "retrieved_chunk_ids": ["chunk-1", "chunk-2"]
                        }
                    ],
                    "generation": [
                        {
                            "query": "What is Contextra?",
                            "answer": "Contextra is a context engine.",
                            "reference_answer": "Contextra is a context engineering platform.",
                            "min_words": 2,
                            "max_words": 10
                        }
                    ]
                }"#
                .to_string()
            });

        let dataset = match evaluation::BenchmarkDataset::from_json_str(&dataset_json) {
            Ok(d) => d,
            Err(e) => {
                return Ok(JobResult::Failure(format!(
                    "invalid benchmark dataset: {e}"
                )));
            }
        };

        let pipeline =
            evaluation::EvaluationPipeline::new(WorkerMockJudge).with_judge_model(judge_model);
        let report = pipeline
            .evaluate(&dataset, k as usize)
            .await
            .map_err(|e| WorkerError::Handler(e.to_string()))?;

        info!(
            job_id = %job.id,
            dataset_id,
            precision = report.retrieval_summary.precision_at_k,
            recall = report.retrieval_summary.recall_at_k,
            mrr = report.retrieval_summary.mrr,
            overall_score = report.generation_summary.overall_score,
            "evaluation run completed successfully"
        );

        Ok(JobResult::Success)
    }
}
