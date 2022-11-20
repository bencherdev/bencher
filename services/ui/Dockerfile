# Build Stage
# builder.Dockerfile
FROM bencher-ui-builder as builder

# https://hub.docker.com/_/nginx
FROM nginx:mainline-alpine
COPY --from=builder /usr/src/services/ui/dist /usr/share/nginx/html

RUN rm -rf /etc/nginx/conf.d
COPY conf /etc/nginx

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]