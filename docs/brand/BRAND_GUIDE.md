# Trot brand guide

## Brand idea

**Trot is the quiet, open-source engine under the desk.** It connects to a treadmill, keeps a local record, and exposes plain data without pretending that a 3 km/h walk while answering email is a heroic transformation.

The identity combines two references:

1. **A treadmill console / phosphor terminal** — dark surfaces, precise monospace labels, bright signal green.
2. **Clean pixel art** — a deliberately simple runner and treadmill, used as a recognizable product mark rather than as decoration everywhere.

The result should feel like useful developer infrastructure with a small, dry sense of humor.

## Brand essence

| Dimension | Direction |
|---|---|
| Promise | Bluetooth in. Local API out. Nothing leaves your machine. |
| Personality | Honest, modest, nerdy, useful, quietly playful. |
| Audience | Developers, self-hosters, quantified-self users, privacy-minded walkers, integrators. |
| Positioning | The open-source treadmill engine and local data layer. |
| Primary tagline | **TROT's Really Only Treadmilling.** |
| Product descriptor | **The open-source engine under the desk.** |
| Proof line | **No account. No cloud. No telemetry.** |

### Personality ratio

- 70% utilitarian engineering
- 20% retro terminal / treadmill console
- 10% dry humor

## Voice and messaging

Write in short, plain sentences. Be technically exact. Use understated confidence instead of hype.

### Do

- “Install, pair, walk.”
- “Bluetooth in. Local API out.”
- “Your data, on your disk.”
- “One binary between your treadmill and everything else.”
- “It really only treadmills.”
- Use lowercase commands exactly as users type them: `trot daemon`, `trot scan`, `trot today`.

### Avoid

- “Transform your fitness journey.”
- “Crush your goals.”
- “Revolutionary,” “magical,” “game-changing,” or “AI-powered.”
- Body-transformation imagery, competitive gym language, or claims that Trot coaches the user.
- Implying cloud services, social features, or a graphical interface are part of the engine.

### Messaging hierarchy

1. **What it is:** an open-source treadmill engine, CLI and background daemon.
2. **What it does:** reads treadmill data over Bluetooth, records locally, exposes HTTP + WebSocket.
3. **Why it matters:** local-first, interoperable, hackable, no account or telemetry.
4. **What it is not:** a training platform, a social network, or a cloud service.

## Logo system

The approved direction is the neon-green retro pixel runner on a treadmill. The kit includes:

- `assets/source/approved-retro-pixel-icon.png` — the exact approved concept.
- `assets/source/trot-app-icon-clean-source.svg` — a production-ready vector reconstruction with a full square background.
- `assets/logos/trot-mark-*.svg` — the runner + treadmill mark without the wordmark.
- `assets/logos/trot-wordmark-*.svg` — a custom 5×7 pixel wordmark with no font dependency.
- `assets/logos/trot-lockup-*.svg` — ready-made horizontal and stacked combinations.

### Clear space

Use at least one **head width** of empty space around the standalone mark. For lockups, use at least the height of one wordmark pixel module around every edge.

### Minimum size

- Full app icon with wordmark: 64 px minimum in product UI; 128 px preferred.
- Standalone runner mark: 40 px minimum.
- Pixel wordmark: 92 px minimum width.
- At favicon sizes, use the simplified pixel `T`, not the detailed runner.

### Logo rules

- Keep the pixel edges square and aligned to a whole-pixel grid.
- Do not smooth, round, outline, stretch, rotate, or add 3D perspective to the mark.
- Glow is optional and should remain subtle. The flat mark is the default for documentation and small sizes.
- Never place the phosphor mark on a bright green or visually busy background.
- Do not set interface paragraphs in a pixel font. Pixel geometry belongs to the mark and occasional display accents only.

## Color

Trot is dark-first. The bright green is a signal, not a wallpaper.

| Token | Hex | Use |
|---|---:|---|
| Trot Ink | `#050908` | Page background, app icon background |
| Console | `#0B1210` | Terminal blocks, code surfaces |
| Panel | `#111A17` | Cards and elevated sections |
| Raised Panel | `#17211D` | Hover and selected surfaces |
| Border | `#24302B` | Dividers and quiet outlines |
| Phosphor | `#9BF43E` | Primary accent, logo, primary CTA |
| Signal | `#87E939` | Active state, success, live connection |
| Deep Green | `#2F751F` | Secondary green and charts |
| Text | `#EAF4EC` | Primary copy on dark surfaces |
| Muted | `#91A099` | Secondary copy and labels |
| Warning | `#FFC857` | Non-destructive warnings only |
| Danger | `#FF6B6B` | Destructive actions and errors |

