# Interactive demo runbook

Use the installed binary in an empty disposable project. This guide starts no agent
and contains no scripted model responses. Paste the prompt yourself.

```sh
mkdir snake-demo
cd snake-demo
git init
s1code
```

On the entry screen, select native Claude and the official Jev endpoint:

```text
/provider claude
/model claude-opus-5
/jev typesafe
/eviction jev
/permissions full-access
```

Provider/model/retention preferences persist. Credentials already saved in macOS
Keychain are reused; they never belong in the task field or recording. Automatic
approval applies to supported actions in this conversation only; it is not an OS
sandbox or permission for arbitrary shell commands.

Paste this as the first task:

```text
Build a polished playable Snake website here using vanilla HTML, CSS and JavaScript.
Use a dark arcade design with green accents, responsive layout, a 20x20 board,
arrow/WASD controls, Space to pause, start/pause/restart buttons and mobile controls.
Prevent reversing direction even with rapid input between ticks. Food must spawn
only on empty cells. Eating adds 10 points; wall/self collision ends the game.
Persist best score when localStorage works and keep playing when it is unavailable.
Create index.html, styles.css, game.js and game.test.js. Keep game logic testable
without a browser. Write meaningful tests using node:test, run node --test and fix
failures. No external dependencies, CDN, installation, commits or pushes. Finish
with the actual verification result.
```

Inspect the result; F2 then 4 opens the real patch diff. F2 returns to conversation.
After completion, use the follow-up field without restarting the program:

```text
Add an accessible difficulty selector with Easy, Normal and Hard. Apply changes on
the next game start, preserve pause/restart behavior, and add tests for the selected
speed and restart state. Run the tests and report the actual result.
```

Esc closes the conversation. In another terminal, preview the local site:

```sh
cd snake-demo
python3 -m http.server 8082 --bind 127.0.0.1
```

Open http://127.0.0.1:8082. Stop the preview with Ctrl-C. A saved session can be reopened
with `s1code sessions` then `s1code resume SESSION_ID`.

A small task may never exceed its context budget; that is expected. Do not claim an
eviction occurred without an actual event. For an offline demonstration of exact
stored-byte recovery use `s1code context-demo --workspace /tmp/s1code-context-demo-new`.
That command uses real artifacts and no model call; label it accordingly.

Keep Jev service measurements and traces private pending clearance. This runbook is
not authorization to publish provider-restricted results, a comparative benchmark,
or a claim of superiority. A recorded successful run establishes that run's outcome;
it does not establish an average success rate or efficiency advantage. For a public
recording without Jev clearance, select `/decision rules` and `/eviction conservative`
and accurately label those policies. Never relabel a Jev run as a rules run.
