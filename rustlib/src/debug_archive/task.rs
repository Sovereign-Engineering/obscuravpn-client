use serde::Deserialize;
use serde::Serialize;
use std::error::Error;
use std::time::Duration;
use std::time::Instant;
use tokio::task::JoinError;
use tokio::time::timeout;

const TASK_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Serialize, Deserialize)]
pub struct DebugTask<T> {
    total_s: f32,
    pub result: TaskResult<T>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TaskResult<T> {
    Failure(String),
    Panic(String),
    Success(T),
    Timeout,
}

impl<T> TaskResult<T> {
    pub fn get(&self) -> Option<&T> {
        match self {
            TaskResult::Success(v) => Some(v),
            _ => None,
        }
    }
}

pub async fn run_debug_task<T>(task: impl Future<Output = Result<T, Box<dyn Error>>>) -> DebugTask<T> {
    let start = Instant::now();
    let result = match timeout(TASK_TIMEOUT, task).await {
        Ok(Ok(r)) => TaskResult::Success(r),
        Ok(Err(err)) => TaskResult::Failure(format!("{:?}", err)),
        Err(_) => TaskResult::Timeout,
    };

    DebugTask { result, total_s: start.elapsed().as_secs_f32() }
}

pub fn debug_panic_error<T>(error: JoinError) -> DebugTask<T> {
    tracing::error!(message_id = "lai6Ok9e", ?error, "Debug bundle task failed",);
    DebugTask { total_s: -1.0, result: TaskResult::Panic(error.to_string()) }
}
