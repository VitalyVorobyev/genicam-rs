use std::net::Ipv4Addr;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Serialize;
use tracing::info;

use viva_gige::nic::IfaceSelector;

use crate::common::{self, DEFAULT_DISCOVERY_TIMEOUT_MS};

#[derive(Serialize)]
struct ExecuteResponse<'a> {
    name: &'a str,
    executed: bool,
}

/// Execute a GenApi `<Command>` feature.
///
/// Separate from `set` because a command has no value: `set --name UserSetLoad
/// --value 1` worked, since `Camera::set` dispatches commands and discards the
/// value, but nothing said so and the required `--value` reads like a mistake
/// (issue #121).
///
/// There is deliberately no read-back. `Camera::get` on a `Command` returns a
/// type error, and GenICam's `pIsDone` polling is not implemented — so the only
/// honest report is that the write was acknowledged.
pub async fn run(
    ip: Option<Ipv4Addr>,
    index: Option<usize>,
    name: String,
    iface: Option<IfaceSelector>,
    json: bool,
) -> Result<()> {
    let timeout = Duration::from_millis(DEFAULT_DISCOVERY_TIMEOUT_MS);
    let device = common::select_device(ip, index, iface.as_ref(), timeout).await?;
    info!(ip = %device.ip, "opening camera for execute");
    let mut camera = common::open_camera(&device)
        .await
        .context("open camera for execute")?;
    camera
        .execute_command(&name)
        .with_context(|| format!("execute command {name}"))?;

    if json {
        common::print_json(&ExecuteResponse {
            name: &name,
            executed: true,
        })?;
    } else {
        println!("{name} executed");
    }

    Ok(())
}
