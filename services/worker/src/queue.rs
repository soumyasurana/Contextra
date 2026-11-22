use async_trait::async_trait;
use redis::{AsyncCommands, Client, Script};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

use crate::error::WorkerError;
use crate::job::Job;

// ---------------------------------------------------------------------------
// Retry / backoff configuration
// ---------------------------------------------------------------------------

/// Maximum number of delivery attempts before a job is dead-lettered.
pub const MAX_RETRIES: u32 = 3;
/// Base delay for exponential backoff on retry.
pub const BASE_BACKOFF: Duration = Duration::from_secs(5);
/// Hard ceiling on the computed backoff delay.
pub const MAX_BACKOFF: Duration = Duration::from_secs(60);

/// Compute the next `scheduled_at` Unix timestamp after a failed attempt.
pub fn next_scheduled_at(retry_count: u32) -> u64 {
    let base_secs = BASE_BACKOFF.as_secs();
    // 2^retry_count, clamped to avoid overflow (retry_count is small in practice)
    let multiplier = 1_u64.checked_shl(retry_count).unwrap_or(u64::MAX);
    let delay_secs = (base_secs.saturating_mul(multiplier)).min(MAX_BACKOFF.as_secs());
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .saturating_add(delay_secs)
}

// ---------------------------------------------------------------------------
// JobQueue trait
// ---------------------------------------------------------------------------

/// Abstraction over a durable, ordered job queue.
///
/// `RedisJobQueue` provides a production Redis-backed implementation;
/// `InMemoryJobQueue` provides a lightweight in-process version used in tests.
#[async_trait]
pub trait JobQueue: Send + Sync {
    /// Push a job to the tail of the ready queue.
    async fn enqueue(&self, job: &Job) -> Result<(), WorkerError>;

    /// Block until a job is available (up to `timeout`) and return it.
    ///
    /// Returns `Ok(None)` if the timeout elapsed without a job becoming ready.
    async fn dequeue(&self, timeout: Duration) -> Result<Option<Job>, WorkerError>;

    /// Acknowledge successful processing — remove the job from the in-flight
    /// tracking list.
    async fn ack(&self, job_id: &str) -> Result<(), WorkerError>;

    /// Negatively acknowledge a failed job.
    ///
    /// If `job.retry_count < MAX_RETRIES`, the job is re-enqueued with an
    /// incremented retry counter and an exponential backoff delay applied to
    /// `scheduled_at`.  Otherwise it is pushed to the dead-letter queue.
    async fn nack(&self, job: &Job) -> Result<(), WorkerError>;
}

// ---------------------------------------------------------------------------
// Redis key constants
// ---------------------------------------------------------------------------

/// Jobs waiting to be picked up by a worker.
const QUEUE_KEY: &str = "worker:queue";
/// Jobs currently being processed.
const PROCESSING_KEY: &str = "worker:processing";
/// Jobs waiting for their scheduled time.
const DELAYED_KEY: &str = "worker:delayed";
/// Jobs that exceeded `MAX_RETRIES` and need manual intervention.
const DLQ_KEY: &str = "worker:dlq";

// ---------------------------------------------------------------------------
// RedisJobQueue
// ---------------------------------------------------------------------------

/// Redis-backed queue with atomic claims and retry transitions.
pub struct RedisJobQueue {
    connection: redis::aio::MultiplexedConnection,
}

impl RedisJobQueue {
    pub async fn connect(url: &str) -> Result<Self, WorkerError> {
        let client = Client::open(url)
            .map_err(|error| WorkerError::Queue(format!("failed to open Redis client: {error}")))?;
        let connection = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|error| WorkerError::Queue(format!("failed to connect to Redis: {error}")))?;
        Ok(Self { connection })
    }
}

