# First warning. 

This tool is developed with help of AI (Claude Code & Mistral Vibe now). As the developer, I stay in control and double check for security issues (especially on encoredecks connection for example). 


# Project 

This project has been developed for people using the Weiss Schwarz Simulator.

It runs on Linux and Windows (the simulator itself keeps the same folder layout on both).


# Installation 

# Download 

I will soon setup two archives as a new release, so you can download them 

# Build from source

Clone the GIT using the command line or software you prefer.

Example : 

```sh
git clone https://github.com/Fireez30/SimuDeckParser_RUST.git
cd SimuDeckParser_RUST
```

Build it with a Rust toolchain (install one with [rustup](https://rustup.rs)):

Linux : 

```sh
bash build_linux.sh
./target/release/simu_deck_parser       
```

Windows : 

```sh
bash build_windows.sh
.\target\x86_64-pc-windows-gnu\release\simu_deck_parser.exe
```

Run the tests with `cargo test`. Ignored tests cover the rest: `cargo test -- --ignored` also queries encoredecks.com and renders every screen to `target/screenshots` (or `$SCREENSHOT_DIR`); with `SIMULATOR_ROOT=/path/to/simulator` they load a real install (read-only) and report cards without artwork and unresolved deck codes.

# Usage

The interface follows Material 3: a navigation rail on the left (Cards, Decks, Settings), light and dark themes (following the system by default; switch from the rail or Settings).

## First start

The tool asks for the folder where the simulator is installed: the one containing `CardContent` and `Decks` (for example `~/Games/SimuWeiss`). Simulator folders found under your home folder (and in Steam libraries) are listed so you can pick one; you can also paste a path (a subfolder such as `CardContent` or `Decks` works too). 

The field tells you right away whether the folder is a simulator install.

The tool reads:

- the cards from `CardContent/Cards` (every serie folder with a `SingleSetData.txt`),
- the common effects from `<Game>_Data/StreamingAssets/CommonEffects_USER_REFERENCE.txt`,
- the decks from `Decks`, one text file per deck.
- the sleeves from either `<Game>_Data/StreamingAssets/Sleeves` , or in `Sleeves`.

It saves the simulator path so you don't have to load it again,  in `~/.config/weiss_simulator_parser/settings.conf` for linux, or in %APPDATA% for windows, which is the default config folder for the OS.

The settings file contains : 

- encoredecks user name (session token is encrypted in credentials)
- path to simulator 
- What screen should be opened by default (last before close)
- What series should be opened by default (last before close)
- Current theme

## Any menu showing a card includes the following : 

- Image
- Name 
- Code 
- Color
- Type 
- Level
- Cost 
- Power
- Souls 
- Traits 
- A related card button : this button opens a menu showing you all cards related to this card (card cited in the card text, or cards citing this card)
- Effect

## Cards

Top row: 
- Pick a serie at the top (the list has a search field); its cards load immediately. When the serie has several sets, a second menu narrows the view to one set.
- Search by name, card code or effect text with the search bar (`Ctrl+F`). The search applies within the active filters.
- Right buttons in order : hide filter menu, toggle grid view, toggle list view with effect with effect.

Filters menu on the left : 
- Filter on : type, color, level, cost, only climax combos and traits. Nothing selected means no filter; the first chip you pick narrows the list, the next ones add to it. 
- "Clear" resets everything.

## Decks

- Decks of the simulator's `Decks` folder are listed on the left (with a search field when there are many), the AI opponents' decks (`AI_*.txt`) in their own section. Click one to open it.

## Deck page : 
- Bind sleeves to a deck (right now, sleeves are not saved between update, until the simulator is fully updated.)
- Edit page to add or remove cards to the deck.
- Duplicate to create a copy of the deck. 
- Upload will send the deck to encore decks. Either using your account if you set it up in the settings, or as anonymous (please don't spam this).
- The deck page shows its size (with a warning when it is not 50 cards or has more than 8 climaxes), the type split, the triggers reparition,the level curve and the colors.
- Than all card are shown , sorted by type first, than by level. 
- Codes that matched no card are listed in a banner.
- Hover / click cards to preview / pin them, as in the card view.


## Precisions on : Encore decks upload

Open a deck in the Decks screen and click "Upload" to publish it on encoredecks.com. After confirming, the deck is sent in the background and the dialog shows its link (open it in the browser or copy it).

The deck is created as a new public deck in your Encore Decks account when one is linked (see below). Without an account it is uploaded anonymously, and then it cannot be edited or deleted there afterwards. Card codes are matched to the Encore Decks cards the same way as for imports (JP or EN, foil or non foil); for cards with several arts, the first art (code ending in "a") is used. Only complete decks are published: the deck must have exactly 50 cards and every one of them must exist on Encore Decks. Otherwise nothing is uploaded, and the error lists the codes Encore Decks does not know.

## Deck builder

- "New deck" in the Decks screen opens the builder: the card browser (serie and set menus, search, filters) with the deck on the right.
- Click a card to add a copy, right-click to remove one (in the list view, use the Add / Remove buttons). The deck panel also has −/+ buttons per card, and hovering a card shows it in full.*
- Filters are exactly the same than cards list. 
- The deck panel groups cards by level (characters and events together), with climaxes last, and switches between a list (with −/+ buttons) and a grid of pictures (click adds a copy, right-click removes one).
- The builder follows the deck rules: at most 50 cards and 4 copies of cards sharing a name. It shows the card count and warns above 8 climaxes.
- You can specify sleeves before saving, or let no sleeves (default behaviour).
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


### Precision on Encore Decks account

In Settings, "Encore Decks account", sign in with your Encore Decks email and password to upload decks to your account. "Unlink" signs out.

- The password is never stored. It is sent once, over HTTPS, to encoredecks.com, then wiped from memory.
- Only the login session is kept, in the system credential store: the Secret Service (GNOME Keyring, KWallet) on Linux, the Credential Manager on Windows. These store it encrypted and tied to your login. It is never written to the settings file (which only holds the account name). Without a credential store, the account cannot be linked; there is no plaintext fallback.
- Requests that carry the password or the session never follow redirects, so they only ever reach encoredecks.com.
- If the session has expired, the upload stops (it is not silently uploaded anonymously) and the account is unlinked; sign in again to continue.


## Settings

The settings file contains : 

- encoredecks user name (session token is encrypted in credentials)
- path to simulator 
- What screen should be opened by default (last before close)
- What series should be opened by default (last before close)
- Current theme

## Release note 
latest (0.1.2) :

- Added feature to bind sleeve to a deck. Use the simulator Sleeves folder
- the sleeve filter can set or unset sleeve for both new or update decks 
- Added a name filter for sleeve filtering
- Prepared for the move of Sleeves folder in the simulator

0.1.1 : 

Added a whole section that use encoredecks connection to list encore decks decks.
- list of all decks paginated 
- possibility to see a full encoredecks deck 
- possibility to import a deck from your encore decks deck, not only by list

0.1.0:
Visuals :
- New material design with theme switching

Settings : 
- Added settings button to refresh data
- Added a login form to encore decks (for upload). Login use password to retrieve the token, but password is not stored. The token is encrypted in Windows / Linux keyring or credentials manager. Only username is kept in plain text in settings

Decks : 
- Added a new deck button, to build a new deck to be added to the simulator 
- Added a button to duplicate a deck 
- Added a button to upload a deck to encoredecks (upload as anonymous if not logged in, to your account if logged in settings)
- Added a button to download a PDF with proxies of the deck
- Added a button do download a PDF of deck translations
- Added trigger count to deck stats 

Cards view : 

- Display cards as a grid or list
- Added a related cards button (or on mouse hover in a card effect, that display the card citing the current card (resonate, bond , etc)) 
- Sorted cards by types > color > cost > power 
- Added a combo climax filter
- Added traits 

Previous functionalities : 
Cards : 
- Card listing from the cards folder. 
- Filtering by card type, card color, level , cost
- Filtering by card text (effect, name or card code) 
- Card panel on hover or click, with effect visible

Decks : 
- Import from encoredecks, with automated matching (not perfect)
- Deck list 
- Deck stats (cards, types, level and colors) 






# Planned features

Soon : 

- A proper building and packaging pipeline for linux and windows, instead of giving raw builds links to share 
- Add photos to the README
- A new round of security check 
- Getting feedbacks 
- What ever crosses my mind 

Longer : 
 - mac support ?



# Questions

- I have updated or changed my simulator path, how can I change it in the software ? 

=> Open Settings and press "Change folder…". You will be able to enter the new path and save it again.

- I have a bug, what can I do ? 

=> You can post an issue in this github repository, explaining the bug, and if possible how you reached the bug, a screenshot and/or a video

- What about the security ? 

=> When you know what you are doing, AI can lead to quite secure code. I made sure to audit security (especially wiping sensitive memory, not storing passwords , ...). Of course no code is perfect, and I'm sure I would have created unsecure code without AI too. 
Except the encoredeck connection, the tool is fully local












