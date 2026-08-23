use std::io::ErrorKind;

use super::status::LoginItemStatus;
use super::user_config_dir;
use camino::Utf8PathBuf;

const ENTRY_FILE_NAME: &str = "net.obscura.vpn.gui.desktop";
const ENTRY: &str = include_str!("autostart.desktop");

fn entry_path() -> Option<Utf8PathBuf> {
    Some(user_config_dir()?.join("autostart").join(ENTRY_FILE_NAME))
}

pub async fn autostart_status() -> LoginItemStatus {
    let Some(path) = entry_path() else {
        tracing::error!(message_id = "Hm4kWc7P", "cannot determine user config dir for autostart entry");
        return LoginItemStatus { registered: false };
    };
    let registered = tokio::fs::try_exists(&path).await.unwrap_or_else(|error| {
        tracing::error!(message_id = "Qs8nBv2L", %error, %path, "failed to check for autostart entry");
        false
    });
    LoginItemStatus { registered }
}

pub async fn register_autostart() -> Result<(), ()> {
    let Some(path) = entry_path() else {
        tracing::error!(message_id = "Zf3tRk9D", "cannot determine user config dir to register autostart entry");
        return Err(());
    };
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|error| tracing::error!(message_id = "Xn6pJw4T", %error, %dir, "failed to create autostart dir"))?;
    }
    tokio::fs::write(&path, ENTRY)
        .await
        .map_err(|error| tracing::error!(message_id = "Kr2vGd8M", %error, %path, "failed to write autostart entry"))?;
    tracing::info!(message_id = "Pd5yLn3B", %path, "registered autostart entry");
    Ok(())
}

pub async fn unregister_autostart() -> Result<(), ()> {
    let Some(path) = entry_path() else {
        tracing::error!(message_id = "Wq7cFs2N", "cannot determine user config dir to unregister autostart entry");
        return Err(());
    };
    match tokio::fs::remove_file(&path).await {
        Ok(()) => {
            tracing::info!(message_id = "Jt4mXb6R", %path, "unregistered autostart entry");
            Ok(())
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => {
            tracing::error!(message_id = "Vb9sQk5H", %error, %path, "failed to remove autostart entry");
            Err(())
        }
    }
}