// Move jobs atomically into the processing list. Inspect the ready list for
// delayed entries produced by older workers and move them to the sorted set.
const CLAIM_JOB: &str = r#"
local now = tonumber(redis.call('TIME')[1])
local due = redis.call('ZRANGEBYSCORE', KEYS[2], '-inf', now, 'LIMIT', 0, 1)
if due[1] then
    redis.call('RPUSH', KEYS[3], due[1])
    redis.call('ZREM', KEYS[2], due[1])
    return due[1]
end
for i = 1, 32 do
    local raw = redis.call('LINDEX', KEYS[1], 0)
    if not raw then return nil end
    local ok, job = pcall(cjson.decode, raw)
    if not ok or type(job) ~= 'table' or type(job.scheduled_at) ~= 'number' or type(job.id) ~= 'string' then
        redis.call('RPUSH', KEYS[4], raw)
    elseif job.scheduled_at > now then
        redis.call('ZADD', KEYS[2], job.scheduled_at, raw)
    else
        redis.call('RPUSH', KEYS[3], raw)
        redis.call('LPOP', KEYS[1])
        return raw
    end
    redis.call('LPOP', KEYS[1])
end
return nil
"#;

// Move the failed attempt before removing it from processing. Match the
// attempt as well as the ID so repeating a failed transition is harmless.
const FAIL_JOB: &str = r#"
for _, raw in ipairs(redis.call('LRANGE', KEYS[1], 0, -1)) do
    local ok, job = pcall(cjson.decode, raw)
    if ok and type(job) == 'table' and job.id == ARGV[1] and job.retry_count == tonumber(ARGV[2]) then
        if ARGV[3] == 'retry' then
            redis.call('ZADD', KEYS[2], ARGV[4], ARGV[5])
        else
            redis.call('RPUSH', KEYS[3], ARGV[5])
        end
        return redis.call('LREM', KEYS[1], 1, raw)
    end
end
return 0
"#;

#[async_trait]
impl JobQueue for RedisJobQueue {
    async fn enqueue(&self, job: &Job) -> Result<(), WorkerError> {
        let serialized = serde_json::to_string(job)?;
        let mut connection = self.connection.clone();
        let _: i64 = connection
            .rpush(QUEUE_KEY, serialized)
            .await
            .map_err(|error| WorkerError::Queue(format!("failed to enqueue job: {error}")))?;
        Ok(())
    }

    async fn dequeue(&self, timeout: Duration) -> Result<Option<Job>, WorkerError> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut connection = self.connection.clone();
        let script = Script::new(CLAIM_JOB);

