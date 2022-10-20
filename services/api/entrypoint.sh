#!/bin/sh
set -e

# Restore the database if it does not already exist.
if [ -f /bencher.db ]; then
	echo "Database already exists, skipping restore."
else
	echo "No database found, restoring from replica if it exists."
	litestream restore -v -if-replica-exists -o "/data/$BENCHER_DB" "$BENCHER_REPLICA_URL"
fi

# Run litestream with your app as the subprocess.
exec litestream replicate
