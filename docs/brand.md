# Fyle visual identity

Fyle's identity pairs a quiet midnight surface with bright, purposeful accents.
The folder-and-page mark represents the app's main job directly and stays
recognizable at Dock, taskbar and launcher sizes.

## Core mark

`assets/fyle.svg` is the master mark. Its rounded midnight tile frames a cyan
folder with a clear document inset and a small lime signal dot. Avoid adding
wording, shadows outside the tile or decorative detail to the mark. Use the
SVG for scalable Linux and web surfaces; `assets/fyle.icns` is the macOS icon
rendered from the same source.

Regenerate the macOS icon on a Mac with `rsvg-convert` and Apple's `iconutil`:

```sh
scripts/build_macos_icon.sh
```

## Palette

| Role | Color | Use |
| --- | --- | --- |
| Midnight | `#09121F` | App icon and deep surfaces |
| Slate | `#14253A` | Raised icon surface |
| Aqua | `#55F0D2` | Folder and primary mark |
| Cyan | `#24B8D2` | Folder depth and interactive accent |
| Paper | `#F4FCFF` | File/document surface |
| Lime | `#B9F85B` | Small focus cue |

Fyle's original cyberpunk theme remains available in the interface: electric
blue, cyber green, magenta and amber continue to distinguish interactive and
file-category states. The app mark uses a smaller, calmer subset so it reads
clearly in system chrome and at small sizes.

## macOS app bundle

macOS release archives contain `Fyle.app` and `bin/fyle-cli`. Copy `Fyle.app` to
`/Applications` or `~/Applications`; launch it from Finder, Launchpad or the
Dock. The release bundle is not Developer ID signed or notarized; macOS may ask
you to confirm the first launch in Privacy & Security. `scripts/package_macos.sh`
creates the bundle and release archive from the standard Cargo release binaries.
