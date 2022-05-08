# https://hub.docker.com/_/nginx
FROM nginx:1.20.2-alpine

RUN rm /etc/nginx/conf.d/default.conf
COPY /dev.conf /etc/nginx/conf.d