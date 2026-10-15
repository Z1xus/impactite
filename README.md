<p align="center"><img src="brand/mark.svg" width="96" alt=""></p>

# impactite

self-hosted web analytics in one binary. it speaks plausible and goatcounter, so their scripts, apis and exports work as they are

the dashboard, the tracker and the storage engine (crater, in `crates/crater`) are all in one static binary. no postgres, no clickhouse, no redis. the tracker is 795 B with brotli

## install

grab a build from the [releases](https://github.com/z1xus/impactite/releases) and run it

```sh
curl -L https://github.com/z1xus/impactite/releases/latest/download/impactite-x86_64-unknown-linux-musl.tar.gz | tar xz --strip-components=1
sudo setcap cap_net_bind_service=+ep ./impactite
./impactite serve --url https://stats.example.com --acme
```

or the docker image, which is just the binary on scratch

```sh
docker run -d -p 443:8000 -p 80:80 -v impactite:/data ghcr.io/z1xus/impactite serve --url https://stats.example.com --acme
```

the first start prints a setup link, so open it and make your account. `--acme` gets a certificate on its own and listens on 443, `setcap` lets it do that without root. behind a proxy just drop it and it listens on `127.0.0.1:8000` (or pass `--cert` and `--key`). the landing page has a builder that writes these commands for your host

## moving over

point your existing script tag at your own host

```html
<script defer data-domain="example.com" src="https://stats.example.com/js/script.js"></script>
<script data-goatcounter="https://stats.example.com/count" async src="https://stats.example.com/count.js"></script>
```

and history comes along too: upload a plausible csv export or a goatcounter csv in the site settings, pull straight from a plausible clickhouse with `impactite import plausible`, or connect google analytics 4 in the site settings

coming from umami? dump the site's events from its postgres, with your website id, then upload the file in the site settings or run `impactite import umami umami.csv --site example.com`

```sh
psql "$DATABASE_URL" -c "\copy (select e.*, s.browser, s.os, s.device, s.screen, s.language, s.country, s.region, s.city from website_event e join session s using (session_id) where e.website_id = 'WEBSITE_ID' order by e.created_at) to 'umami.csv' csv header"
```

## config

every flag has an env var, `impactite serve --help` lists them

- `IMPACTITE_DATA` - where everything lives, `data` by default
- `IMPACTITE_URL` - the public address, used for links, passkeys and acme
- `IMPACTITE_MAIL` - `smtp://`, `smtps://`, `resend://KEY` or `cloudflare://TOKEN@ACCOUNT`, mail is only logged without it
- `IMPACTITE_GOOGLE_CLIENT_ID` and `IMPACTITE_GOOGLE_CLIENT_SECRET` - for search console terms
- `IMPACTITE_SSO_ISSUER`, `IMPACTITE_SSO_CLIENT_ID`, `IMPACTITE_SSO_CLIENT_SECRET` - sign in through okta, entra, google workspace, authentik, keycloak or anything else with openid connect. set the redirect url to `https://stats.example.com/api/app/sso/callback`. `IMPACTITE_SSO_NAME` names the button. people with an account sign in by email, new ones get in only when sign-up is open
- `IMPACTITE_ALIAS` - a second path for the tracker, see below
- `IMPACTITE_NO_UPDATE_CHECK` - stops the daily look at github releases that tells admins about new versions

to update a binary install run `impactite update` and restart it. with docker, pull the new image

reports go to an email, a slack or discord webhook, or any https url as json. a raw export report puts each day's events into an s3 compatible bucket every morning, set it to `s3://KEY:SECRET@host/bucket/folder` (`s3+http://` for one on your own network)

## spreadsheets

the stats api answers in csv with `format=csv`, and takes the key in the url as `key=` for tools that can't send headers. in google sheets:

```
=IMPORTDATA("https://stats.example.com/api/v1/stats/timeseries?site_id=example.com&period=30d&format=csv&key=KEY")
```

looker studio reads that sheet. make a separate key for it, since a url can end up in logs

## ad blockers

some blocklists know `/js/script.js` and `/api/event`. set `IMPACTITE_ALIAS=insights` (any word you like) and the tracker also lives at `/insights/script.js`, with events going to `/insights/e`. the snippet in site settings switches to it on its own

to be fully invisible, proxy that path through the site itself, so the browser never talks to another domain. for nginx:

```nginx
location /insights/ {
    proxy_pass https://stats.example.com/insights/;
    proxy_set_header X-Forwarded-For $remote_addr;
}
```

then point the snippet at your own domain, and add the site server's ip to `IMPACTITE_TRUSTED_PROXIES` so visitors keep their real ip

## building

needs rust and bun

```sh
cd web && bun install && bun run build && cd ..
cargo build --release
```

## license

agpl 3.0. the tracker in `tracker` is mit, so it can sit on any site

if the agpl doesn't fit your company, there's a [commercial license](COMMERCIAL.md). contributions go under a short [cla](CLA.md) so both can exist
