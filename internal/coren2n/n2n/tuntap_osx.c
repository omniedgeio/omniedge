/**
 * (C) 2007-18 - ntop.org and contributors
 * Modified for native utun (L3) support on macOS
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 3 of the License, or
 * (at your option) any later version.
 */

#include "n2n.h"

#ifdef __APPLE__

#include <arpa/inet.h>
#include <fcntl.h>
#include <net/if_utun.h>
#include <netinet/in.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/kern_control.h>
#include <sys/socket.h>
#include <sys/sys_domain.h>
#include <sys/types.h>
#include <unistd.h>

#define UTUN_CONTROL_NAME "com.apple.net.utun_control"

/* ********************************** */

int tuntap_open(tuntap_dev *device, char *dev, const char *address_mode,
                char *device_ip, char *device_mask, const char *device_mac,
                int mtu) {
  struct sockaddr_ctl addr;
  struct ctl_info info;
  int fd;

  fd = socket(PF_SYSTEM, SOCK_DGRAM, SYSPROTO_CONTROL);
  if (fd < 0) {
    traceEvent(TRACE_ERROR, "socket(PF_SYSTEM) failed: %s", strerror(errno));
    return -1;
  }

  memset(&info, 0, sizeof(info));
  strncpy(info.ctl_name, UTUN_CONTROL_NAME, sizeof(info.ctl_name));
  if (ioctl(fd, CTLIOCGINFO, &info) < 0) {
    traceEvent(TRACE_ERROR, "ioctl(CTLIOCGINFO) failed: %s", strerror(errno));
    close(fd);
    return -1;
  }

  addr.sc_id = info.ctl_id;
  addr.sc_len = sizeof(addr);
  addr.sc_family = AF_SYSTEM;
  addr.ss_sysaddr = AF_SYS_CONTROL;
  addr.sc_unit = 0; /* Let system assign utun unit */

  /* If dev starts with utunX, try to use it */
  if (dev && strncmp(dev, "utun", 4) == 0) {
    addr.sc_unit = atoi(dev + 4) + 1;
  }

  if (connect(fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
    traceEvent(TRACE_ERROR, "connect(utun) failed: %s", strerror(errno));
    close(fd);
    return -1;
  }

  device->fd = fd;

  /* Get actual interface name */
  char utun_name[20];
  socklen_t utun_name_len = sizeof(utun_name);
  if (getsockopt(fd, SYSPROTO_CONTROL, UTUN_OPT_IFNAME, utun_name,
                 &utun_name_len) == 0) {
    strncpy(device->dev_name, utun_name, sizeof(device->dev_name));
  }

  /* Configure IP, Netmask and MTU via ifconfig */
  char cmd[256];
  snprintf(cmd, sizeof(cmd), "ifconfig %s %s %s netmask %s mtu %d up",
           device->dev_name, device_ip, device_ip, device_mask, mtu);
  system(cmd);

  /* Generate/Store MAC */
  if (device_mac && device_mac[0]) {
    str2mac(device->mac_addr, device_mac);
  } else {
    for (int i = 0; i < 6; i++)
      device->mac_addr[i] = rand() % 256;
    device->mac_addr[0] &= 0xfe; /* Unicast */
    device->mac_addr[0] |= 0x02; /* Local */
  }

  device->ip_addr = inet_addr(device_ip);
  traceEvent(TRACE_NORMAL, "Native %s up and running (%s/%s)", device->dev_name,
             device_ip, device_mask);

  return device->fd;
}

/*
 * L3 to L2 Bridge:
 * utun provides raw IP packets (with a 4-byte protocol header).
 * n2n expects Ethernet frames.
 */
int tuntap_read(struct tuntap_dev *tuntap, unsigned char *buf, int len) {
  unsigned char raw_buf[2048];
  int nread = read(tuntap->fd, raw_buf, sizeof(raw_buf));
  if (nread < 4)
    return nread;

  /* Skip 4-byte utun header (Protocol family) */
  int ip_len = nread - 4;
  if (ip_len + 14 > len)
    return -1;

  /* Construct Ethernet header */
  ether_hdr_t *hdr = (ether_hdr_t *)buf;
  memset(hdr->dhost, 0xFF, 6); /* Default to broadcast, n2n will learn/route */
  memcpy(hdr->shost, tuntap->mac_addr, 6);
  hdr->type = htons(0x0800); /* IPv4 */

  memcpy(buf + 14, raw_buf + 4, ip_len);
  return ip_len + 14;
}

int tuntap_write(struct tuntap_dev *tuntap, unsigned char *buf, int len) {
  if (len < 14)
    return 0;
  ether_hdr_t *hdr = (ether_hdr_t *)buf;

  if (ntohs(hdr->type) == 0x0800) {
    /* IPv4: Strip Ethernet header and prepend 4-byte utun AF_INET header */
    unsigned char out_buf[2048];
    uint32_t af_inet = htonl(AF_INET);

    memcpy(out_buf, &af_inet, 4);
    memcpy(out_buf + 4, buf + 14, len - 14);

    return write(tuntap->fd, out_buf, len - 14 + 4);
  } else if (ntohs(hdr->type) == 0x0806) {
    /* ARP: Dummy handling - utun is L3, it doesn't need ARP. */
    return len;
  }

  return 0;
}

void tuntap_close(struct tuntap_dev *tuntap) {
  if (tuntap->fd >= 0) {
    close(tuntap->fd);
  }
}

void tuntap_get_address(struct tuntap_dev *tuntap) {
  /* Dynamic address updates not implemented for utun yet */
}

#endif /* __APPLE__ */
