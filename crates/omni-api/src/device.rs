use crate::client::ApiClient;
use crate::types::{DeviceResponse, HeartbeatResponse};
use anyhow::Result;
use serde_json::json;

pub struct DeviceService<'a> {
    pub client: &'a ApiClient,
}

impl<'a> DeviceService<'a> {
    pub fn new(client: &'a ApiClient) -> Self {
        Self { client }
    }

    pub async fn register(
        &self,
        name: &str,
        hardware_id: &str,
        os: &str,
    ) -> Result<DeviceResponse> {
        let builder = self.client.post("/devices").json(&json!({
            "name": name,
            "hardware_uuid": hardware_id,
            "platform": os,
        }));
        self.client.send(builder).await
    }

    pub async fn heartbeat(
        &self,
        hardware_id: &str,
        is_exit_node: bool,
    ) -> Result<HeartbeatResponse> {
        let builder = self.client.post("/devices/heartbeat").json(&json!({
            "hardware_id": hardware_id,
            "is_exit_node": is_exit_node,
        }));
        self.client.send(builder).await
    }

    pub async fn list_all(&self) -> Result<Vec<DeviceResponse>> {
        let builder = self.client.get("/devices");
        match self
            .client
            .send::<crate::types::ListWrapper<Vec<DeviceResponse>>>(builder)
            .await
        {
            Ok(wrapper) => Ok(wrapper.data),
            Err(e) => {
                let builder = self.client.get("/devices");
                match self.client.send::<Vec<DeviceResponse>>(builder).await {
                    Ok(devs) => Ok(devs),
                    Err(_) => Err(e),
                }
            }
        }
    }
}
