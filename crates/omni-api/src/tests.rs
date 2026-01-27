use crate::auth::AuthService;
use crate::client::ApiClient;
use crate::device::DeviceService;
use crate::network::NetworkService;

#[tokio::test]
async fn test_device_flow_init() {
    let mut server = Server::new_async().await;
    let url = server.url();

    let mock = server
        .mock("POST", "/oauth/device/code")
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

    let client = ApiClient::new(url, None);
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
        .mock("GET", "/virtual-networks/all/list")
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

    let client = ApiClient::new(url, None);
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
        .mock("POST", "/devices/heartbeat")
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

    let client = ApiClient::new(url, None);
    let device_service = DeviceService::new(&client);
    let hb = device_service.heartbeat("hw-123").await.unwrap();

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
            "/virtual-networks/vn-1/devices/dev-1/select-exit-node",
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(json!({ "code": 200, "message": "Success", "data": null }).to_string())
        .create_async()
        .await;

    let client = ApiClient::new(url, None);
    let net_service = NetworkService::new(&client);
    let result = net_service
        .select_exit_node("vn-1", "dev-1", Some("exit-1"))
        .await;

    assert!(result.is_ok());
    mock.assert_async().await;
}
