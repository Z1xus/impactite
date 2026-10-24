# <img src="brand/mark.svg" alt="" width="36" height="36" align="absmiddle"> impactite

<a href="https://github.com/Z1xus/impactite/actions/workflows/ci.yml"><picture><source media="(prefers-color-scheme: dark)" srcset="https://www.shieldcn.dev/github/ci/Z1xus/impactite.svg?variant=secondary&amp;size=xs&amp;mode=dark"><img alt="CI status" src="https://www.shieldcn.dev/github/ci/Z1xus/impactite.svg?variant=secondary&amp;size=xs&amp;mode=light"></picture></a>

Self-hosted web analytics in one binary. Works with your existing [Plausible](https://plausible.io) and [GoatCounter](https://www.goatcounter.com) scripts, API calls and exports.

<a href="https://demo.impactite.dev/demo/impactite.dev"><img src="https://github.com/Z1xus/impactite/blob/assets/dashboard.png?raw=true" alt="impactite dashboard"></a>

The dashboard, the tracker and the storage engine ([crater](crates/crater)) are all in one static binary. No Postgres, no ClickHouse, no Redis. You can click around the [live demo](https://demo.impactite.dev/demo/impactite.dev) or read more on [impactite.dev](https://impactite.dev).

## Install

On any Linux server with systemd:

```sh
curl -fsSL https://impactite.dev/install | sh
```

Or use Docker:

```sh
docker run -d -p 443:8000 -p 80:80 -v impactite:/data ghcr.io/z1xus/impactite serve --url https://stats.example.com --acme
```

For rootless Podman, the [install builder](https://impactite.dev/#install) writes a hardened Quadlet unit.

On first start it prints a setup link where you make your account. `--acme` gets the certificate for you. If you already have a reverse proxy, drop that flag and point the proxy at `127.0.0.1:8000`.

Updating is `impactite update` and a restart (or pulling the new image).

## Moving from Plausible or GoatCounter

Change the host in the script you already have:

```html
<script defer data-domain="example.com" src="https://stats.example.com/js/script.js"></script>
<script data-goatcounter="https://stats.example.com/count" async src="https://stats.example.com/count.js"></script>
```

Your old stats can come along too. Upload a Plausible or GoatCounter CSV export in the site settings, or pull everything straight from a Plausible ClickHouse with `impactite import plausible`. Google Analytics 4 connects from the site settings, and Umami goes through `impactite import umami`.

## Benchmarks

Each tool got 1M pageviews, one at a time, on the same machine (Ryzen 5 7600X). You can check the sources and the raw results in [impactite-benchmarks](https://github.com/Z1xus/impactite-benchmarks).

| | impactite | Plausible CE | GoatCounter |
| --- | ---: | ---: | ---: |
| Pageviews per second | 159,799 | 7,753 | 9,738 |
| Top pages query | 0.34 ms | 56.5 ms | 1.34 ms |
| CPU for 1M pageviews | 21.5 s | 1,248.2 s | 137.9 s |
| Idle memory | 18 MiB | 942 MiB | 37 MiB |
| Memory after 1M | 124 MiB | 2,217 MiB | 482 MiB |
| Image size | 19 MiB | 987 MiB | 36 MiB |
| Tracker, gzipped | 930 B | 1,324 B | 3,259 B |

## Why it's faster

- It's one process. Plausible runs on Elixir, Postgres and ClickHouse, so every pageview goes over the network and through a SQL insert.
- The tracker endpoint drops the event on an in-memory queue and answers right away. A single writer thread appends to a write-ahead log in batches.
- Storage is columnar. Strings go into a per-site dictionary, and events store small integer ids. Each column is bit-packed to the width its values need (a column that never changes takes no space at all).
- At the end of a day, crater seals it into a segment plus a rollup with the counts for every value of every dimension. An unfiltered 30-day query reads 30 small rollups and only scans today.
- Results for sealed days are cached.

## Configuration

Every flag has an environment variable, and `impactite serve --help` lists them all. You'll most likely need these:

- `IMPACTITE_URL`: the public address. Links, passkeys and certificates use it.
- `IMPACTITE_DATA`: where everything is stored (`data` by default).
- `IMPACTITE_MAIL`: `smtp://`, `smtps://`, `resend://KEY` or `cloudflare://TOKEN@ACCOUNT`. Without it, mail only goes to the log.
- `IMPACTITE_ALIAS`: a second path for the tracker, for when ad blockers catch `/js/script.js`.
- `IMPACTITE_SSO_ISSUER`, `IMPACTITE_SSO_CLIENT_ID` and `IMPACTITE_SSO_CLIENT_SECRET`: sign-in with any OpenID Connect provider.

## Build

You need [Rust](https://rustup.rs) and [Bun](https://bun.sh).

```sh
cd web && bun install && bun run build && cd ..
cargo build --release
```

## License

AGPL-3.0. The tracker in [tracker](tracker) is MIT.

If the AGPL doesn't work for your company, there's a [commercial license](COMMERCIAL.md). Contributions go under a short [CLA](CLA.md).
