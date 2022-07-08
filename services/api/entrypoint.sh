#!/bin/sh

litestream restore -o $LITESTREAM_DB_PATH $LITESTREAM_REPLICA_URL
sudo systemctl enable litestream
sudo systemctl start litestream
./api
