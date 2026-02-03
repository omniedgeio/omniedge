#
# OmniEdge OpenWrt Package Makefile
#
# This Makefile is for building OmniEdge within the OpenWrt build system.
# For standalone builds, use the GitHub Actions workflow or Docker builder.
#
# Usage:
#   1. Clone this into openwrt/package/network/services/omniedge/
#   2. Run: make menuconfig (select Network -> VPN -> omniedge)
#   3. Run: make package/omniedge/compile V=s
#

include $(TOPDIR)/rules.mk

PKG_NAME:=omniedge
PKG_VERSION:=2.3.0
PKG_RELEASE:=1

PKG_SOURCE_PROTO:=git
PKG_SOURCE_URL:=https://github.com/omniedgeio/omniedge.git
PKG_SOURCE_VERSION:=v$(PKG_VERSION)
PKG_SOURCE_SUBDIR:=$(PKG_NAME)-$(PKG_VERSION)
PKG_SOURCE:=$(PKG_NAME)-$(PKG_VERSION).tar.gz
PKG_MIRROR_HASH:=skip

PKG_LICENSE:=Apache-2.0 MIT
PKG_LICENSE_FILES:=LICENSE-APACHE LICENSE-MIT
PKG_MAINTAINER:=OmniEdge Team <support@omniedge.io>

PKG_BUILD_DEPENDS:=rust/host
PKG_BUILD_PARALLEL:=1

include $(INCLUDE_DIR)/package.mk
include $(TOPDIR)/feeds/packages/lang/rust/rust-package.mk

define Package/omniedge
  SECTION:=net
  CATEGORY:=Network
  SUBMENU:=VPN
  TITLE:=OmniEdge - Peer-to-peer VPN
  URL:=https://omniedge.io
  DEPENDS:=$(RUST_ARCH_DEPENDS) +kmod-tun
endef

define Package/omniedge/description
  OmniEdge is a peer-to-peer mesh VPN solution for edge computing.
  It provides secure, low-latency connectivity between devices across
  different networks without requiring a central server for data relay.
  
  Features:
  - Peer-to-peer connectivity with NAT traversal
  - WireGuard-based encryption
  - IPv4 and IPv6 dual-stack support
  - Minimal resource usage suitable for embedded devices
endef

define Package/omniedge/conffiles
/etc/config/omniedge
endef

# Rust build configuration
CARGO_PKG_CONFIG_PATH:=$(PKG_BUILD_DIR)/target/release

define Build/Compile
	$(call Build/Compile/Cargo,omniedge-cli)
endef

define Package/omniedge/install
	$(INSTALL_DIR) $(1)/usr/bin
	$(INSTALL_BIN) $(PKG_BUILD_DIR)/target/$(RUSTC_TARGET_ARCH)/release/omniedge $(1)/usr/bin/
	
	$(INSTALL_DIR) $(1)/etc/init.d
	$(INSTALL_BIN) ./files/omniedge.init $(1)/etc/init.d/omniedge
	
	$(INSTALL_DIR) $(1)/etc/config
	$(INSTALL_CONF) ./files/omniedge.config $(1)/etc/config/omniedge
endef

$(eval $(call BuildPackage,omniedge))
