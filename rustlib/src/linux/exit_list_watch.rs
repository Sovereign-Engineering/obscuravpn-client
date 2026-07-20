use std::sync::Arc;
use std::time::Duration;

use obscuravpn_api::cmd::ExitList;
use tokio::sync::watch;
use tokio::time::sleep;
use tokio_util::task::AbortOnDropHandle;

use super::ipc::run_command;
use crate::cached_value::CachedValue;
use crate::manager_cmd::ManagerCmd;

pub struct GuiExitListWatch {
    tx: watch::Sender<Option<Arc<ExitList>>>,
    _tasks: AbortOnDropHandle<()>,
}

impl GuiExitListWatch {
    pub async fn watch() -> Arc<Self> {
        let (tx, _) = watch::channel(None);
        let task = tokio::spawn({
            let tx = tx.clone();
            async move {
                tokio::join!(run_poller(tx), run_refresher());
            }
        });
        Arc::new(Self { tx, _tasks: AbortOnDropHandle::new(task) })
    }

    pub async fn changed(&self, known: Option<&Arc<ExitList>>) -> Arc<ExitList> {
        self.tx
            .subscribe()
            .wait_for(|value| value.is_some() && value.as_ref() != known)
            .await
            .expect("sender held by self")
            .clone()
            .expect("wait_for guarantees Some")
    }
}

async fn run_poller(tx: watch::Sender<Option<Arc<ExitList>>>) {
    let mut known_version: Option<Vec<u8>> = None;
    loop {
        match run_command::<CachedValue<Arc<ExitList>>>(ManagerCmd::GetExitList { known_version: known_version.clone() }).await {
            Ok(Ok(cached)) => {
                known_version = Some(cached.version);
                tx.send_replace(Some(cached.value));
            }
            Ok(Err(error)) => {
                tracing::error!(message_id = "Fq7mVs2d", ?error, "service failed to get exit list");
                sleep(Duration::from_secs(1)).await;
            }
            Err(error) => {
                tracing::debug!(message_id = "Bn4kWy6h", %error, "cannot reach service to get exit list");
                sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

const REFRESH_INTERVAL: Duration = Duration::from_secs(3600);

async fn run_refresher() {
    loop {
        match run_command::<()>(ManagerCmd::RefreshExitList { freshness: REFRESH_INTERVAL }).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => tracing::warn!(message_id = "Gz8pLc4t", ?error, "service failed to refresh exit list"),
            Err(error) => tracing::debug!(message_id = "Dm3rHx9s", %error, "cannot reach service to refresh exit list"),
        }
        sleep(REFRESH_INTERVAL).await;
    }
}
