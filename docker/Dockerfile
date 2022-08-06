FROM ubuntu:20.04

ARG TARGETARCH
ARG VERSION=0.2.3

# Install all needed packages
RUN apt-get update && \
apt-get install -y wget git autoconf automake openssl autoconf build-essential libssl-dev zip wget

WORKDIR /

RUN wget https://go.dev/dl/go1.18.4.linux-amd64.tar.gz
RUN rm -rf /usr/local/go && tar -C /usr/local -xzf go1.18.4.linux-amd64.tar.gz
RUN export PATH=$PATH:/usr/local/go/bin
RUN export CGO_ENABLED=1

WORKDIR /app

COPY . .
ARG BUILD_ENV

ENV GOARCH=${TARGETARCH}
ENV GOOS=linux
RUN go generate ./...
RUN BUILD_ENV=$BUILD_ENV make build
