#!/bin/sh
set -e

# Restore the database if it does not already exist.
if [ -f "$LITESTREAM_DB_PATH"  ]; then
	echo "Database already exists, skipping restore."
else
	echo "No database found, restoring from replica if it exists."
	litestream restore -if-replica-exists -o "$LITESTREAM_DB_PATH" "$LITESTREAM_REPLICA_URL"
fi

# Run litestream with your app as the subprocess.
exec litestream replicate -exec "/api" "$LITESTREAM_DB_PATH" "$LITESTREAM_REPLICA_URL"