The primary, signal, text, muted and warning colors all exceed WCAG AA contrast for normal text on the dark brand surfaces. On light surfaces, use `#3C7F20` for green text instead of the bright phosphor.

### Color proportion

- 70–80% ink / console / panels
- 15–25% text and neutral lines
- 3–7% phosphor green
- Less than 2% warning or danger colors

## Typography

### Recommended stack

- **Interface and body:** Inter, then the operating-system sans-serif stack.
- **Commands, labels and data:** IBM Plex Mono or a system monospace stack.
- **Logo:** supplied vector pixel wordmark; do not retype it in a downloaded “pixel font.”

Use tabular numerals for speed, distance, duration and step counts. Headings may be tight and large; body copy should remain calm and highly readable.

### Type scale

| Role | Desktop | Mobile | Notes |
|---|---:|---:|---|
| Hero | 88–112 px | 52–64 px | Sans, tight tracking |
| H2 | 48–64 px | 36–44 px | Sans, concise |
| H3 | 20–24 px | 20–22 px | Sans, medium/bold |
| Body large | 18–21 px | 17–19 px | 1.55 line height |
| Body | 16–18 px | 16–17 px | 1.55–1.65 line height |
| Mono label | 12–14 px | 12–13 px | Uppercase optional, spaced |

## Layout and geometry

- Base spacing unit: **4 px**; common steps are 8, 12, 16, 24, 32, 48 and 72 px.
- Use an 8 px visual grid for component alignment.
- Card radii: 14–24 px. Buttons: 10–12 px. The app icon may use platform masking.
- Borders are usually 1 px and low-contrast. Use glow only on key live or primary elements.
- Page width: 1160–1200 px maximum, with generous outer gutters.

## Imagery and graphics

Prefer:

- Pixel diagrams, terminal panels, API flows and live-data readouts.
- Actual treadmill hardware photos only when compatibility needs explanation.
- Flat charts with green as the active series and neutral supporting lines.
- Black, charcoal or off-white backdrops.

Avoid:

- Stock gym photography, dramatic sweat imagery or neon cyberpunk city scenes.
- Generic running-shoe or heart-rate icons as the primary identity.
- Excessive scanlines, noise, chromatic aberration or arcade-game decoration.

## Motion

Motion should suggest a quiet machine running in the background.

- Cursor blink: 1.0–1.2 seconds.
- Live connection pulse: 1.8–2.4 seconds, low opacity.
- Hero float: optional, 7–10 seconds, less than 10 px travel.
- Page transitions: 140–220 ms.
- Always honor `prefers-reduced-motion`.

## Landing-page system

Recommended order:

1. Header with `$ trot`, concise navigation and “View source.”
2. Hero: “The engine under the desk.” + one-sentence explanation + install/API actions.
3. Proof strip: adapters, platforms, local-only behavior.
4. Three feature cards: talks to the belt, keeps the record, gets out of the way.
5. Architecture diagram.
6. Installation commands.
7. API / integration section.
8. Open-source call to action and license note.
9. Optional relationship module: “Trot is the engine. Nowhere is the app.”

The runnable reference implementation is in `web/`.

## GitHub system

Use the supplied `readme-header.svg` at the top of the README, followed by a one-sentence descriptor, compact badges and an immediately usable install command. Keep the README technical and scannable; the branded header provides personality so the rest can stay plain.

Recommended section order:

1. Header graphic and concise descriptor.
2. Install.
3. What it does.
4. Architecture graphic.
5. Core commands.
6. Local API.
7. Privacy and security.
8. Supported treadmills.
9. Build from source.
10. Contributing, license and trademark note.

Use `github/README_TEMPLATE.md` as a drop-in starting point.

## Accessibility

- Keep all body text at WCAG AA contrast or better.
- Do not encode connection state by color alone; pair green with words or icons.
- Every decorative brand image should have empty alt text; explanatory diagrams need meaningful alt text.
- Maintain visible keyboard focus.
- Do not place long paragraphs in all caps or monospace.
- Avoid animation that resembles rapid flicker.

## Brand guardrails

Trot is the engine, not a lifestyle brand. Every design decision should make the product feel easier to understand, inspect and trust. When in doubt, remove decoration, show the command, and state what happens locally.
