# https://hub.docker.com/_/debian
FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y libpq5