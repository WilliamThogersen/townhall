//! Run independent jobs at once and keep their order.

use std::future::Future;

use tokio::task::{JoinError, JoinSet};

use crate::{Error, Result};

/// Every job's value in job order, or the first error. A panic in a job is
/// resumed here.
pub(crate) async fn join_all<T, F>(jobs: Vec<F>) -> Result<Vec<T>>
where
    F: Future<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
{
    let mut set = JoinSet::new();
    for (index, job) in jobs.into_iter().enumerate() {
        set.spawn(async move { (index, job.await) });
    }
    let mut done = Vec::with_capacity(set.len());
    while let Some(joined) = set.join_next().await {
        let (index, result) = joined.map_err(lost)?;
        done.push((index, result?));
    }
    done.sort_by_key(|(index, _)| *index);
    Ok(done.into_iter().map(|(_, value)| value).collect())
}

/// Run blocking work, such as file system calls, off the async executor.
pub(crate) async fn blocking<T, F>(work: F) -> Result<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work).await.map_err(lost)
}

fn lost(e: JoinError) -> Error {
    if e.is_panic() {
        std::panic::resume_unwind(e.into_panic());
    }
    Error::Io {
        context: "repo inference job".to_string(),
        source: std::io::Error::other(e),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::join_all;
    use crate::{Error, Result};

    async fn after(ms: u64, value: Result<u64>) -> Result<u64> {
        tokio::time::sleep(Duration::from_millis(ms)).await;
        value
    }

    #[tokio::test]
    async fn values_keep_job_order() {
        let jobs = vec![after(30, Ok(1)), after(0, Ok(2)), after(10, Ok(3))];
        assert_eq!(join_all(jobs).await.unwrap(), vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn blocking_work_returns_its_value() {
        assert_eq!(super::blocking(|| 7).await.unwrap(), 7);
    }

    #[tokio::test]
    async fn an_error_comes_back() {
        let jobs = vec![after(0, Ok(1)), after(0, Err(Error::NotFound("x".into())))];
        let err = join_all(jobs).await.unwrap_err();
        assert!(matches!(err, Error::NotFound(_)), "{err:?}");
    }
}
