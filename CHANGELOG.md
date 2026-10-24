# Changelog

## 0.4.2
- New: Retake heatmap snapshots
- Improved: Replays show fonts, images and video
- Improved: Smoother scrolling in replays
- Improved: Heatmaps open on the home page
- Fix: Replays work on http pages
- Fix: Text inside embeds shows in recordings
- Fix: Full-screen sections in heatmaps have the right height

## 0.4.1
- New: Clicked elements in heatmaps
- New: Scroll map in heatmaps
- New: Pick a vital to see its trend and slowest pages
- Improved: Typed text shows in unmasked parts, passwords stay hidden
- Improved: Exports run in the background with progress and can email you when ready
- Improved: Flags and logos without frames

## 0.4.0
- New: Notes on the visitor chart
- New: Traffic spikes explained
- New: Web vitals
- New: Paths between pages
- New: Opt-in heatmaps and session replays
- New: MCP server for AI assistants
- New: One-line install script
- New: Notice when a site still loads the Plausible script
- Improved: Instant page and tab switches
- Improved: Loading animations and a progress bar
- Improved: Plainer wording everywhere
- Improved: Clearer goal, filter and settings controls
- Security: Limits on recording uploads
- Security: Page text hidden in recordings by default
- Security: Chart notes visible to members only
- Fix: Resetting stats clears heatmap snapshots
- Fix: MCP lists every site for server keys

## 0.3.0
- New: Uptime monitors
- New: Incidents on the visitor chart
- New: Alerts by email, Slack, Discord, Telegram, ntfy and webhook
- New: Status pages and badges
- New: Monitor API
- New: Import from Uptime Kuma
- Improved: Password manager support
- Improved: Simpler site cards

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
