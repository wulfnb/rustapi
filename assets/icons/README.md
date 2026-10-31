# RustAPI application icon

The **Hyper-Bolt Brackets** icon (code brackets `{ }` framing a bidirectional lightning arrow on a dark carbon tile) is RustAPI's identity on every platform and on the web.

## Palette

- Tile: carbon fiber slate (`#12131A` to `#0E0D14`) with a graphite metal bevel
- Accent: crimson-orange (`#FF3B30`), ember amber (`#FF9500`), solar gold (`#FFD60A`)

## Regenerating

```sh
uv run scripts/generate-icons.py
```

`rustapi-icon-source.png` is the only hand-made input. It is a flattened render: the tile sits on a baked-in dark background with no alpha channel. The script cuts the tile out with a superellipse mask (`TILE_BOX`, `TILE_EXPONENT`). Never ship the source directly, or the background shows as a dark frame around the icon. `iconutil` (macOS only) builds the `.icns`; on other systems that file is left as is.

## Files and per-platform rules

| File | Used by | Rule |
| --- | --- | --- |
| `rustapi-icon.png` | master | transparent, tile at 94 % of 1024 px |
| `rustapi-icon-macos.png` | master | Apple Big Sur grid: 824 px tile centred on 1024 px, soft shadow |
| `RustAPIIcon.icns` | macOS app bundle (`scripts/package.py`) | built from the macOS grid; a full-bleed or opaque icon gets boxed in a grey tile by macOS 26+ |
| `rustapi-icon-macos-512.png` | runtime Dock icon (`src/main.rs`, macOS) | same grid, so the running app matches the bundle icon |
| `rustapi-icon-256.png` | runtime window icon (Windows, X11) | transparent, 94 % |
| `RustAPIIcon.ico` | `rustapi.exe` (`build.rs`) and Inno Setup installer | 16–256 px, transparent, 94 % |
| `hicolor/<N>x<N>/apps/rustapi.png` | Linux tarball + Docker image, with `packaging/linux/rustapi.desktop` | 16–512 px; Wayland finds it through the `rustapi` app id |
| `site/favicon.png`, `site/logo.png` | website | transparent |
| `site/apple-touch-icon.png` | website | opaque full-bleed square; iOS applies its own mask |

At 16/24/32 px the bevel is cropped away so the glyph stays legible.
