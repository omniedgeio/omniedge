FROM ubuntu:18.04
RUN apt-get update && apt-get install -y curl wget unzip bash git
RUN curl https://omniedge.io/install/omniedge-install.sh | bash
RUN omniedge version
RUN cp /proc/sys/kernel/random/uuid /etc/machine-id
RUN cp /usr/local/bin/omniedge /usr/sbin/
COPY entrypoint /usr/sbin/entrypoint
RUN chmod +x /usr/sbin/entrypoint
ENTRYPOINT ["/usr/sbin/entrypoint"]
