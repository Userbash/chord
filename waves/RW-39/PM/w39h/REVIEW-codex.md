**FAIL**

**P0**

- [embeds.js](/home/user/orca/project/rust/gui-spike/ui/features/embeds.js:37): `createEmbed()` copies `embed.url` directly into `href`. A `javascript:` URL can execute when the link is activated. Repro: call `createEmbed({ kind: "link", url: "javascript:alert(document.domain)" })` and click its title. Validate URLs against allowed protocols before assigning `href`.

**P1**

- [index.html](/home/user/orca/project/rust/gui-spike/ui/index.html:110): `data-action="gift"` has no entry in the handler table in [actions.js](/home/user/orca/project/rust/gui-spike/ui/features/actions.js:75), and the button is not marked `aria-disabled`. Repro: click the Gift button; it reports an unknown action and does nothing. Add a working handler or mark it disabled with an explanatory label.

**P2**

- [search-overlay.js](/home/user/orca/project/rust/gui-spike/ui/features/search-overlay.js:388): “По новизне” and “Самое актуальное” both sort by descending index, so selecting either produces the same ordering.

I found no `innerHTML`-style DOM insertion. The reviewed tests contain no `skip` markers, and include assertions for row heights, the 500-row loaded limit, and toolbar disabled states. Both test files are untracked at this HEAD, so I could not compare them with a tracked baseline to verify that no assertions were removed. I did not run tests or launch the GUI.