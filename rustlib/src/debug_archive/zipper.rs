use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use std::{
    fs::File,
    io::{Read as _, Write as _},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

#[derive(Debug)]
pub struct Zipper {
    name: String,
    dst: Utf8PathBuf,
    writer: ZipWriter<File>,
    options: SimpleFileOptions,
    buf: Vec<u8>,
}

impl Zipper {
    pub fn new(dst_parent: &Utf8Path, name: String) -> anyhow::Result<Self> {
        let mut dst = dst_parent.join(&name);
        dst.set_extension("zip");
        tracing::debug!(message_id = "tpKqzGkz", ?dst, "starting archive");
        let writer = ZipWriter::new(File::create(&dst)?);
        Ok(Self {
            name,
            dst,
            writer,
            options: SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
            buf: Vec::new(),
        })
    }

    fn root(&self) -> &Utf8Path {
        self.name.as_ref()
    }

    /// `dst` is relative to the archive root.
    /// Parent directories must be created using `create_dir` first.
    #[allow(unused)]
    pub fn create_dir(&mut self, dst: impl AsRef<Utf8Path>) -> anyhow::Result<()> {
        self.writer
            .add_directory(Utf8PathBuf::from_iter([self.root(), dst.as_ref()]), self.options)
            .map_err(Into::into)
    }

    /// `dst` is relative to the archive root.
    /// Parent directories must be created using `create_dir` first.
    pub fn write_file(&mut self, dst: impl AsRef<Utf8Path>, data: &[u8]) -> anyhow::Result<()> {
        self.writer
            .start_file(Utf8PathBuf::from_iter([self.root(), dst.as_ref()]), self.options)?;
        let result = self.writer.write_all(data);
        if result.is_err() {
            self.writer.abort_file()?;
        }
        result.map_err(Into::into)
    }

    /// Recursively copies existing files into the archive. If any errors are
    /// encountered while recursing, that path is skipped.
    ///
    /// `dst` is relative to the archive root.
    /// Parent directories must be created using `create_dir` first.
    pub fn copy_from_fs(&mut self, src: &Utf8Path, dst: &Utf8Path) -> anyhow::Result<()> {
        let dst_abs = Utf8PathBuf::from_iter([self.root(), dst]);
        let metadata = src.metadata()?;
        tracing::debug!(message_id = "r9oFrh5g", ?src, ?dst, ?metadata, "started copy");
        if metadata.is_dir() {
            self.writer.add_directory(dst_abs.as_str(), self.options)?;
            for entry in src.read_dir_utf8()? {
                let entry = entry?;
                let entry_path = entry.path();
                let file_name = entry_path
                    .file_name()
                    .with_context(|| format!("entry path {entry_path:?} had no file name"))?;
                if let Err(error) = self.copy_from_fs(entry_path, &dst.join(file_name)) {
                    tracing::error!(message_id = "3MrmBtrD", ?error, ?entry, "failed to copy entry; skipping");
                }
            }
        } else if metadata.is_file() {
            let mut file = File::open(src)?;
            file.read_to_end(&mut self.buf)?;
            self.writer.start_file(&dst_abs, self.options)?;
            let result = self.writer.write_all(&self.buf);
            self.buf.clear();
            if result.is_err() {
                self.writer.abort_file()?;
            }
            result?;
        } else {
            anyhow::bail!("neither a dir nor a regular file: {src:?}");
        }
        tracing::debug!(message_id = "UfZicz2T", ?src, ?dst, ?metadata, "finished copy");
        Ok(())
    }

    pub fn finish(self) -> anyhow::Result<Utf8PathBuf> {
        self.writer.finish()?;
        tracing::debug!(message_id = "1HFVjluv", dst =? self.dst, "finished archive");
        Ok(self.dst)
    }
}