        loop {
            let raw: Option<String> = script
                .key(QUEUE_KEY)
                .key(DELAYED_KEY)
                .key(PROCESSING_KEY)
                .key(DLQ_KEY)
                .invoke_async(&mut connection)
                .await
                .map_err(|error| WorkerError::Queue(format!("failed to claim job: {error}")))?;
            if let Some(raw) = raw {
                return match serde_json::from_str(&raw) {
                    Ok(job) => Ok(Some(job)),
                    Err(error) => {
                        let _: () = redis::pipe()
                            .atomic()
                            .rpush(DLQ_KEY, &raw)
                            .ignore()
                            .lrem(PROCESSING_KEY, 1, &raw)
                            .ignore()
                            .query_async(&mut connection)
                            .await
                            .map_err(|error| {
                                WorkerError::Queue(format!(
                                    "failed to dead-letter invalid job: {error}"
                                ))
                            })?;
                        Err(error.into())
                    }
                };
            }

            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }
            tokio::time::sleep(remaining.min(Duration::from_millis(200))).await;
        }
    }

    async fn ack(&self, job_id: &str) -> Result<(), WorkerError> {
        let mut connection = self.connection.clone();
        let entries: Vec<String> =
            connection
                .lrange(PROCESSING_KEY, 0, -1)
                .await
                .map_err(|error| {
                    WorkerError::Queue(format!("failed to read processing list: {error}"))
                })?;

        for entry in entries {
            if let Ok(partial) = serde_json::from_str::<serde_json::Value>(&entry)
                && partial.get("id").and_then(|value| value.as_str()) == Some(job_id)
            {
                let _: i64 = connection
                    .lrem(PROCESSING_KEY, 1, entry)
                    .await
                    .map_err(|error| {
                        WorkerError::Queue(format!("failed to acknowledge job: {error}"))
                    })?;
                break;
            }
        }
        Ok(())
    }

    async fn nack(&self, job: &Job) -> Result<(), WorkerError> {
        let retry = job.retry_count < MAX_RETRIES;
        let next = if retry {
            job.with_incremented_retry(next_scheduled_at(job.retry_count))
        } else {
            job.clone()
        };
        let _: i64 = Script::new(FAIL_JOB)
            .key(PROCESSING_KEY)
            .key(DELAYED_KEY)
            .key(DLQ_KEY)
            .arg(job.id.to_string())
            .arg(job.retry_count)
            .arg(if retry { "retry" } else { "dead-letter" })
            .arg(next.scheduled_at)
            .arg(serde_json::to_string(&next)?)
            .invoke_async(&mut self.connection.clone())
            .await
            .map_err(|error| {
                WorkerError::Queue(format!("failed to reschedule failed job: {error}"))
            })?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// InMemoryJobQueue — for unit tests
// ---------------------------------------------------------------------------

/// An in-process, mutex-backed queue that satisfies the same `JobQueue` trait.
///
/// This does **not** persist state across test runs and is never meant for
/// production use. It also applies retry / DLQ logic identically to
/// `RedisJobQueue` so behaviour is tested faithfully.
#[derive(Debug, Default)]
pub struct InMemoryJobQueue {
    /// Ready jobs waiting to be dequeued.
    pub ready: Arc<Mutex<VecDeque<Job>>>,
    /// Jobs currently being processed (indexed by job id string).
    pub processing: Arc<Mutex<Vec<Job>>>,
    /// Jobs that exceeded `MAX_RETRIES`.
    pub dead_letter: Arc<Mutex<Vec<Job>>>,
}

impl InMemoryJobQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Drain all items from the dead-letter queue and return them (for test assertions).
    pub async fn drain_dlq(&self) -> Vec<Job> {
        let mut dlq = self.dead_letter.lock().await;
        dlq.drain(..).collect()
    }

    /// Snapshot the ready queue without consuming it.
    pub async fn ready_len(&self) -> usize {
        self.ready.lock().await.len()
    }
}

#[async_trait]
impl JobQueue for InMemoryJobQueue {
    async fn enqueue(&self, job: &Job) -> Result<(), WorkerError> {
        self.ready.lock().await.push_back(job.clone());
        Ok(())
    }

    async fn dequeue(&self, timeout: Duration) -> Result<Option<Job>, WorkerError> {
        // Poll with 10ms intervals up to `timeout` to simulate blocking behaviour.
        let deadline = std::time::Instant::now() + timeout.min(Duration::from_millis(500));

        loop {
            let now = std::time::Instant::now();
            {
                let mut ready = self.ready.lock().await;
                // Skip jobs whose scheduled_at is in the future.
                if let Some(pos) = ready.iter().position(|job| {
                    let now_secs = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    job.scheduled_at <= now_secs
                }) && let Some(job) = ready.remove(pos)
                {
                    self.processing.lock().await.push(job.clone());
                    return Ok(Some(job));
                }
            }
            if now >= deadline {
                return Ok(None);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    async fn ack(&self, job_id: &str) -> Result<(), WorkerError> {
        let mut processing = self.processing.lock().await;
        processing.retain(|job| job.id.to_string() != job_id);
        Ok(())
    }

    async fn nack(&self, job: &Job) -> Result<(), WorkerError> {
        self.ack(&job.id.to_string()).await?;

        if job.retry_count >= MAX_RETRIES {
            self.dead_letter.lock().await.push(job.clone());
        } else {
            let next_at = next_scheduled_at(job.retry_count);
            let retry_job = job.with_incremented_retry(next_at);
            self.ready.lock().await.push_back(retry_job);
        }
        Ok(())
    }
}
