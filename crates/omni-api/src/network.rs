use crate::client::ApiClient;
use crate::types::{
    JoinVirtualNetworkResponse, ScanResult, VirtualNetworkDeviceResponse, VirtualNetworkResponse,
};
use anyhow::Result;
use serde_json::json;

pub struct NetworkService<'a> {
    pub client: &'a ApiClient,
}

impl<'a> NetworkService<'a> {
    pub fn new(client: &'a ApiClient) -> Self {
        Self { client }
    }

    pub async fn list_all(&self) -> Result<Vec<VirtualNetworkResponse>> {
        let builder = self.client.get("/api/v2/virtual-networks/all/list");
        match self
            .client
            .send::<crate::types::ListWrapper<Vec<VirtualNetworkResponse>>>(builder)
            .await
        {
            Ok(wrapper) => Ok(wrapper.data),
            Err(e) => {
                // Fallback for cases where it might return a direct array or SuccessResponse<Vec>
                let builder = self.client.get("/api/v2/virtual-networks/all/list");
                match self.client.send::<Vec<VirtualNetworkResponse>>(builder).await {
                    Ok(nets) => Ok(nets),
                    Err(_) => Err(e), // Return original error if both fail
                }
            }
        }
    }

    pub async fn join(
        &self,
        network_id: &str,
        device_id: &str,
    ) -> Result<JoinVirtualNetworkResponse> {
        let path = format!(
            "/api/v2/virtual-networks/{}/devices/{}",
            network_id, device_id
        );
        let builder = self.client.post(&path);
        self.client.send(builder).await
    }

    pub async fn get_devices(&self, network_id: &str) -> Result<Vec<VirtualNetworkDeviceResponse>> {
        let path = format!("/api/v2/virtual-networks/{}/devices", network_id);
        let builder = self.client.get(&path);
        match self
            .client
            .send::<crate::types::ListWrapper<Vec<VirtualNetworkDeviceResponse>>>(builder)
            .await
        {
            Ok(wrapper) => Ok(wrapper.data),
            Err(e) => {
                // Fallback for cases where it might return a direct array or fail with the wrapper
                let builder = self.client.get(&path);
                self.client
                    .send::<Vec<VirtualNetworkDeviceResponse>>(builder)
                    .await
                    .map_err(|_| e)
            }
        }
    }

    pub async fn update_device(
        &self,
        network_id: &str,
        device_id: &str,
        is_exit_node: bool,
    ) -> Result<()> {
        let path = format!(
            "/api/v2/virtual-networks/{}/devices/{}",
            network_id, device_id
        );
        let builder = self.client.put(&path).json(&json!({
            "is_exit_node": is_exit_node,
        }));
        let _resp: serde_json::Value = self.client.send(builder).await?;
        Ok(())
    }

    pub async fn select_exit_node(
        &self,
        network_id: &str,
        device_id: &str,
        exit_node_id: Option<&str>,
    ) -> Result<()> {
        let path = format!(
            "/api/v2/virtual-networks/{}/devices/{}/select-exit-node",
            network_id, device_id
        );
        let builder = self.client.put(&path).json(&json!({
            "exit_node_id": exit_node_id,
        }));
        // Use a generic send that doesn't expect data
        let _resp: serde_json::Value = self.client.send(builder).await?;
        Ok(())
    }

    pub async fn upload_subnets(
        &self,
        device_id: &str,
        ip: &str,
        mac: &str,
        mask: &str,
        scans: &[ScanResult],
    ) -> Result<()> {
        let path = format!("/api/v2/devices/{}/subnets", device_id);
        let builder = self.client.post(&path).json(&json!({
            "ip": ip,
            "mac_addr": mac,
            "subnet_mask": mask,
            "devices": scans,
        }));
        let _resp: serde_json::Value = self.client.send(builder).await?;
        Ok(())
    }
}
