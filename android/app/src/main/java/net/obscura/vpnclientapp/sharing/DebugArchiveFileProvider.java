package net.obscura.vpnclientapp.sharing;

import androidx.core.content.FileProvider;
import net.obscura.vpnclientapp.R;

// We need to extend `FileProvider` because some OEMs strip `meta-data` tags from the manifest:
// https://github.com/androidx/androidx/commit/a4385569db989747caf6b110b345a09ceb86acc7
// ...unfortunately, Kotlin subclasses don't inherit static methods, so we need to use Java.
public class DebugArchiveFileProvider extends FileProvider {
    public DebugArchiveFileProvider() {
        super(R.xml.debug_archive_file_provider_paths);
    }
}
