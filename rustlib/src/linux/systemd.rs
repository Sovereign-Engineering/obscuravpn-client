use zbus_systemd::systemd1::{ManagerProxy, UnitProxy};
use zbus_systemd::zbus;
use zbus_systemd::zbus::proxy::CacheProperties;

pub const UNIT_NAME: &str = "obscura.service";

#[derive(Debug, Clone, Copy, strum::Display)]
#[strum(serialize_all = "kebab-case")]
pub enum SystemdUnitStatus {
    NotInstalled,
    Unknown,
    Active,
    Reloading,
    Refreshing,
    Inactive,
    Failed,
    Activating,
    Deactivating,
}

impl SystemdUnitStatus {
    pub async fn get() -> Self {
        match Self::query().await {
            Ok(unit_status) => unit_status,
            Err(()) => Self::Unknown,
        }
    }

    async fn query() -> Result<Self, ()> {
        let conn = zbus::Connection::system()
            .await
            .map_err(|error| tracing::debug!(message_id = "gW4nRc7K", %error, "failed to connect to the system bus"))?;
        let systemd = ManagerProxy::builder(&conn)
            .cache_properties(CacheProperties::No)
            .build()
            .await
            .map_err(|error| tracing::debug!(message_id = "sN8kVb3T", %error, "failed to create systemd manager proxy"))?;
        match systemd.get_unit_file_state(UNIT_NAME.to_owned()).await {
            Err(zbus::Error::MethodError(ref name, _, _)) if name.as_str() == "org.freedesktop.DBus.Error.FileNotFound" => {
                return Ok(Self::NotInstalled);
            }
            Err(error) => {
                tracing::debug!(message_id = "cJ2xPm9F", %error, "failed to query service unit file state");
                return Err(());
            }
            Ok(_) => {}
        }
        let path = systemd
            .load_unit(UNIT_NAME.to_owned())
            .await
            .map_err(|error| tracing::debug!(message_id = "uD6tHw2Q", %error, "failed to load service unit"))?;
        let unit = UnitProxy::builder(&conn)
            .cache_properties(CacheProperties::No)
            .path(path)
            .map_err(|error| tracing::debug!(message_id = "bK3wNd8S", %error, "invalid service unit path"))?
            .build()
            .await
            .map_err(|error| tracing::debug!(message_id = "gV5cTm2X", %error, "failed to create unit proxy"))?;
        let active_state = unit
            .active_state()
            .await
            .map_err(|error| tracing::debug!(message_id = "wQ9fJb6L", %error, "failed to query service active state"))?;
        Ok(match active_state.as_str() {
            "active" => Self::Active,
            "reloading" => Self::Reloading,
            "refreshing" => Self::Refreshing,
            "inactive" => Self::Inactive,
            "failed" => Self::Failed,
            "activating" => Self::Activating,
            "deactivating" => Self::Deactivating,
            _ => {
                tracing::warn!(message_id = "hT5mBw8R", active_state, "unrecognized systemd unit active state");
                Self::Unknown
            }
        })
    }
}
