![OmniEdge](https://user-images.githubusercontent.com/93888/185755146-a79ad5d6-7901-4855-9efb-ae108dbdcdf6.png)

<div align="center">
  <h1>OmniEdge</h1>
  <p><strong>Secure Connectivity Fabric for AI Clusters, Humanoids, and Edge Computing</strong></p>

[![Website](https://img.shields.io/website?label=omniedge.io&url=https%3A%2F%2Fomniedge.io)](https://omniedge.io)
[![Sync Status](https://github.com/omniedgeio/omniedge/workflows/sync/badge.svg)](https://github.com/omniedgeio/omniedge/actions)
[![License](https://img.shields.io/github/license/omniedgeio/omniedge)](LICENSE)
[![Release](https://img.shields.io/github/v/release/omniedgeio/omniedge)](https://github.com/omniedgeio/omniedge/releases)
[![Docker](https://img.shields.io/docker/v/omniedge/omniedge?label=Docker)](https://hub.docker.com/r/omniedge/omniedge)

  <br />
  <a href="https://connect.omniedge.io/download">Download</a>
  <span>&nbsp;&nbsp;•&nbsp;&nbsp;</span>
  <a href="https://connect.omniedge.io/docs">Documentation</a>
  <span>&nbsp;&nbsp;•&nbsp;&nbsp;</span>
  <a href="https://discord.gg/FY6Yd6jcPu">Discord</a>
  <span>&nbsp;&nbsp;•&nbsp;&nbsp;</span>
  <a href="https://twitter.com/omniedgeio">Twitter</a>
  <span>&nbsp;&nbsp;•&nbsp;&nbsp;</span>
  <a href="https://buy.stripe.com/4gwcNy54x75RfCw5kw">Support Project</a>
  <br />
  <hr />
</div>



## 🌟 The Nervous System for the Agentic Future

OmniEdge is a next-generation decentralized networking platform designed to provide a secure, low-latency connectivity fabric for the agentic future. It transforms traditional VPN concepts into a **Secure Connectivity Fabric**, enabling seamless machine-to-machine communication for:

- 🤖 **AI Clusters & Humanoids**: Low-latency P2P tunnels for real-time agent coordination.
- 📡 **Edge Computing**: Securely mesh NVIDIA Jetson, Raspberry Pi, and industrial edge nodes.
- 🏢 **Industrial IoT**: High-density fleet management with sub-millisecond overhead.
- 💻 **Remote Teams**: Zero-config private networks for global collaboration.

### Why OmniEdge?
- **Decentralized P2P Mesh**: No central bottlenecks. What happens in the intranet, stays in the intranet.
- **Zero-Config Onboarding**: Connect nodes in minutes with an encrypted mesh.
- **Technical Observability**: Real-time status pulses and heartbeat tracking for robotics fleets.
- **Fully Open Source**: From backend to frontend and mobile apps.

---

## 📁 Repository Ecosystem

This is the **Meta-Repository** for OmniEdge, synchronizing all core components:

| Component | Repository | Description |
|-----------|------------|-------------|
| **CLI** | [omniedge-cli](https://github.com/omniedgeio/omniedge-cli) | Core engine for Linux, macOS, FreeBSD, and Edge devices. |
| **Android** | [omniedge-android](https://github.com/omniedgeio/omniedge-android) | Mobile & TV client for Android. |
| **iOS/macOS** | [omniedge-iOS](https://github.com/omniedgeio/omniedge-iOS) / [omniedge-macOS](https://github.com/omniedgeio/omniedge-macOS) | Native Apple ecosystem clients. |
| **Windows** | [omniedge-windows](https://github.com/omniedgeio/omniedge-windows) | Native Windows client (Qt). |
| **Synology** | [omniedge-synology](https://github.com/omniedgeio/omniedge-synology) | NAS package for storage-centric edge nodes. |
| **OpenWrt** | [omniedge-openwrt](https://github.com/omniedgeio/omniedge-openwrt) | Mesh networking for router infrastructure. |

---

## 🚀 Quick Start in 5 Minutes

1. **Sign Up**: Create your account on the [Dashboard](https://connect.omniedge.io/register).
2. **Download**: Get the [OmniEdge Client](https://connect.omniedge.io/download) for your platform.
3. **Connect**:
   ```bash
   # On CLI-based systems (Linux, macOS, FreeBSD)
   curl https://connect.omniedge.io/install/omniedge-install.sh | bash
   omniedge login -u your@email.com
   sudo omniedge join
   ```

### Docker Usage
```bash
sudo docker run -d \
  -e OMNIEDGE_SECURITYKEY=YOUR_KEY \
  -e OMNIEDGE_VIRTUALNETWORK_ID="YOUR_NETWORK_ID" \
  --network host \
  --privileged \
  omniedge/omniedge:latest
```

---

## 📊 Monthly Operations Infrastructure
*Transparency on the cost of running the OmniEdge SaaS*

- **Backend**: $12.00
- **Frontend**: $25.00
- **Supernode**: $6.00
- **PostgresSQL**: $15.15
- **CI/CD (GitHub)**: $0.00
- **Total**: **$57.15**

---

## 📝 Contributors & Advisors

Special thanks to our [Advisors](https://omniedge.io/docs/article/about) and the [Global Team](https://github.com/orgs/omniedgeio/people) across US, AU, CN, DE, MY, and CA.

Built with ❤️ by [OmniEdge](https://connect.omniedge.io)

© 2026 OmniEdge Inc. All rights reserved.
*Built by a global remote team.*
