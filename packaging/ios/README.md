# WordCraft for iPad

An iPadOS build of the WordCraft desktop app, made for an iPad with a hardware keyboard and
trackpad (Magic Keyboard). It is the same app as on the desktop, with the ribbon, styles,
tables, track changes and .docx files, drawn with Metal.

The IPA is **for sideloading**: it isn't signed for the App Store, and you install it with your
own Apple ID.

## Get the IPA

1. On GitHub, open **Actions › iPad IPA** in this repository (enable Actions for the fork first
   if GitHub asks).
2. Open the latest green run, or press **Run workflow** on the `ios-ipad` branch.
3. Download the **wordcraft-ipados** artifact and unzip it: inside is
   `wordcraft-<version>-ipados.ipa`.

To build it yourself on a Mac with Xcode and rustup:

```bash
packaging/ios/package.sh
```

The IPA lands in `dist/release/`. Set `CRAFT_FONTS_DIR` and `CRAFT_I18N_DIR` to checkouts of
storytold/craft-fonts and storytold/craft-i18n to embed the fonts and translations, as the
workflow does.

## Install it

Any sideloading tool that signs with your Apple ID works. With a free Apple ID the app runs for
7 days, then needs signing again (AltStore and SideStore do that in the background). A paid
developer account gives a year.

- **Sideloadly** (Windows or macOS): connect the iPad by USB, drop the IPA on Sideloadly, sign in
  with your Apple ID and press Start.
- **AltStore or SideStore**: open the IPA from AltStore's **My Apps › +**.

Then, on the iPad:

1. **Settings › Privacy & Security › Developer Mode**: on (iPadOS 16 and later asks for this
   once, then restarts).
2. **Settings › General › VPN & Device Management**: trust your Apple ID's developer certificate.

## Using it

- **Files.** File › Open, Save As and Export use the iPad's own document picker, so you can open
  and save anywhere the Files app reaches: On My iPad, iCloud Drive and other providers. A
  document opened from somewhere else is edited where it is, and Save writes back to it. Save As
  asks where to put the file; if the name is taken, the picker asks whether to replace it.
- **Open in WordCraft.** WordCraft is listed for Word documents and templates (.docx, .doc,
  .dotx), OpenDocument text, RTF, Markdown, HTML and plain text. In the Files app, touch and hold
  a document and choose **Share › WordCraft** (or **Open With**); other apps' share sheets offer
  it too.
- **Keyboard.** Typing, Return, Tab, Delete, the arrow keys (with Shift, Option and Command), Page
  Up/Down, and Command and Control shortcuts all work, and held keys repeat.
- **On-screen keyboard.** It appears when you tap into the text (the document, or a text box in a
  dialog), not when you tap a button or scroll, and never while a hardware keyboard is attached.
  Its dismiss key hides it until you tap into text again.
- **Trackpad.** The pointer hovers over controls, a click is a mouse click, a two-finger click
  opens the context menu, and two-finger scrolling scrolls, with momentum.
- **Touch.** Tap to place the caret or press a button; double-tap selects a word. Drag with one
  finger to scroll (it glides on when you let go). Touch and hold, then drag, to select text.
  Pinch with two fingers to zoom. Apple Pencil selects as it moves, like a mouse.
- **Clipboard.** Copy and paste go through the iPad's clipboard: plain text, HTML, RTF and
  pictures, to and from other apps. iPadOS may ask you to allow pasting from another app.
- **Multitasking.** Split View, Slide Over and Stage Manager resize the window.

## Known limits

- Save As can't rename in the picker: the file gets the document's name, and you can rename it in
  the Files app.
- A document opened from another provider is rewritten in place on Save (a copy of its previous
  version is kept in WordCraft's temporary folder until the save succeeds); iPadOS doesn't let
  the app put a temporary file beside it.
- Printing doesn't reach AirPrint. Export to PDF and print the PDF from Files.
- The pointer keeps its round iPadOS shape; it doesn't turn into an I-beam over text.
- Messages the desktop app shows in a system dialog box (rare; for example, a failed start) only
  go to the log on iPad.
- No self-update: install a newer IPA the same way.

## How the build differs from the desktop one

`prepare-sources.sh` fetches two dependencies and patches them for iPadOS (`patches/`):

- **winit** (storytold/winit at the workspace's pinned rev): hardware keyboard key events with
  modifiers and repeat (UIKit presses); trackpad hover, two-finger scroll with momentum and clicks
  as mouse buttons; finger taps, one-finger scrolling, press-and-drag and pinch; the on-screen
  keyboard only after a tap at the caret; and an app delegate that hands documents opened from
  other apps to the window as dropped files.
- **rfd** 0.17.2: an iOS backend on UIKit's document picker (open in place, export for Save As,
  folders).

The app itself uses UIPasteboard for the clipboard (`apps/wordcraft/src/ios.rs`, through
`crates/ios-pasteboard`), keeps its settings in `Library/Application Support/WordCraft` inside
the app's container, and leaves out the desktop-only parts (X11/Wayland, arboard, the fixed
window size, self-update).
