# Build Stage
# builder.Dockerfile
FROM bencher-api-builder as builder

# Bundle Stage
# https://github.com/GoogleContainerTools/distroless/blob/main/cc/README.md
FROM gcr.io/distroless/cc-debian12
COPY --from=builder /etc/fonts /etc/fonts
COPY --from=builder /usr/include/fontconfig /usr/include/fontconfig
COPY --from=builder /usr/lib/bencher /usr/lib
COPY --from=builder /usr/share/fonts /usr/share/fonts

COPY --from=builder /usr/bin/litestream/litestream /bin/litestream

COPY --from=builder /usr/bin/bencher/api /api
COPY --from=builder /usr/bin/bencher/data /data

ENV PORT 61016

CMD ["/api"]
