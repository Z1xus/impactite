<p align="center"><img src="brand/mark.svg" width="96" alt=""></p>

# impactite

self-hosted web analytics in one binary. it speaks plausible and goatcounter, so their scripts, apis and exports work as they are

the dashboard, the tracker and the storage engine (crater, in `crates/crater`) are all in one static binary. no postgres, no clickhouse, no redis. the tracker is 795 B with brotli

## install

grab a build from the [releases](https://github.com/z1xus/impactite/releases) and run it

```sh
curl -L https://github.com/z1xus/impactite/releases/latest/download/impactite-x86_64-unknown-linux-musl.tar.xz | tar xJ
./impactite serve --url https://stats.example.com --acme
```

or the docker image, which is just the binary on scratch

```sh
docker run -d -p 443:8000 -p 80:80 -v impactite:/data ghcr.io/z1xus/impactite serve --url https://stats.example.com --acme
```

the first start prints a setup link, so open it and make your account. `--acme` gets a certificate on its own, behind a proxy just drop it (or pass `--cert` and `--key`)

## moving over

point your existing script tag at your own host

```html
<script defer data-domain="example.com" src="https://stats.example.com/js/script.js"></script>
<script data-goatcounter="https://stats.example.com/count" async src="https://stats.example.com/count.js"></script>
```

and history comes along too: upload a plausible csv export or a goatcounter csv in the site settings, or pull straight from a plausible clickhouse with `impactite import plausible`

## config

every flag has an env var, `impactite serve --help` lists them

- `IMPACTITE_DATA` - where everything lives, `data` by default
- `IMPACTITE_URL` - the public address, used for links, passkeys and acme
- `IMPACTITE_MAIL` - `smtp://`, `smtps://`, `resend://KEY` or `cloudflare://TOKEN@ACCOUNT`, mail is only logged without it
- `IMPACTITE_GOOGLE_CLIENT_ID` and `IMPACTITE_GOOGLE_CLIENT_SECRET` - for search console terms

## building

needs rust and bun

```sh
cd web && bun install && bun run build && cd ..
cargo build --release
```

## license

agpl 3.0. the tracker in `tracker` is mit, so it can sit on any site
