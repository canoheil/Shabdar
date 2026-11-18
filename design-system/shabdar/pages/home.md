# The neighborhood storefront

Keep the marketplace functional before it is decorative. The first screen has one
search, one shop illustration, an aisle of actual category links, and the beginning
of the live feed. All typography is live Persian text, never baked into the picture.

- The forest-green search band pairs «از دلِ محله، به خانه‌ی شما.» with a screen-printed market stall. At phone widths the smaller WebP sits beside the headline, not above it. In either theme the artwork has the same ink ground. It is decorative, has an empty alt, and does not animate.
- The top location control is a native shortcut to the existing province filter. A change clears the old city and keeps the other URL filters. Desktop keeps sorting visible beside the listings and puts the existing immediate filters behind an expanded/collapsed button. A URL with adjustable filters starts expanded. Phone filters remain a draft/apply dialog with focus return.
- Category colors are decorative. The selected category has a filled surface **and** `aria-current`. The row scrolls independently of the page when it does not fit.
- Use real listing media only. The WebPs in `apps/web/scripts/visual-fixtures/` are generated product photography for the local read-only visual fixture, *not* stock replacements for production images. The live API, moderation and missing-image behavior are unchanged.

Intentional test contract changes (`home.spec.ts`): assert the new headline; open the desktop disclosure before using its controls; find sort in the always-visible results toolbar; check that the region shortcut and the detailed province field agree. No accessibility, performance or data gate is relaxed.
