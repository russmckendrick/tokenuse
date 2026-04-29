---
version: "alpha"
name: "tokenuse TUI"
description: "A dense dark terminal design system for a token usage analytics dashboard."
colors:
  primary: "#FF8F40"
  secondary: "#62A6FF"
  tertiary: "#4CF2A0"
  surface: "#202438"
  neutral: "#25293D"
  on-surface: "#CBD4F2"
  muted: "#A1A7C3"
  warning: "#FFD60A"
  error: "#FF5F6D"
  cyan: "#4DF3E8"
  magenta: "#F05AF2"
  bar-empty: "#292D42"
typography:
  display:
    fontFamily: "JetBrains Mono, SFMono-Regular, Menlo, Consolas, monospace"
    fontSize: "18px"
    fontWeight: 700
    lineHeight: 1.2
  panel-title:
    fontFamily: "JetBrains Mono, SFMono-Regular, Menlo, Consolas, monospace"
    fontSize: "16px"
    fontWeight: 700
    lineHeight: 1.25
  body:
    fontFamily: "JetBrains Mono, SFMono-Regular, Menlo, Consolas, monospace"
    fontSize: "15px"
    fontWeight: 500
    lineHeight: 1.35
  label:
    fontFamily: "JetBrains Mono, SFMono-Regular, Menlo, Consolas, monospace"
    fontSize: "14px"
    fontWeight: 600
    lineHeight: 1.25
rounded:
  none: "0px"
  sm: "2px"
  md: "4px"
spacing:
  xs: "1px"
  sm: "2px"
  md: "4px"
  lg: "8px"
  xl: "12px"
components:
  app-surface:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.on-surface}"
    typography: "{typography.body}"
    rounded: "{rounded.none}"
    padding: "{spacing.sm}"
  dashboard-panel:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.on-surface}"
    typography: "{typography.body}"
    rounded: "{rounded.sm}"
    padding: "{spacing.md}"
  summary-panel:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.primary}"
    typography: "{typography.display}"
    rounded: "{rounded.sm}"
    padding: "{spacing.md}"
  info-panel-title:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.secondary}"
    typography: "{typography.panel-title}"
    rounded: "{rounded.sm}"
    padding: "{spacing.sm}"
  success-panel-title:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.tertiary}"
    typography: "{typography.panel-title}"
    rounded: "{rounded.sm}"
    padding: "{spacing.sm}"
  warning-panel-title:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.warning}"
    typography: "{typography.panel-title}"
    rounded: "{rounded.sm}"
    padding: "{spacing.sm}"
  danger-panel-title:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.error}"
    typography: "{typography.panel-title}"
    rounded: "{rounded.sm}"
    padding: "{spacing.sm}"
  cyan-panel-title:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.cyan}"
    typography: "{typography.panel-title}"
    rounded: "{rounded.sm}"
    padding: "{spacing.sm}"
  magenta-panel-title:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.magenta}"
    typography: "{typography.panel-title}"
    rounded: "{rounded.sm}"
    padding: "{spacing.sm}"
  muted-label:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.muted}"
    typography: "{typography.label}"
    rounded: "{rounded.none}"
    padding: "{spacing.xs}"
  heatbar-empty:
    backgroundColor: "{colors.bar-empty}"
    textColor: "{colors.on-surface}"
    typography: "{typography.label}"
    rounded: "{rounded.none}"
    padding: "{spacing.xs}"
---

## Overview

tokenuse should feel like a compact operator console for people who care about token spend, model behavior, and workflow efficiency. The interface is dark, dense, and calm, with bright terminal accents used as structural signposts rather than decoration. The desired response is quick orientation: the user should be able to scan costs, calls, hot spots, and command hints without leaving the keyboard.

## Colors

- **Primary (#FF8F40):** active period/provider states, summary borders, and command keys.
- **Secondary (#62A6FF):** informational panels such as Daily Activity.
- **Tertiary (#4CF2A0):** successful or efficient usage signals.
- **Warning (#FFD60A):** money, token savings, and metric values that need attention.
- **Error (#FF5F6D):** risk, high severity optimization items, and Top Sessions.
- **Cyan (#4DF3E8) and Magenta (#F05AF2):** secondary category accents for tools, models, and MCP-like surfaces.
- **Surface and Neutral:** layered dark blue-gray backgrounds; hierarchy comes from borders and color, not shadows.

## Typography

Use a monospace stack everywhere. Labels and values should align cleanly in columns, with bold reserved for panel titles, active navigation, and important numeric values. Avoid display-scale type inside the dashboard; the interface should stay compact enough for repeated terminal use.

## Layout

The layout is a high-density grid with thin gaps, fixed-height summary/nav/footer bands, and proportional two-column content rows. Panels should preserve predictable column alignment at common terminal widths, degrading by truncating long labels before hiding key metrics.

## Elevation & Depth

Use flat depth. Borders, foreground color, and background tone provide hierarchy; shadows and gradients do not belong in the terminal implementation. Heat bars may use stepped color ramps to imply magnitude.

## Shapes

Terminal panels use square or nearly square corners. Any rounded interpretation should stay at 2-4px when this system is translated to a graphical surface.

## Components

Dashboard panels use one-pixel borders, a colored title, and dense table content. The summary panel uses the primary border and brighter numeric emphasis. Footer commands are inline key/value pairs with orange keys and muted labels.

## Do's and Don'ts

- Do keep the first screen as the actual dashboard, not a splash or marketing page.
- Do keep metric values right-aligned and labels left-aligned for fast scanning.
- Do use color to group panels and severity, but keep the dark surface dominant.
- Do prefer native TUI widgets and layout primitives over custom terminal drawing.
- Don't add decorative backgrounds, oversized type, or large empty hero areas.
- Don't use rounded card-like styling that makes the terminal feel like a web mockup.
- Don't hide keyboard commands behind help text; keep the footer visible.
