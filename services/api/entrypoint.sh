#!/bin/sh

litestream restore -o $LITESTREAM_DB_PATH $LITESTREAM_REPLICA_URL
nohup litestream replicate &
./api
