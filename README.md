# TusmoResolver

A Rust bot that plays the **Infinite** (endless) mode of [Tusmo](https://www.tusmo.xyz) on its own. It opens a real Chromium window, reads the grid from the page, picks the best word with a constraint solver, types it, and moves on to the next game, forever.

> **Educational project.** Automating an online game may go against the site's terms of use. Do not use it in ranked or competitive modes, and use it at your own risk.
## Known Issues

### Language switching

Switching between French and English can be unreliable.

Tusmo stores the selected language in a browser cookie. As a result, changing the language through the bot's configuration does not necessarily change the language actually loaded by the website.

The browser may keep using the language stored in its existing cookie, even when a different language is requested.

**Workaround:** Manually change the language on Tusmo before starting the bot, or clear the relevant browser cookie to reset the language preference.

This issue is related to Tusmo's language persistence mechanism rather than the solver itself.

## Disclaimer

This project is not affiliated with Tusmo. It is a personal learning project about browser automation and constraint solving in Rust.