use std::path::PathBuf;

use crate::api::prelude::InternetPayload;
use crate::proxy::send::{send_internet, send_message};
use crate::utils::io;

pub async fn parse_uri(cmd: &str) -> crate::Result<()> {
	let cmd = handle_cmd(cmd).await?;
	send_internet(cmd).await?;
	Ok(())
}

pub async fn handle_cmd(cmd: &str) -> crate::Result<InternetPayload> {
	tracing::debug!("parsing command {}", &cmd);

	if let Some(sub) = cmd.strip_prefix("onelauncher://") {
		Ok(handle_uri(sub).await?)
	} else {
		let path = io::canonicalize(PathBuf::from(cmd))?;
		if let Some(extension) = path.extension() {
			if extension == "zip" {
				return Ok(InternetPayload::InstallPath { path });
			}
		}
		send_message(&format!("invalid command filetype {}", path.display())).await?;
		Err(anyhow::anyhow!("invalid command filename {}", path.display()).into())
	}
}

pub async fn handle_uri(sub: &str) -> crate::Result<InternetPayload> {
	Ok(match sub.split_once('/') {
		// ://pkg/{id} - installs package id
		Some(("pkg", id)) => InternetPayload::InstallPackage { id: id.to_string() },
		// ://pack/{id} - installs pack id
		Some(("pack", id)) => InternetPayload::InstallPack { id: id.to_string() },
		// unknown uri command: error
		_ => {
			send_message(&format!("invalid command {sub}")).await?;
			return Err(anyhow::anyhow!("invalid command {sub}").into());
		}
	})
}
