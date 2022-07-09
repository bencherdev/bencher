#!/bin/sh
set -e

# Restore the database if it does not already exist.
if [ -f /bencher.db ]; then
	echo "Database already exists, skipping restore."
else
	echo "No database found, restoring from replica."
	litestream restore -v -o "$LITESTREAM_DB_PATH" "$LITESTREAM_REPLICA_URL"
fi

# Run litestream.
exec litestream replicate
