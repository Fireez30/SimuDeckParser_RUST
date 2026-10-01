# PLEASE READ THIS :

This project has been stopped and will not be developed anymore. 

I developed this so I could handle my decks and stats from an interface I wanted, on Linux. 

Loosing access to new versions of the simulator tool, this tool not useful for connecting encore decks and the simulator, which was my priority

# Normal read me :


This project has been developed for people using the Weiss Schwarz Simulator.

It runs on Linux and Windows (the simulator itself keeps the same folder layout on both).


# Installation 

The tool is written in Rust (it was originally a C++/Qt application).

Build it with a Rust toolchain (install one with [rustup](https://rustup.rs)):

```sh
cargo build --release
./target/release/simu_deck_parser        # Linux
.\target\release\simu_deck_parser.exe    # Windows
```

On Windows, the rustup installer offers to install the Visual Studio C++ build tools; they are needed (used by the HTTPS library of the Encore Decks import). CI builds both a Linux binary and a Windows `.exe`, available as artifacts of each run.

Run the tests with `cargo test`. Ignored tests cover the rest: `cargo test -- --ignored` also queries encoredecks.com and renders every screen to `target/screenshots` (or `$SCREENSHOT_DIR`); with `SIMULATOR_ROOT=/path/to/simulator` they load a real install (read-only) and report cards without artwork and unresolved deck codes.

# Usage

The interface follows Material 3: a navigation rail on the left (Cards, Decks, Settings), light and dark themes (following the system by default; switch from the rail or Settings).

## First start

The tool asks for the folder where the simulator is installed: the one containing `CardContent` and `Decks` (for example `~/Games/SimuWeiss`). Simulator folders found under your home folder (and in Steam libraries) are listed so you can pick one; you can also paste a path (a subfolder such as `CardContent` or `Decks` works too). The field tells you right away whether the folder is a simulator install.

The tool reads:

- the cards from `CardContent/Cards` (every serie folder with a `SingleSetData.txt`),
- the common effects from `<Game>_Data/StreamingAssets/CommonEffects_USER_REFERENCE.txt`,
- the decks from `Decks`, one text file per deck.

It is saved in `~/.config/weiss_simulator_parser/settings.conf`, together with your theme, last screen and last serie.

## Cards

- Pick a serie at the top (the list has a search field); its cards load immediately. When the serie has several sets, a second menu narrows the view to one set.
- Search by name, card code or effect text with the search bar (`Ctrl+F`). The search applies within the active filters.
- Filter with the chips on the left: type, color, level, cost and traits. Nothing selected means no filter; the first chip you pick narrows the list, the next ones add to it. "Clear" resets everything.
- Grid view: hover a card to preview it on the right, click to pin it (click again or press `Esc` to unpin). A slider sets the card size.
- List view: every card with its details and effect text.

## Decks

- Decks of the simulator's `Decks` folder are listed on the left (with a search field when there are many), the AI opponents' decks (`AI_*.txt`) in their own section. Click one to open it.
- The deck page shows its size (with a warning when it is not 50 cards or has more than 8 climaxes), the type split, the level curve and the colors, then the cards grouped by type with their number of copies.
- Codes that matched no card are listed in a banner.
- Hover / click cards to preview / pin them, as in the card view.

## Deck builder

- "New deck" in the Decks screen opens the builder: the card browser (serie and set menus, search, filters) with the deck on the right.
- Click a card to add a copy, right-click to remove one (in the list view, use the Add / Remove buttons). The deck panel also has −/+ buttons per card, and hovering a card shows it in full.
- The deck panel groups cards by level (characters and events together), with climaxes last, and switches between a list (with −/+ buttons) and a grid of pictures (click adds a copy, right-click removes one).
- The builder follows the deck rules: at most 50 cards and 4 copies of cards sharing a name. It shows the card count and warns above 8 climaxes.
- Name the deck and press Save: it is written to the simulator's `Decks` folder (a taken name gets a " (2)" suffix). A deck can be saved before it reaches 50 cards.
- On an open deck, "Edit" changes it in place (renaming the file if you rename the deck) and "Duplicate" starts a new deck from it. AI opponents' decks can only be duplicated. Codes that match no loaded card are kept when saving.
- Leaving the builder, or closing the window, with unsaved changes asks for confirmation.

## Encore decks import

Click "Import" in the Decks screen and paste the deck link, e.g. " https://www.encoredecks.com/deck/PPyvcLuvt ". The dialog checks the link as you type; press Import (or Enter).

The deck is saved as a new file in the simulator's `Decks` folder, in the simulator's format. Existing decks are never overwritten: if the name is taken, the new deck gets a " (2)" suffix.

The import runs in the background; the new deck opens when it is done. If a card is not found, the tool searches for the equivalent card (JP or EN, foil or non foil). If a deck cannot be imported, please open an issue with the link of the deck.

## Encore Decks account decks

The "Encore" screen shows every deck of your linked account (see below) on encoredecks.com, private and unfinished ones included, 24 decks per page: "Previous" and "Next" turn the pages, and the list can be re-read at any time.

Click a deck to preview it: its cards are read from encoredecks.com and shown like a simulator deck, matched to the loaded cards (JP/EN, foil or non foil). Import it with "Import" if you want it in the simulator: it is saved as a new file in the `Decks` folder, exactly like a link import; existing decks are never overwritten. Decks already present in the simulator are marked "in the simulator".

Without a linked account, the screen explains how to link one in Settings. The lists are read in the background, with the same session as uploads. If the session has expired, the account is unlinked: link it again in Settings.

## Settings

Theme, the simulator folder (with "Reload cards" after a simulator update and "Change folder…"), and where the decks folder is.

# Planned features

Soon : 


Longer : 

 - mac support ?


# Questions

- I have updated or changed my simulator path, how can I change it in the software ? 

=> Open Settings and press "Change folder…". You will be able to enter the new path and save it again.

- I have a bug, what can I do ? 

=> You can post an issue in this github repository, explaining the bug, and if possible how you reached the bug, a screenshot and/or a video













## Encore decks upload

Open a deck in the Decks screen and click "Upload" to publish it on encoredecks.com. After confirming, the deck is sent in the background and the dialog shows its link (open it in the browser or copy it).

The deck is created as a new public deck in your Encore Decks account when one is linked (see below). Without an account it is uploaded anonymously, and then it cannot be edited or deleted there afterwards. Card codes are matched to the Encore Decks cards the same way as for imports (JP or EN, foil or non foil); for cards with several arts, the first art (code ending in "a") is used. Only complete decks are published: the deck must have exactly 50 cards and every one of them must exist on Encore Decks. Otherwise nothing is uploaded, and the error lists the codes Encore Decks does not know.

### Encore Decks account

In Settings, "Encore Decks account", sign in with your Encore Decks email and password to upload decks to your account. "Unlink" signs out.

- The password is never stored. It is sent once, over HTTPS, to encoredecks.com, then wiped from memory.
- Only the login session is kept, in the system credential store: the Secret Service (GNOME Keyring, KWallet) on Linux, the Credential Manager on Windows. These store it encrypted and tied to your login. It is never written to the settings file (which only holds the account name). Without a credential store, the account cannot be linked; there is no plaintext fallback.
- Requests that carry the password or the session never follow redirects, so they only ever reach encoredecks.com.
- If the session has expired, the upload stops (it is not silently uploaded anonymously) and the account is unlinked; sign in again to continue.
