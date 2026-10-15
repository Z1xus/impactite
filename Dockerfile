FROM alpine:3 AS certs
RUN apk add --no-cache ca-certificates

FROM scratch
ARG TARGETARCH
COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY dist/impactite-${TARGETARCH} /impactite
ENV IMPACTITE_DATA=/data IMPACTITE_LISTEN=0.0.0.0:8000
VOLUME /data
EXPOSE 80 8000
ENTRYPOINT ["/impactite"]
CMD ["serve"]
