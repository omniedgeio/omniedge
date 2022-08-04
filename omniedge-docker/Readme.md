# OmniEdge Docker

Run OmniEdge in a container

## Usage

```
sudo docker run -d \
  -e OMNIEDGE_SECURITYKEY=OMNIEDGE_SECURITYKEY \
  -e OMNIEDGE_VIRUTALNETWORK_ID="vnw_ddddddddddd" \
  --network host \
  --privileged \
  omniedge/omniedge:latest
```