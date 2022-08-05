# OmniEdge Docker

Run OmniEdge in a container

## Usage

```
sudo docker run -d \
  -e OMNIEDGE_SECURITYKEY=OMNIEDGE_SECURITYKEY \
  -e OMNIEDGE_VIRUTALNETWORK_ID="OMNIEDGE_VIRUTALNETWORK_ID" \
  --network host \
  --privileged \
  omniedge/omniedge:latest
```