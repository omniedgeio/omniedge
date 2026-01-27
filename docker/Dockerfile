FROM ubuntu:20.04

ARG TARGETARCH
ARG BUILD_ENV
ARG VERSION

# Install all needed packages
RUN apt-get update && \
    apt-get install -y wget make git autoconf automake build-essential libtool patch pkg-config software-properties-common

RUN add-apt-repository ppa:longsleep/golang-backports -y && \ 
    apt-get update && \
    apt-get install -y golang-1.23-go && \
    ln -sf /usr/lib/go-1.23/bin/go /usr/local/bin/go

ENV GOWORK=off
WORKDIR /app

COPY . .
RUN rm -f go.work
ENV GOOS=linux
RUN go generate ./...
RUN VERSION=$VERSION BUILD_ENV=$BUILD_ENV make build
