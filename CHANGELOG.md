# Changelog

## 0.2.2
- New: Plain style in Account, Display: smooth charts, line icons and no dithering, in dark and light
- New: Demo visitors can open Account, Display to change the theme and formats

## 0.2.1
- Fix: The dashboard no longer loads blank behind Cloudflare Rocket Loader

## 0.2.0
- New: Delete and manage segments from the dashboard filter menu
- New: Hide All sites in Account, Display
- New: Opt out of tracking with `localStorage.impactite_ignore`
- New: Sign in by email link with two factor on
- New: Arrow keys move through menus, toggles and panels
- Improved: Smoother pages with no flashes, faster loads and prefetch on hover
- Improved: Better layout on phones and wider pages on large screens
- Improved: More distinct avatars
- Security: Rate limits on sign-in, cross-site write checks, security headers and stricter size limits
- Security: Webhooks and buckets can't reach private addresses
- Fix: Crimea shows as part of Ukraine on the map
- Fix: Deleting a site removes all of its data

## 0.1.4
- New: Servers can report the visitor with an API key and `X-Forwarded-For`
- New: Admins see a notice when an update is available
- Improved: Clearer update notes

## 0.1.3
- Fix: Switching a panel tab no longer reloads the other panels

## 0.1.2
- New: Add all new events as goals at once
- Improved: Chart and table controls sit on the left and share a line with the table headers
- Fix: The new events list scrolls instead of running off the screen
- Fix: Tables keep their padding next to the scrollbar
- Fix: Update checks run every hour and show their status in the admin page

## 0.1.1
- Improved: Simpler dashboard reports and layout
- Improved: Organized settings and faster page navigation
- Fix: Trackers work on isolated pages

## 0.1.0
- New: One binary with the dashboard, the tracker and its own storage engine
- New: Plausible and GoatCounter trackers, APIs and imports
- New: Umami and Google Analytics 4 imports
- New: Goals, funnels, segments, realtime and comparisons
- New: Public, password and embedded dashboard shares
- New: Teams, passkeys, two factor and SSO
- New: Email reports, traffic alerts and webhooks
- New: Built-in TLS and `impactite update`
