// https://www.freedesktop.org/wiki/Specifications/file-manager-interface/
#[zbus::proxy(
    interface = "org.freedesktop.FileManager1",
    default_service = "org.freedesktop.FileManager1",
    default_path = "/org/freedesktop/FileManager1"
)]
trait FileManager1 {
    async fn show_items(&self, uris: Vec<&str>, startup_id: &str) -> zbus::Result<()>;
}

pub async fn reveal_item_in_dir(path: &str) -> Result<(), ()> {
    let url = url::Url::from_file_path(path).map_err(|()| tracing::error!(message_id = "vN3qXw8D", path, "cannot build file url to reveal item"))?;
    let connection = zbus::Connection::session()
        .await
        .map_err(|error| tracing::error!(message_id = "kD7pRv2S", ?error, "failed to connect to session bus to reveal item"))?;
    let proxy = FileManager1Proxy::new(&connection)
        .await
        .map_err(|error| tracing::error!(message_id = "jW5tBn9G", ?error, "failed to create file manager proxy to reveal item"))?;
    proxy
        .show_items(vec![url.as_ref()], "")
        .await
        .map_err(|error| tracing::error!(message_id = "rT2mFx6K", ?error, path, "failed to reveal item in file manager"))?;
    Ok(())
}
