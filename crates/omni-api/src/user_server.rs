use crate::client::ApiClient;
use crate::types::{CreateUserServerRequest, UpdateUserServerRequest, UserServer};
use anyhow::Result;

pub struct UserServerService<'a> {
    pub client: &'a ApiClient,
}

impl<'a> UserServerService<'a> {
    pub fn new(client: &'a ApiClient) -> Self {
        Self { client }
    }

    pub async fn list(&self) -> Result<Vec<UserServer>> {
        let builder = self.client.get("/user-servers");
        match self
            .client
            .send::<crate::types::ListWrapper<Vec<UserServer>>>(builder)
            .await
        {
            Ok(wrapper) => Ok(wrapper.data),
            Err(e) => {
                let builder = self.client.get("/user-servers");
                match self.client.send::<Vec<UserServer>>(builder).await {
                    Ok(servers) => Ok(servers),
                    Err(_) => Err(e),
                }
            }
        }
    }

    pub async fn create(&self, request: CreateUserServerRequest) -> Result<UserServer> {
        let builder = self.client.post("/user-servers").json(&request);
        self.client.send(builder).await
    }

    pub async fn update(&self, id: &str, request: UpdateUserServerRequest) -> Result<UserServer> {
        let path = format!("/user-servers/{}", id);
        let builder = self.client.put(&path).json(&request);
        self.client.send(builder).await
    }
}
