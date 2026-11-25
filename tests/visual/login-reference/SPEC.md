# Desktop login: pixel reference (measured from the real web page)

These numbers come from rendering the **production web login** (`src/pages/Login.tsx` and
`src/styles/login-auth.css`, same CSS bundle as `stock.afmarbre.com`) in headless Chromium
131 at device-scale 1, with DM Sans loaded. Nothing here is estimated. Every box comes from
`getBoundingClientRect()` and every color from `getComputedStyle()`. The raw dump is in
`measurements.json`.

**Native mode = web without Turnstile.** The native desktop flow has no Turnstile widget
(`docs/desktop-native-login-api.md`). All screenshots and numbers below are taken in that mode,
so the 65 px Turnstile slot is gone and "Continuer" sits 30 px under "Se souvenir 30 jours".

Coordinates are CSS px = egui points at `pixels_per_point = 1`. Origin is the window's top-left.
Main reference window: **1280×800** (the desktop's default inner size).

## Reference images (pixel-compare against these)

| State | Light theme | Dark theme |
|---|---|---|
| Idle (username focused, as on page load) | `default-light-1280x800.png` | `default-dark-1280x800.png` |
| Typing in username | `focus-light-1280x800.png` | `focus-dark-1280x800.png` |
| Remember checked, submit hovered | `checked-light-1280x800.png` | `checked-dark-1280x800.png` |
| Wrong password | `error-light-1280x800.png` | `error-dark-1280x800.png` |
| OTP step | `otp-light-1280x800.png` | `otp-dark-1280x800.png` |
| QR scanned, waiting for phone | `claimed-light-1280x800.png` | `claimed-dark-1280x800.png` |
| Idle at 1440×900 | `default-light-1440x900.png` | `default-dark-1440x900.png` |

**The login is a dark page in both app themes.** "Light" and "dark" only change the few
tokens marked L/D below. Never render a white login card.

## Assets (embed in the binary, draw at native/2 for crisp HiDPI)

- Lockup: `public/atlas-logo-dark.png` (white "Atlas" lockup, 781×253). Draw at **197.6×64**.
- QR center icon: `public/atlas-icon-light.png` (253×253).
- Font: **DM Sans** (variable, opsz 9–40, wght 400/500/600/700). No system-font fallback in
  screenshots.

## Page

| Token | Value |
|---|---|
| Background | L: `linear-gradient(160deg, #121214 0%, #0c0c0e 45%, #080809 100%)` plus radial glows `rgba(250,248,244,.07)` at 18%/12% and `rgba(148,163,184,.06)` at 88%/88%. D: solid `#111113` plus radial `rgba(250,250,250,.05)` at 12%/18% and `rgba(250,250,250,.03)` at 88%/82%. A 3-stop vertical mesh gradient is acceptable in egui. |
| Layout | Column, centered horizontally. Padding 56 top / 16 sides / 32 bottom. Gap 16. The card+footer group is **vertically centered** when it fits, otherwise starts at y=56 and scrolls. |

## Card (two columns)

| Element | 1280×800 box (x, y, w, h) | Style |
|---|---|---|
| Card | 160, 79.8, **960**, 556.2 | radius **24**; border 1px L `#2e2e33` / D `rgba(244,244,245,.10)`; shadow L `0 30 80 rgba(24,24,27,.14)` + `0 2 6 rgba(24,24,27,.06)`, D `0 30 90 rgba(0,0,0,.65)` + `0 2 6 rgba(0,0,0,.5)` + inset top 1px `rgba(244,244,245,.06)` |
| Left (form) column | 161, 80.8, **678**, 554.2 | bg `#141414`; padding **48 / 48 / 40 / 48**; children centered |
| Right (QR) column | 839, 80.8, **280**, 554.2 | bg L `#141414` / D `#161618`; **left border 1px `#27272a`**; padding 36 / 28; content vertically **centered** |

At 1440×900 the card is at x=240, y=129.8 (same size), so everything shifts by (+80, +50).

### Left column, top to bottom (x relative to window @1280×800)

| Element | Box | Style |
|---|---|---|
| Lockup image | 401.2, 128.8, 197.6, 64 | centered; 14 below → title |
| Title "Bon retour" | text block 209, 206.8, 582×36 | **24px / 700**, letter-spacing **-0.48px**, line-height 36, `#fafafa`, centered |
| Subtitle "Connectez-vous pour gérer stock, caisse et facturation" | 209, 248.8, 582×20.9 | **14.4px / 400**, line-height 20.88, `#a1a1aa`, centered. 6 px under the title. |
| Form column | x=300, **width 400**, starts y=289.7 (20 under subtitle) | vertical gap **14** |
| Label "IDENTIFIANT" | 300, 289.7, h17.3 | **11.52px / 700**, UPPERCASE, letter-spacing **0.4608px**, `#a1a1aa` |
| Username input | 300, **319.3**, 400×**44** | radius **10**; padding 12/20; text **14px** L `#fafafa` / D `#e4e4e7`; bg L `#0a0a0a` / D `#18181b`; border 1px `#3f3f46`; placeholder "Votre identifiant" L `#757575` / D `#777d8d` |
| Username, focused | same box | border 1px `#fafafa`. Ring L: 1px `#fafafa` (looks 2px). D: 3px `rgba(250,250,250,.12)` and bg `#202024`. Caret visible. |
| Label "MOT DE PASSE" | 300, 377.3 | same as label above |
| Password input | 300, **407**, 400×44 | same as username; placeholder "••••••••" |
| Eye toggle | 662, 415, **28×28** (10 px from input's right edge, vertically centered) | radius 4; icon 20×20 stroke 2 `#a1a1aa` (lucide eye / eye-off); no background |
| Remember row | 300, **469**, h20 (margin-top 4) | gap 8 |
| Checkbox | 300, 469, **20×20** | radius **6**. Unchecked: L bg `#ffffff`, border `#d4d4d8`; D bg `#1c1c1f`, border `#3f3f46`. Checked (both themes): bg + border `#0a0a0a`, white check 12px stroke 3 (polyline 20 6, 9 17, 4 12). |
| "SE SOUVENIR 30 JOURS" | 328, 470.3 | same style as the field labels |
| Submit "Continuer →" | 300, **519**, 400×**48** | pill radius 999; bg `#fafafa`; text `#0a0a0a` **14px / 700**, letter-spacing 0.14px; text + 20px arrow (`M5 12h14`, `m12 5 7 7-7 7`) centered with gap 8 (text x=451, arrow x=529). Hover: L bg `#e4e4e7`; D bg `#c7cbd6` with text `#18181b`. Loading: "Un instant…" with no arrow. Trusted device: "Entrer dans Atlas". |
| Hint | 20 under the button, 12.8px, L `#71717a` / D `#a1a1aa`, centered | **empty in the idle password step** (renders nothing) |

### Error state (`error-*.png`)

The alert sits **20 below the subtitle**, and the form starts **16 below the alert**. Everything
under it moves down 57.5 and the card grows to 613.7 tall (y=56).

| Element | Box | Style |
|---|---|---|
| Alert | 300, 265.9, 400×**41.5** | radius 10; padding 10/12; **13px / 400**; L text `#fca5a5`, bg `rgba(239,68,68,.12)`, border 1px `rgba(239,68,68,.35)`; D text `#f87171`, bg `rgba(220,38,38,.12)`, border `rgba(220,38,38,.35)`. Copy: the server's `REASON_MESSAGES[code]`, e.g. "Identifiant ou mot de passe incorrect." |

### OTP step (`otp-*.png`)

The title stays **"Bon retour"** and the subtitle is unchanged.

| Element | Box (@1280) | Style |
|---|---|---|
| Info box | 300, 289.5, 400×41.5 | radius 10; padding 10/12; 13px. **L: text `#3f3f46` on `#f4f4f5`, border `#e4e4e7` (a light box)**. D: text `#e4e4e7` on `#27272a`, border `#3f3f46`. Copy: server `message`, e.g. "Un code de vérification a été envoyé à a•••@afmarbre.com." |
| Label "CODE DE VÉRIFICATION (OTP)" | 300, 347 | field-label style |
| OTP input | 300, 376.6, 400×44 | input style, **focused** on entry; placeholder "Code reçu par e-mail"; digits only, max 8 |
| Submit "Entrer dans Atlas" | 300, 440.6, 400×48 | submit style, **no arrow** |
| "← Retour" | 300, 512.6, 400×44 | radius 12; padding 10/18; **13px / 600**. L: transparent, text `#52525b`. D: bg `#3f3f46`, text `#f4f4f5` |
| Hint "Code valable 10 min." | centered, y=576.6 | 12.8px, L `#71717a` / D `#a1a1aa` |

### Right column (QR), idle

All elements are centered on x=979 (column center). Content width is 223.

| Element | Box (@1280) | Style |
|---|---|---|
| QR frame | 891.5, 149.1, **176×176** | bg `#fafafa`; radius **12**; padding 12 |
| QR code | 152×152 inside the frame | ECC **H**, modules `#0a0a0a` on `#ffffff`; center icon `atlas-icon-light.png` at **≈34×34 with an excavated (white) area**; no quiet zone beyond the frame padding |
| "Connexion avec un QR" | centered, y=349.1, h25.2 | **16.8px / 700** `#fafafa`; **24 below the frame** |
| Hint | 874.5, 390.3, **max-width 210**, 2 lines | **12.8px / 400**, line-height 18.56, `#a1a1aa`: "Scannez avec l’**app Atlas** pour vous connecter instantanément." "app Atlas" is 700, L `#a1a1aa` / D `#e4e7ef`. 16 below the title. |
| "Ou se connecter avec une passkey" | 868, 445.4, **223×62.8** (wraps to 2 lines) | radius **12**; padding 10/14; **13.6px / 600**, letter-spacing .136; L bg `rgba(255,255,255,.06)`, border 1px `rgba(63,63,70,.9)`, text `#e4e4e7`; D bg `#3f3f46`, border `#52525b`, text `#f4f4f5`. Hover: bg `rgba(255,255,255,.10)`, border `rgba(161,161,170,.7)`, text `#fafafa`. 18 below the hint. |
| "Télécharger l'app Android" | 868, 524.2, 223×42.4 | same as the light-theme passkey style in **both** themes; 16 below the passkey button; opens `https://stock.afmarbre.com/android` in the system browser |

### Right column, QR scanned (`claimed-*.png`)

| Element | Box (@1280) | Style |
|---|---|---|
| Avatar | 931.5, 220.3, **96×96** circle | border 3px `#27272a`; bg `linear-gradient(145deg,#3f3f46,#18181b)`; initial **32px / 700** `#fafafa` (or the user's photo, cropped to a circle) |
| "Vérifiez votre téléphone !" | centered, y=336.3 | QR title style |
| "Acceptez sur le téléphone pour **Amine**" | y=377.5 | QR hint style; the name is bold |
| "Ce n’est pas moi — recommencer" | 868, 432.6, 223×62.8 | passkey button style (theme-dependent). No Android button in this state. |

## Footer (outside the card)

| Element | Box (@1280) | Style |
|---|---|---|
| Footer | centered, **467.7 wide**, top y=680 (44 below the card) | border-top 1px `rgba(244,244,245,.10)`; padding-top 16 |
| Links row | y=697, h18 | **12px / 500**, links L `#71717a` / D `#a1a1aa`, separators "·" L `#52525b` / D `#71717a`, gap 10. Order: Confidentialité · Conditions · Licences · À propos · GitHub · Nouveautés. Each opens the web page in the system browser. Spacing is uniform: word, 10, "·", 10, word (22.4 px between words). Measured x positions: 406.2, 512.3, 596.6, 669.7, 743.7, 806.3. |
| Meta row | y=727, h17.3 | **11.5px**, opacity .82, L `#52525b` / D `#b4b9c6`, gap 8: "© 2026 Atlas Commerce · v{desktop version} · {build date}" |

## Narrow windows (desktop-only rule; the web switches to its phone layout below 900px and hides the QR, which the desktop must not do)

- Window width ≥ 992: everything exactly as above, centered.
- 720 ≤ width < 992: card width = window − 32. The QR column stays 280. The form column shrinks,
  and the form width = min(400, form column − 96). All vertical metrics stay unchanged.
- Height too small: the whole page scrolls vertically (the card is never clipped or scaled).
