FROM ubuntu:20.04

ARG TARGETARCH
ARG BUILD_ENV
ARG VERSION=0.2.3

# Install all needed packages
RUN add-apt-repository ppa:longsleep/golang-backports -y && \
    apt-get update && \
    apt-get install -y wget make git autoconf automake build-essential golang-go

WORKDIR /app

COPY . .

ENV GOARCH=${TARGETARCH}
ENV GOOS=linux
RUN go generate ./...
RUN BUILD_ENV=$BUILD_ENV make build
