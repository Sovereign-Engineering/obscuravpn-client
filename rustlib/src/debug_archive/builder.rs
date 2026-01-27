use super::zipper::Zipper;
use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use std::fmt::{Debug, Display};

#[derive(Debug)]
pub struct DebugArchiveBuilder {
    zipper: Zipper,
}

impl DebugArchiveBuilder {
    pub fn new() -> anyhow::Result<Self> {
        let dst_parent = Utf8PathBuf::try_from(std::env::temp_dir())
            .context("temp dir path wasn't valid UTF-8")?
            .join("debug-archives");
        std::fs::create_dir_all(&dst_parent).with_context(|| format!("failed to create dirs for {dst_parent:?}"))?;
        let zipper = Zipper::new(
            &dst_parent,
            format!("Obscura Debugging Archive {}", Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
        )?;
        Ok(Self { zipper })
    }

    fn write_error(&mut self, name: &str, error: impl Debug + Display) {
        tracing::error!(message_id = "dezX8SLf", ?error, "failed to archive {name:?}");
        if let Err(error) = self.zipper.write_file(format!("archive-error-{name}.txt"), error.to_string().as_bytes()) {
            tracing::error!(message_id = "eWLYHIG7", ?error, "failed to archive error for {name:?}");
        }
    }

    fn add(&mut self, name: &str, f: impl FnOnce(&mut Zipper) -> anyhow::Result<()>) {
        if let Err(error) = f(&mut self.zipper) {
            self.write_error(name, error);
        }
    }

    pub fn add_bytes(&mut self, name: &str, ext: &str, data: &[u8]) {
        self.add(name, |zipper| zipper.write_file(format!("{name}.{ext}"), data));
    }

    pub fn add_txt(&mut self, name: &str, text: &str) {
        self.add_bytes(name, "txt", text.as_bytes());
    }

    #[allow(unused)]
    pub fn add_json(&mut self, name: &str, value: &impl Serialize) {
        self.add(name, |zipper| {
            zipper.write_file(format!("{name}.json"), &serde_json::to_vec_pretty(value)?)
        });
    }

    pub fn add_path(&mut self, name: &str, ext: Option<&str>, path: &Utf8Path) {
        self.add(name, |zipper| {
            if let Some(ext) = ext {
                zipper.copy_from_fs(path, format!("{name}.{ext}").as_ref())
            } else {
                zipper.copy_from_fs(path, name.as_ref())
            }
        });
    }

    pub fn add_cmd(&mut self, name: &str, ext: &str, mut cmd: diva::Command) {
        self.add(name, |zipper| {
            let output = cmd.run_and_wait_for_output()?;
            zipper.write_file(format!("{name}-stdout.{ext}"), output.stdout())?;
            zipper.write_file(format!("{name}-stderr.txt"), output.stderr())?;
            if !output.success()
                && let Some(code) = output.status().code()
            {
                zipper.write_file(format!("{name}-status.txt"), &code.to_le_bytes())?;
            }
            Ok(())
        });
    }

    pub fn finish(self) -> anyhow::Result<Utf8PathBuf> {
        self.zipper.finish()
    }
}
