# Live screenshot assets

These PNGs are cropped captures of the running Glance dashboard in Windows Terminal, connected to a real Herdr session. They are not image-generated mockups or fixture renders. The themes were switched with `t` in the same session; they are not independent benchmark runs.

The three named review agents performed real, read-only work on this repository:

| Agent | Task |
| --- | --- |
| `install-review` | Trace README installation and configuration commands to the implementation; identify unsupported claims and missing prerequisites. |
| `cache-audit` | Review counts-cache scoping, detached refresh, Windows handle inheritance, file replacement, expiry, and regression tests. |
| `dashboard-safety` | Review stable selection, explicit focus revalidation, terminal-control stripping, native states, and narrow layouts. |

The review caught the Windows pre-delete/rename race and documentation mismatches. The corresponding fixes were tested before publication. Review sessions remained open after reporting their results; Herdr can report an awaiting-input session as idle.

`attention-needed.png` also includes two real agents stopped at native startup confirmation dialogs. This demonstrates an actual blocked state, not a fabricated task question. Their personal paths and account data are outside the captured excerpt.

| Asset | Intended use |
| --- | --- |
| `catppuccin-mocha.png` | Main README / webpage product screenshot |
| `dracula.png`, `tokyo-night.png`, `nord.png`, `gruvbox.png` | Theme gallery |
| `attention-needed.png` | Explain the blocked-first order and detection excerpt |
| `manifest.json` | Capture times, dimensions, and nearby native state observations |

All images are native desktop captures at 1638 × 910. They exclude the desktop, Herdr sidebar, private configuration, account usage, and unrelated conversation text. Task-only status lines replaced account/usage footers in the review sessions. No displayed status, timer, graph, or agent response was edited.

To reproduce: launch bounded read-only agents in a dedicated Herdr workspace, run `glance` in that workspace, wait for real activity, and capture only the dashboard pane. Use `t` to cycle themes. Check the actual captured image after each change; the desktop can display a different workspace from the one returned by a CLI read. These are manual live captures; `node docs/capture.cjs` separately regenerates the synthetic fixture images in the parent directory.
