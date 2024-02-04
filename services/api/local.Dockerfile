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

COPY --from=builder /usr/bin/litestream/litestream /usr/bin/litestream
COPY --from=builder /usr/bin/bencher /usr/bin/bencher

ENV PORT 61016

CMD ["/usr/bin/bencher/api"]
