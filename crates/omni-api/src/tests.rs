use crate::auth::AuthService;
use crate::client::ApiClient;
use crate::device::DeviceService;
use crate::network::NetworkService;

#[tokio::test]
async fn test_device_flow_init() {
    let mut server = Server::new_async().await;
    let url = server.url();

    let mock = server
        .mock("POST", "/api/v2/oauth/device/code")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            json!({
                "code": 200,
                "message": "Success",
                "data": {
                    "device_code": "dc-123",
                    "user_code": "UC-456",
                    "verification_uri": "https://example.com/verify",
                    "verification_uri_complete": "https://example.com/verify?code=UC-456",
                    "expires_in": 300,
                    "interval": 5
                }
            })
            .to_string(),
        )
        .create_async()
        .await;

    let client = ApiClient::new(format!("{}/api/v2", url), None);
    let auth_service = AuthService::new(&client);
    let dr = auth_service
        .device_flow_init("client-123", "scope")
        .await
        .unwrap();

    assert_eq!(dr.device_code, "dc-123");
    assert_eq!(dr.user_code, "UC-456");
    mock.assert_async().await;
}
use mockito::Server;
use serde_json::json;

#[tokio::test]
async fn test_list_all_networks() {
    let mut server = Server::new_async().await;
    let url = server.url();

    let mock = server
        .mock("GET", "/api/v2/virtual-networks/all/list")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            json!({
                "code": 200,
                "message": "Success",
                "data": [
                    {
                        "uuid": "net-123",
                        "name": "My Network",
                        "ip_range": "100.64.0.0/10",
                        "role": 100
                    }
                ]
            })
            .to_string(),
        )
        .create_async()
        .await;

    let client = ApiClient::new(format!("{}/api/v2", url), None);
    let net_service = NetworkService::new(&client);
    let nets = net_service.list_all().await.unwrap();

    assert_eq!(nets.len(), 1);
    assert_eq!(nets[0].id, "net-123");
    assert_eq!(nets[0].name, "My Network");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_heartbeat() {
    let mut server = Server::new_async().await;
    let url = server.url();

    let mock = server
        .mock("POST", "/api/v2/devices/heartbeat")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            json!({
                "code": 200,
                "message": "Success",
                "data": {
                    "message": "Heartbeat received",
                    "last_seen": "2026-01-24T00:00:00Z",
                    "exit_nodes": {}
                }
            })
            .to_string(),
        )
        .create_async()
        .await;

    let client = ApiClient::new(format!("{}/api/v2", url), None);
    let device_service = DeviceService::new(&client);
    let hb = device_service.heartbeat("hw-123", false).await.unwrap();

    assert_eq!(hb.message, "Heartbeat received");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_select_exit_node() {
    let mut server = Server::new_async().await;
    let url = server.url();

    let mock = server
        .mock(
            "PUT",
            "/api/v2/virtual-networks/vn-1/devices/dev-1/select-exit-node",
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(json!({ "code": 200, "message": "Success", "data": null }).to_string())
        .create_async()
        .await;

    let client = ApiClient::new(format!("{}/api/v2", url), None);
    let net_service = NetworkService::new(&client);
    let result = net_service
        .select_exit_node("vn-1", "dev-1", Some("exit-1"))
        .await;

    assert!(result.is_ok());
    mock.assert_async().await;
}

#[tokio::test]
async fn test_join_network_response_parsing() {
    use crate::types::{JoinVirtualNetworkResponse, SuccessResponse};

    // This is the exact response from the API that was failing
    let json_body = r#"{"code":200,"data":{"cluster":"rLl13SS0U0MXval","community_name":"rLl13SS0U0MXval","secret_key":"89c3b07614869022aed5771aa5f67764d1a5b5e094a83b131b8afae6777b84ee","virtual_ip":"100.100.0.158","virtual_ip_v6":null,"subnet_mask":"255.255.255.0","subnet_prefix_v6":null,"ip_range_v6":null,"server":{"name":"Australia","host":"prod-us.omniedge.io:7787","country":"AU"}}}"#;

    // First try parsing with SuccessResponse wrapper (like client.rs does)
    let wrapped_result: Result<SuccessResponse<JoinVirtualNetworkResponse>, _> =
        serde_json::from_str(json_body);

    match &wrapped_result {
        Ok(resp) => {
            assert_eq!(resp.data.cluster, "rLl13SS0U0MXval");
            assert_eq!(resp.data.virtual_ip, "100.100.0.158");
            assert_eq!(resp.data.server.host, "prod-us.omniedge.io:7787");
        }
        Err(e) => {
            panic!("Failed to parse wrapped response: {}", e);
        }
    }
}
