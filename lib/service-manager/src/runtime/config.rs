// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Formal init owns its configuration; bootstrap selects only the executable.

use serde::Deserialize;
use std::io::Read;

use super::Error;

const PATH: &str = "/system/etc/hyper/init.json";
const MAX_BYTES: usize = 4096;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Config {
    pub(super) services: String,
    #[serde(default, rename = "root-switch")]
    pub(super) root_switch: Option<super::root_switch::Config>,
}

impl Config {
    pub(super) fn load() -> Result<Self, Error> {
        let file = std::fs::File::open(PATH).map_err(|_| Error::Source)?;
        let mut text = String::new();
        file.take(MAX_BYTES as u64 + 1)
            .read_to_string(&mut text)
            .map_err(|_| Error::Source)?;
        Self::parse(&text)
    }

    fn parse(text: &str) -> Result<Self, Error> {
        if text.len() > MAX_BYTES {
            return Err(Error::Source);
        }
        let config: Self = serde_json::from_str(text).map_err(|_| Error::Source)?;
        if !config.services.starts_with('/') || config.services.as_bytes().contains(&0) {
            return Err(Error::Source);
        }
        if let Some(root) = &config.root_switch {
            root.validate().map_err(|_| Error::Source)?;
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formal_policy_is_validated_without_bootstrap_configuration() {
        assert!(Config::parse(r#"{"services":"/system/etc/custom-services.json"}"#).is_ok());
        for text in [
            r#"{"services":"relative.json"}"#,
            r#"{"services":"/a\u0000b"}"#,
            r#"{"services":"/services.json","root-switch":{"new-root":"relative","put-old":"/boot"}}"#,
            r#"{"system-init":"/system/svc/init-system","services":"/services.json"}"#,
        ] {
            assert!(Config::parse(text).is_err());
        }
    }
}
