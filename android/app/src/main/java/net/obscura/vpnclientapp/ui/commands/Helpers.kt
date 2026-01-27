package net.obscura.vpnclientapp.ui.commands

import android.content.Context
import android.content.Intent
import java.io.File
import net.obscura.vpnclientapp.sharing.DebugArchiveFileProvider

fun shareDebugArchive(
    context: Context,
    path: String,
    email: Boolean,
    subject: String? = null,
    body: String? = null,
) {
    val uri = DebugArchiveFileProvider.getUriForFile(
        context,
        "${context.packageName}.debug_archive_file_provider",
        File(path),
    );
    val intent = Intent(Intent.ACTION_SEND).apply {
        this.type = "application/zip"
        this.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        this.putExtra(Intent.EXTRA_STREAM, uri)
        if (email) {
            this.putExtra(Intent.EXTRA_EMAIL, arrayOf("support@obscura.net"))
            this.putExtra(Intent.EXTRA_SUBJECT, subject)
            this.putExtra(Intent.EXTRA_TEXT, body)
        }
    }
    if (email) {
        // There unfortunately isn't a way to only show email apps *and* have attachments. By not
        // using the chooser here, we at least give the user the option to save their previously
        // selected email app.
        context.startActivity(intent)
    } else {
        context.startActivity(Intent.createChooser(intent, null))
    }
}
