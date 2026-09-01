use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename = "WmiMonitorBrightness")]
#[serde(rename_all = "PascalCase")]
pub struct WmiPanel {
    pub active: Option<bool>,
    pub current_brightness: u8,
    pub instance_name: String,
}

impl WmiPanel {
    pub fn current(&self) -> u8 {
        self.current_brightness.min(100)
    }
}

#[derive(Deserialize)]
struct WmiMonitorBrightnessMethods;

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct SetBrightnessIn {
    timeout: u32,
    brightness: u8,
}

fn connect() -> Result<wmi::WMIConnection> {
    wmi::WMIConnection::with_namespace_path("root\\WMI").map_err(|e| anyhow::anyhow!("{e}"))
}

pub fn list() -> Result<Vec<WmiPanel>> {
    let con = connect()?;
    let rows: Vec<WmiPanel> = con.query().map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(rows
        .into_iter()
        .filter(|p| p.active.unwrap_or(true))
        .collect())
}

#[allow(dead_code)]
pub fn get(instance_name: &str) -> Result<u8> {
    let panels = list()?;
    panels
        .into_iter()
        .find(|p| p.instance_name == instance_name)
        .map(|p| p.current())
        .ok_or_else(|| anyhow::anyhow!("WMI instance not found"))
}

pub fn set(instance_name: &str, brightness: u8) -> Result<()> {
    let con = connect()?;
    let escaped = instance_name.replace('\\', "\\\\").replace('"', "\\\"");
    let path = format!("WmiMonitorBrightnessMethods.InstanceName=\"{escaped}\"");
    let input = SetBrightnessIn {
        timeout: 1,
        brightness: brightness.min(100),
    };
    let _: () = con
        .exec_instance_method::<WmiMonitorBrightnessMethods, ()>(
            &path,
            "WmiSetBrightness",
            input,
        )
        .or_else(|_| {
            // Some providers return a ReturnValue instead of void.
            #[derive(Deserialize)]
            #[allow(non_snake_case)]
            struct Out {
                ReturnValue: Option<u32>,
            }
            let out: Out = con.exec_instance_method::<WmiMonitorBrightnessMethods, Out>(
                &path,
                "WmiSetBrightness",
                SetBrightnessIn {
                    timeout: 1,
                    brightness: brightness.min(100),
                },
            )?;
            if out.ReturnValue.unwrap_or(0) == 0 {
                Ok(())
            } else {
                Err(anyhow::anyhow!(
                    "WmiSetBrightness returned {:?}",
                    out.ReturnValue
                ))
            }
        })
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}
