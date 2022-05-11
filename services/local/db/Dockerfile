# https://hub.docker.com/_/postgres
FROM postgres:14.2-bullseye

# run create.sql on init
ADD initdb.sql /docker-entrypoint-initdb.d
